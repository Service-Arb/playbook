#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
ask_llm = { version = "3.3", default-features = false }
serde_json = "1"
tokio = { version = "1", features = ["rt", "time"] }
---

//! `./scripts/call-watch.rs [<capture.md>...]` — write `<capture>/shown.md` for every call capture
//! whose recording has a picture and nobody has watched yet: what is on screen that the speech does
//! not say, each line at the frame it was read off, and that frame kept beside it. With no arguments
//! it takes every capture under `ref/`.
//!
//! The frames looked at are the ones where the picture changes, and one every so often besides.

use std::{
	path::{Path, PathBuf},
	process::Command,
	sync::Mutex,
	time::Duration,
};

use ask_llm::{Api, Client, Error, Model};

/// The scene score past which ffmpeg counts a frame as the picture changing.
const SCENE: f64 = 0.06;
/// Frames looked at in one request.
const BATCH: usize = 24;
const WORKERS: usize = 4;
const PLATFORMS: [&str; 5] = ["ref/loom", "ref/fathom", "ref/drive", "ref/vimeo", "ref/vocaroo"];
/// The line of `ref/README.md` this rewrites with what watching has cost so far.
const TOTALS: &str = "- what the recordings show, as `scripts/call-watch.rs` read it:";

/// A capture, as much of it as watching needs.
struct Capture {
	title: String,
	source: String,
	/// `(start secs, header, text under it)`
	chapters: Vec<(u64, String, String)>,
}

fn main() {
	let root = repo_root();
	let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
	let captures: Vec<PathBuf> = match args.is_empty() {
		true => PLATFORMS
			.iter()
			.map(|d| root.join(d))
			.filter(|d| d.exists())
			.flat_map(|d| std::fs::read_dir(&d).unwrap_or_else(|e| panic!("reading {}: {e}", d.display())))
			.map(|e| e.expect("a directory entry is readable").path())
			.filter(|p| p.extension().is_some_and(|e| e == "md"))
			.collect(),
		false => args.into_iter().map(|a| std::fs::canonicalize(&a).unwrap_or_else(|e| panic!("{}: {e}", a.display()))).collect(),
	};
	let todo: Vec<PathBuf> = captures.into_iter().filter(|c| !c.with_extension("").join("shown.md").exists()).collect();
	eprintln!("{} recordings to watch", todo.len());

	let queue = Mutex::new(todo.into_iter());
	let spent = Mutex::new(0f64);
	std::thread::scope(|s| {
		for _ in 0..WORKERS {
			s.spawn(|| {
				let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a current-thread runtime builds");
				//LOOP: bounded by the queue, which only drains
				loop {
					let Some(path) = queue.lock().expect("no worker panics holding it").next() else { break };
					if let Some(cost) = rt.block_on(watch(&root, &path)) {
						let mut spent = spent.lock().expect("no worker panics holding it");
						*spent += cost;
						eprintln!("${cost:.4} {} — ${:.4} this run", path.display(), *spent);
					}
				}
			});
		}
	});
	totals(&root);
}

fn repo_root() -> PathBuf {
	let mut dir = std::env::current_dir().expect("a process has a cwd");
	loop {
		if dir.join(".git").exists() {
			return dir;
		}
		if !dir.pop() {
			panic!("run this from inside the playbook checkout");
		}
	}
}

