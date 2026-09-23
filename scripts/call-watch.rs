#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
serde_json = "1"
ureq = { version = "3", features = ["json"] }
---

//! `./scripts/call-watch.rs [<capture.md>...]` — write `<capture>/shown.md` for every call capture
//! whose recording has a picture and nobody has watched yet: what is on screen that the speech does
//! not say, second by second, each with the frame it was read off. With no arguments it takes every
//! capture under `ref/`.
//!
//! The picture is read by gemini, the one model that takes video as it is. The audio is dropped
//! first, since the transcript already holds it, and a long call is watched in windows cut on its
//! chapters — one prompt over hours reads as a sparse and vague timeline.

use std::{
	path::{Path, PathBuf},
	process::Command,
	sync::Mutex,
	time::Duration,
};

const MODEL: &str = "gemini-3.8-flash";
const USD_PER_M_IN: f64 = 0.75;
const USD_PER_M_OUT: f64 = 3.75;
const API: &str = "https://generativelanguage.googleapis.com";
const WINDOW_SECS: u64 = 1200;
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
	let key = std::env::var("GEMINI_KEY").expect("GEMINI_KEY is set in the user's session");
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
				//LOOP: bounded by the queue, which only drains
				loop {
					let Some(path) = queue.lock().expect("no worker panics holding it").next() else { break };
					if let Some(cost) = watch(&root, &key, &path) {
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
fn watch(root: &Path, key: &str, path: &Path) -> Option<f64> {
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
	let duration = probe(&media, &["-show_entries", "format=duration"]).parse::<f64>().expect("ffprobe prints the duration in seconds") as u64;
	let capture = read(path);
	// screen recordings are legible at low resolution; a phone filming signage and doorways is not
	let (resolution, res_name) = match capture.source.contains("drive.google.com") {
		true => ("MEDIA_RESOLUTION_HIGH", "high"),
		false => ("MEDIA_RESOLUTION_LOW", "low"),
	};

	let scratch = root.join("tmp/call-watch").join(kept.file_name().expect("a capture has a name"));
	std::fs::create_dir_all(&scratch).expect("tmp/ is writable");
	let silent = scratch.join("picture.mp4");
	run(Command::new("ffmpeg").args(["-v", "error", "-y", "-i"]).arg(&media).args(["-an", "-c", "copy"]).arg(&silent));
	let file = upload(key, &silent);

	let mut entries: Vec<(u64, String, Option<String>)> = Vec::new();
	let (mut tokens_in, mut tokens_out) = (0u64, 0u64);
	for (from, to) in windows(&capture.chapters, duration) {
		let transcript: String = capture
			.chapters
			.iter()
			.enumerate()
			.filter(|(i, (t, _, _))| *t < to && capture.chapters.get(i + 1).is_none_or(|(next, _, _)| *next > from))
			.map(|(_, (_, header, text))| format!("{header}\n{text}\n"))
			.collect();
		let prompt = format!(
			"This is the part of the recording \"{}\" that runs from {} to {} of it. What is said in it is already transcribed, below.\n\
			 List what is shown that the speech does not already say: screen content, UI states, numbers on screen, physical scene details. \
			 One entry per distinct thing shown, at the second it is best visible. \
			 The people on the call are never entries: their webcams, tiles, names, and their joining, leaving, or turning a camera on or off. \
			 What is shared or filmed is: a shared screen, a document, a dashboard, a site, a phone screen, a place.\n\
			 `t_secs` counts from the start of the whole recording, not of this part. `on_screen_text` is legible text copied as written, where it carries something.\n\
			 Nothing shown beyond what is said is an empty list.\n\n\
			 {}",
			capture.title,
			stamp(from),
			stamp(to),
			match transcript.trim().is_empty() {
				true => "Nobody speaks in this part.".to_string(),
				false => format!("Transcript:\n\n{transcript}"),
			}
		);
		let body = serde_json::json!({
			"contents": [{"role": "user", "parts": [
				{"file_data": {"mime_type": "video/mp4", "file_uri": file["uri"]}, "video_metadata": {"start_offset": format!("{from}s"), "end_offset": format!("{to}s")}},
				{"text": prompt},
			]}],
			"generationConfig": {
				"mediaResolution": resolution,
				"responseMimeType": "application/json",
				"responseSchema": {"type": "ARRAY", "items": {"type": "OBJECT", "properties": {
					"t_secs": {"type": "INTEGER"}, "shown": {"type": "STRING"}, "on_screen_text": {"type": "STRING"}
				}, "required": ["t_secs", "shown"]}},
			},
		});
		eprintln!("  watching {} {}–{}", capture.title, stamp(from), stamp(to));
		let answer = call(key, "POST", &format!("{API}/v1beta/models/{MODEL}:generateContent"), Some(&body));
		let usage = &answer["usageMetadata"];
		let count = |k: &str| usage[k].as_u64().unwrap_or(0); // absent when zero, as thinking is on a model that did none
		tokens_in += count("promptTokenCount");
		tokens_out += count("candidatesTokenCount") + count("thoughtsTokenCount");
		let text = answer["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or_else(|| panic!("{}: gemini answered no text — {answer}", path.display()));
		std::fs::write(scratch.join(format!("{from}.json")), text).expect("tmp/ is writable");
		let listed: serde_json::Value = serde_json::from_str(text).unwrap_or_else(|e| panic!("{}: gemini's answer is not the schema's json: {e}\n{text}", path.display()));
		for e in listed.as_array().expect("the schema is an array") {
			let t = e["t_secs"].as_u64().expect("the schema requires t_secs");
			assert!((from..=to).contains(&t), "{}: gemini placed an entry at {} in the window {}–{} — {e}", path.display(), stamp(t), stamp(from), stamp(to));
			let on_screen = e["on_screen_text"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
			entries.push((t, e["shown"].as_str().expect("the schema requires shown").trim().to_string(), on_screen));
		}
	}
	call(key, "DELETE", &format!("{API}/v1beta/{}", file["name"].as_str().expect("an uploaded file has a name")), None);
	entries.sort_by_key(|e| e.0);

	let frames = kept.join("frames");
	std::fs::create_dir_all(&frames).expect("the capture's dir is writable");
	let cost = tokens_in as f64 / 1e6 * USD_PER_M_IN + tokens_out as f64 / 1e6 * USD_PER_M_OUT;
	let seek = match capture.source.contains("vimeo.com") {
		true => "#t=",
		false => "?t=",
	};
	let today = String::from_utf8(Command::new("date").arg("+%F").output().expect("date runs").stdout).expect("date prints ascii");
	let mut out = format!(
		"# shown: {}\n\n\
		 - source: <{}>\n\
		 - watched by: `{MODEL}` at {res_name} media resolution, audio dropped, in windows of {} minutes at most · `scripts/call-watch.rs`\n\
		 - watched: {}\n\
		 - cost: ${cost:.4} — {tokens_in} tokens in, {tokens_out} out\n",
		capture.title,
		capture.source,
		WINDOW_SECS / 60,
		today.trim(),
	);
	for (t, shown, on_screen) in &entries {
		let frame = frames.join(format!("{t}.jpg"));
		if !frame.exists() {
			run(Command::new("ffmpeg")
				.args(["-v", "error", "-ss", &t.to_string(), "-i"])
				.arg(&media)
				.args(["-frames:v", "1", "-vf", "scale=-2:'min(720,ih)'", "-q:v", "4"])
				.arg(&frame));
		}
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
			None => {
				let last = chapters.last_mut();
				if let Some((_, _, body)) = last {
					body.push_str(line);
					body.push('\n');
				}
			}
		}
	}
	assert!(chapters.windows(2).all(|w| w[0].0 < w[1].0), "{}: chapters out of order — run call-pull.rs --check", path.display());
	Capture { title: title.to_string(), source: field("source").trim_matches(['<', '>']).to_string(), chapters }
}

/// `(from, to)` secs, each at most `WINDOW_SECS` long, cut at the latest chapter start that fits.
fn windows(chapters: &[(u64, String, String)], duration: u64) -> Vec<(u64, u64)> {
	let mut out = Vec::new();
	let mut from = 0;
	while from < duration {
		let limit = from + WINDOW_SECS;
		let to = match limit >= duration {
			true => duration,
			false => chapters.iter().map(|c| c.0).filter(|&t| t > from && t <= limit).max().unwrap_or(limit), // a chapter longer than a window is cut inside it
		};
		out.push((from, to));
		from = to;
	}
	out
}

/// A Files API resumable upload, polled till gemini has processed it.
fn upload(key: &str, file: &Path) -> serde_json::Value {
	let size = std::fs::metadata(file).expect("the stripped picture was written").len();
	let start = agent()
		.post(&format!("{API}/upload/v1beta/files?key={key}"))
		.header("X-Goog-Upload-Protocol", "resumable")
		.header("X-Goog-Upload-Command", "start")
		.header("X-Goog-Upload-Header-Content-Length", &size.to_string())
		.header("X-Goog-Upload-Header-Content-Type", "video/mp4")
		.send_json(serde_json::json!({"file": {"display_name": file.display().to_string()}}))
		.unwrap_or_else(|e| panic!("starting the upload of {}: {e}", file.display()));
	assert!(start.status().is_success(), "starting the upload of {}: {}", file.display(), start.status());
	let url = start.headers().get("x-goog-upload-url").unwrap_or_else(|| panic!("gemini gave no upload url for {}", file.display())).to_str().expect("a url is ascii").to_string();
	let mut done = agent()
		.post(&url)
		.header("X-Goog-Upload-Offset", "0")
		.header("X-Goog-Upload-Command", "upload, finalize")
		.send(std::fs::File::open(file).expect("the stripped picture was written"))
		.unwrap_or_else(|e| panic!("uploading {}: {e}", file.display()));
	assert!(done.status().is_success(), "uploading {}: {}", file.display(), done.status());
	let mut uploaded: serde_json::Value = done.body_mut().read_json().expect("the upload answers json");
	let name = uploaded["file"]["name"].as_str().expect("an uploaded file has a name").to_string();
	//LOOP: bounded by gemini finishing or failing the processing, which it does in minutes
	while uploaded["file"]["state"] != "ACTIVE" {
		assert_ne!(uploaded["file"]["state"], "FAILED", "gemini could not process {}: {uploaded}", file.display());
		std::thread::sleep(Duration::from_secs(5));
		uploaded = serde_json::json!({"file": call(key, "GET", &format!("{API}/v1beta/{name}"), None)});
	}
	uploaded["file"].clone()
}

/// Statuses are read rather than raised, since gemini's error body is what says what went wrong.
fn agent() -> ureq::Agent {
	ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(1800))).http_status_as_error(false).build().into()
}

/// One API call, retried while gemini says it is busy.
fn call(key: &str, method: &str, url: &str, body: Option<&serde_json::Value>) -> serde_json::Value {
	let agent = agent();
	let url = format!("{url}?key={key}");
	//LOOP: bounded by the attempts
	for attempt in 1..=8 {
		let answer = match (method, body) {
			("POST", Some(b)) => agent.post(&url).send_json(b),
			("GET", None) => agent.get(&url).call(),
			("DELETE", None) => agent.delete(&url).call(),
			_ => unreachable!("only the calls above are made"),
		};
		let mut answer = answer.unwrap_or_else(|e| panic!("{method} {}: {e}", url.split('?').next().expect("split yields at least once")));
		let status = answer.status().as_u16();
		let text = answer.body_mut().with_config().limit(64 * 1024 * 1024).read_to_string().expect("gemini answers in utf-8");
		match status {
			200..=299 => return serde_json::from_str(if text.trim().is_empty() { "{}" } else { &text }).unwrap_or_else(|e| panic!("gemini answered not json: {e}\n{text}")),
			429 | 500 | 503 if attempt < 8 => {
				eprintln!("  gemini is busy ({status}), waiting before attempt {}", attempt + 1);
				std::thread::sleep(Duration::from_secs(30 * attempt));
			}
			_ => panic!("{method} {}: {status}\n{text}", url.split('?').next().expect("split yields at least once")),
		}
	}
	unreachable!("the last attempt returns or panics")
}

fn probe(media: &Path, args: &[&str]) -> String {
	let out = Command::new("ffprobe").args(["-v", "error"]).args(args).args(["-of", "csv=p=0"]).arg(media).output().expect("ffprobe runs");
	assert!(out.status.success(), "ffprobe could not read {}: {}", media.display(), String::from_utf8_lossy(&out.stderr));
	String::from_utf8(out.stdout).expect("ffprobe prints ascii").trim().to_string()
}

fn run(cmd: &mut Command) {
	let status = cmd.status().unwrap_or_else(|e| panic!("running {cmd:?}: {e}"));
	assert!(status.success(), "{cmd:?} failed: {status}");
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
			usd += cost.split_whitespace().next().expect("split yields at least once").parse::<f64>().unwrap_or_else(|e| panic!("{}: `{cost}`: {e}", shown.display()));
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