/// Watch one recording into its `shown.md`, returning what it cost; `None` for one with no picture.
async fn watch(root: &Path, path: &Path) -> Option<f64> {
	let kept = path.with_extension("");
	let media = std::fs::read_dir(&kept)
		.unwrap_or_else(|e| panic!("{}: {e} — run call-pull.rs to fetch the recording", kept.display()))
		.map(|e| e.expect("a directory entry is readable").path())
		.find(|p| p.file_stem().is_some_and(|s| s == "recording"))
		.unwrap_or_else(|| panic!("{} holds no recording — run call-pull.rs to fetch it", kept.display()));
	if probe(&media, &["-select_streams", "v", "-show_entries", "stream=index"]).is_empty() {
		eprintln!("audio only, nothing shown — {}", path.display());
		return None;
	}
	let capture = read(path);
	// `(gap, floor)`: frames at least `gap` apart, and one `floor` after the last even if nothing changed at
	// once, since a board scrolled slowly never differs much from one frame to the next. A phone filming a
	// place pans smoothly past a sign in a second or two, so every second of it is looked at
	let (gap, floor) = match capture.source.contains("drive.google.com") {
		true => (1., 1),
		false => (4., 30),
	};

	let scratch = root.join("tmp/call-watch").join(kept.file_name().expect("a capture has a name"));
	if scratch.exists() {
		std::fs::remove_dir_all(&scratch).expect("the scratch dir is ours");
	}
	std::fs::create_dir_all(&scratch).expect("tmp/ is writable");
	let frames = changes(&media, &scratch, gap, floor);
	eprintln!("  {} frames to look at — {}", frames.len(), path.display());

	let mut entries: Vec<(u64, String, Option<String>)> = Vec::new();
	let mut cost = 0f64;
	let mut model = None;
	for (i, batch) in frames.chunks(BATCH).enumerate() {
		let from = batch[0].0;
		let to = frames.get((i + 1) * BATCH).map_or(u64::MAX, |f| f.0);
		let transcript: String = capture
			.chapters
			.iter()
			.enumerate()
			.filter(|(i, (t, _, _))| *t < to && capture.chapters.get(i + 1).is_none_or(|(next, _, _)| *next > from))
			.map(|(_, (_, header, text))| format!("{header}\n{text}\n"))
			.collect();
		let times: Vec<String> = batch.iter().map(|(t, _)| format!("{} ({t})", stamp(*t))).collect();
		let prompt = format!(
			"The {} images are frames of the recording \"{}\", in order at these times (seconds in brackets): {}.\n\
			 Each was taken where the picture changed, or {floor}s after the last. Each carries its second in the band under it. What is said around them is already transcribed, below.\n\
			 List what the frames show that the speech does not already say: screen content, UI states, numbers on screen, physical scene details. \
			 The people on the call are never entries: their webcams, tiles, names, how many there are, and their joining, leaving, or turning a camera on or off. \
			 What is shared or filmed is: a shared screen, a document, a dashboard, a site, a phone screen, a place.\n\
			 Answer {{\"entries\": [{{\"t_secs\": <the seconds printed under the one frame it is read off>, \"shown\": <one sentence>, \"on_screen_text\": <legible text copied as written, where it carries something, else omitted>}}]}}. \
			 A frame that shows nothing new gets no entry.\n\n{}",
			batch.len(),
			capture.title,
			times.join(", "),
			match transcript.trim().is_empty() {
				true => "Nobody speaks around these frames.".to_string(),
				false => format!("Transcript:\n\n{transcript}"),
			}
		);
		let client = batch
			.iter()
			.fold(Client::default().model(Model::Medium).force_json(), |c, (_, f)| c.append_file_from_path(f).expect("the frame was written"));
		let answer = ask(&client, &prompt, path).await;
		cost += answer.cost_cents as f64 / 100.;
		model.get_or_insert(answer.model.clone());
		let listed: serde_json::Value = serde_json::from_str(&answer.text).unwrap_or_else(|e| panic!("{}: the answer is not the json asked for: {e}\n{}", path.display(), answer.text));
		for e in listed["entries"].as_array().unwrap_or_else(|| panic!("{}: the answer has no `entries` — {listed}", path.display())) {
			let t = e["t_secs"].as_u64().unwrap_or_else(|| panic!("{}: an entry with no `t_secs` — {e}", path.display()));
			assert!(batch.iter().any(|(f, _)| *f == t), "{}: an entry at {t}s, which is no frame it was shown — {e}", path.display());
			let shown = e["shown"].as_str().unwrap_or_else(|| panic!("{}: an entry with no `shown` — {e}", path.display()));
			let on_screen = e["on_screen_text"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
			entries.push((t, shown.trim().to_string(), on_screen));
		}
		eprintln!("  {}–{} read — {}", stamp(from), stamp(batch.last().expect("chunks are never empty").0), path.display());
	}
	entries.sort_by_key(|e| e.0);

	let dir = kept.join("frames");
	std::fs::create_dir_all(&dir).expect("the capture's dir is writable");
	let seek = match capture.source.contains("vimeo.com") {
		true => "#t=",
		false => "?t=",
	};
	let today = String::from_utf8(Command::new("date").arg("+%F").output().expect("date runs").stdout).expect("date prints ascii");
	// not `source:`, which would make this a second capture of the recording
	let name = path.file_name().expect("a capture has a name").to_string_lossy();
	let mut out = format!(
		"# shown: {}\n\n\
		 - capture: [{name}](../{name})\n\
		 - watched by: `{}`, over {} frames, where the picture changes or every {floor}s · `scripts/call-watch.rs`\n\
		 - watched: {}\n\
		 - cost: ${cost:.4}\n",
		capture.title,
		model.expect("there is always a first frame, so always a request"),
		frames.len(),
		today.trim(),
	);
	for (t, shown, on_screen) in &entries {
		let frame = &frames.iter().find(|f| f.0 == *t).expect("checked against the batch").1;
		std::fs::copy(frame, dir.join(format!("{t}.jpg"))).expect("the capture's dir is writable");
		out.push_str(&format!("\n- [{}]({}{seek}{t}) {}\n", stamp(*t), capture.source, shown.replace('\n', " ")));
		if let Some(text) = on_screen {
			out.push_str(&format!("  > {}\n", text.replace('\n', " / ")));
		}
		out.push_str(&format!("  ![](frames/{t}.jpg)\n"));
	}
	std::fs::write(kept.join("shown.md"), out).expect("the capture's dir is writable");
	std::fs::remove_dir_all(&scratch).expect("the scratch dir is ours");
	Some(cost)
}

/// One request, waited out while the provider says it is busy.
async fn ask(client: &Client, prompt: &str, path: &Path) -> ask_llm::Response {
	//LOOP: bounded by the attempts
	for attempt in 1..=8u64 {
		match client.ask(prompt).await {
			Ok(r) => return r,
			Err(Error::Api(Api::RateLimited { retry_after, .. })) if attempt < 8 => {
				let wait = retry_after.unwrap_or(Duration::from_secs(30 * attempt));
				eprintln!("  rate limited, waiting {}s — {}", wait.as_secs(), path.display());
				tokio::time::sleep(wait).await;
			}
			Err(Error::Api(Api::Overloaded { .. })) if attempt < 8 => {
				eprintln!("  overloaded, waiting — {}", path.display());
				tokio::time::sleep(Duration::from_secs(30 * attempt)).await;
			}
			Err(e) => panic!("{}: {e:?}", path.display()),
		}
	}
	unreachable!("the last attempt returns or panics")
}

/// `(secs, jpg)` for every frame where the picture changes or `floor` seconds have passed, at least
/// `gap` seconds apart, the first frame always among them.
fn changes(media: &Path, scratch: &Path, gap: f64, floor: u32) -> Vec<(u64, PathBuf)> {
	let out = Command::new("ffmpeg")
		.args(["-hide_banner", "-nostats", "-i"])
		.arg(media)
		.args([
			"-an",
			"-vf",
			&format!("select='eq(n\\,0)+gt(scene\\,{SCENE})+gte(t-prev_selected_t\\,{floor})',showinfo,scale=-2:'min(720\\,ih)',pad=iw:ih+40:0:0:black,drawtext=text='%{{eif\\:t\\:d}}s':x=10:y=h-32:fontsize=26:fontcolor=white"),
			"-fps_mode",
			"passthrough", // `vfr` drops a frame that shares its stamp with the one before, and showinfo still names it
			"-q:v",
			"4",
		])
		.arg(scratch.join("%06d.jpg"))
		.output()
		.expect("ffmpeg runs");
	assert!(out.status.success(), "ffmpeg could not read the frames of {}: {}", media.display(), String::from_utf8_lossy(&out.stderr));
	let log = String::from_utf8_lossy(&out.stderr);
	let times: Vec<f64> = log
		.lines()
		.filter(|l| l.contains("Parsed_showinfo"))
		.filter_map(|l| l.split_once(" pts_time:"))
		.map(|(_, rest)| rest.split_whitespace().next().expect("split yields at least once").parse().unwrap_or_else(|e| panic!("showinfo wrote `{rest}`: {e}")))
		.collect();
	let mut kept: Vec<(u64, PathBuf)> = Vec::new();
	let mut last = f64::NEG_INFINITY;
	for (i, t) in times.iter().enumerate() {
		let jpg = scratch.join(format!("{:06}.jpg", i + 1));
		assert!(jpg.exists(), "showinfo named a frame ffmpeg did not write: {}", jpg.display());
		if t - last >= gap {
			kept.push((*t as u64, jpg));
			last = *t;
		}
	}
	assert!(!kept.is_empty(), "{}: a picture with no first frame", media.display());
	kept
}

fn read(path: &Path) -> Capture {
	let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
	let field = |key: &str| text.lines().find_map(|l| l.strip_prefix(&format!("- {key}: "))).unwrap_or_else(|| panic!("{} states no `{key}:`", path.display()));
	let title = text.lines().next().and_then(|l| l.strip_prefix("# ")).unwrap_or_else(|| panic!("{} opens with no title", path.display()));
	let transcript = text.split_once("\n## transcript\n").unwrap_or_else(|| panic!("{} has no `## transcript`", path.display())).1;
	let mut chapters: Vec<(u64, String, String)> = Vec::new();
	//LOOP: bounded by the lines of a finite file
	for line in transcript.lines() {
		match line.strip_prefix("### [") {
			Some(_) => {
				let link = line.split_once("](").expect("call-pull.rs holds headers to `### [stamp](link)`").1;
				let t = link.split(')').next().expect("split yields at least once").rsplit_once("t=").expect("a header links a second").1;
				chapters.push((t.parse().unwrap_or_else(|e| panic!("{}: `{line}`: {e}", path.display())), line.to_string(), String::new()));
			}
			None =>
				if let Some((_, _, body)) = chapters.last_mut() {
					body.push_str(line);
					body.push('\n');
				},
		}
	}
	assert!(chapters.windows(2).all(|w| w[0].0 < w[1].0), "{}: chapters out of order — run call-pull.rs --check", path.display());
	Capture { title: title.to_string(), source: field("source").trim_matches(['<', '>']).to_string(), chapters }
}

fn probe(media: &Path, args: &[&str]) -> String {
	let out = Command::new("ffprobe").args(["-v", "error"]).args(args).args(["-of", "csv=p=0"]).arg(media).output().expect("ffprobe runs");
	assert!(out.status.success(), "ffprobe could not read {}: {}", media.display(), String::from_utf8_lossy(&out.stderr));
	String::from_utf8(out.stdout).expect("ffprobe prints ascii").trim().to_string()
}

/// `MM:SS` under an hour, `H:MM:SS` past it, as call-pull.rs writes a chapter's start.
fn stamp(secs: u64) -> String {
	match secs / 3600 {
		0 => format!("{:02}:{:02}", secs / 60, secs % 60),
		h => format!("{h}:{:02}:{:02}", (secs % 3600) / 60, secs % 60),
	}
}

/// What every `shown.md` on disk cost, summed into `ref/README.md`'s line for it.
fn totals(root: &Path) {
	let (mut n, mut usd) = (0, 0f64);
	for dir in PLATFORMS.iter().map(|d| root.join(d)).filter(|d| d.exists()) {
		//LOOP: bounded by the files of a finite directory
		for entry in std::fs::read_dir(&dir).expect("listed above") {
			let shown = entry.expect("a directory entry is readable").path().join("shown.md");
			if !shown.exists() {
				continue;
			}
			let text = std::fs::read_to_string(&shown).unwrap_or_else(|e| panic!("reading {}: {e}", shown.display()));
			let cost = text.lines().find_map(|l| l.strip_prefix("- cost: $")).unwrap_or_else(|| panic!("{} states no cost", shown.display()));
			usd += cost.trim().parse::<f64>().unwrap_or_else(|e| panic!("{}: `{cost}`: {e}", shown.display()));
			n += 1;
		}
	}
	let readme = root.join("ref/README.md");
	let text = std::fs::read_to_string(&readme).expect("ref/README.md is the registry");
	let line = text.lines().find(|l| l.starts_with(TOTALS)).unwrap_or_else(|| panic!("ref/README.md has no `{TOTALS}` line to keep the totals on"));
	let now = format!("{TOTALS} {n} recordings, ${usd:.2} in all");
	std::fs::write(&readme, text.replace(line, &now)).expect("ref/README.md is writable");
	eprintln!("{now}");
}
