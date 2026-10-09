#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
ask_llm = { version = "3.7", default-features = false }
tokio = { version = "1", features = ["rt"] }
---

//! `./scripts/call-watch.rs [--legacy | --every <secs>] [<capture.md>...]` — write `<capture>/shown.md` for every call capture
//! whose recording has a picture and nobody has watched yet: what is on screen that the speech does
//! not say, each line at the frame it was read off, and that frame kept beside it. With no arguments
//! it takes every capture under `ref/`. A screen's frames are read only where the speech says
//! something is shown, timed line by line by local whisper; `--legacy` reads them wherever the picture changes.
//!
//! The reading is `ask_llm`'s `Client::watch`; this decides which recordings, and how `shown.md` reads.

use std::{
	path::{Path, PathBuf},
	process::Command,
	sync::Mutex,
};

use ask_llm::{Client, Footage, Model, Pick, Said, Shown, Watch};

const WORKERS: usize = 4;
const PLATFORMS: [&str; 5] = ["ref/loom", "ref/fathom", "ref/drive", "ref/vimeo", "ref/vocaroo"];
/// The line of `ref/README.md` this rewrites with what watching has cost so far.
const TOTALS: &str = "- what the recordings show, as `scripts/call-watch.rs` read it:";

/// A capture, as much of it as watching needs.
struct Capture {
	title: String,
	source: String,
	summary: Option<String>,
	/// `(start secs, header's title, text under it)`
	chapters: Vec<(u64, String, String)>,
}

fn main() {
	let root = repo_root();
	let args: Vec<String> = std::env::args().skip(1).collect();
	let (pick, args) = match args.as_slice() {
		[mode, rest @ ..] if mode == "--legacy" => (Pick::Changes, rest),
		[flag, secs, rest @ ..] if flag == "--every" => (
			Pick::Likely {
				every: secs.parse().unwrap_or_else(|e| panic!("--every `{secs}`: {e}")),
			},
			rest,
		),
		rest => (Pick::Likely { every: 0.5 }, rest),
	};
	let args: Vec<PathBuf> = args.iter().map(PathBuf::from).collect();
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
					if let Some(cost) = rt.block_on(watch(&path, pick)) {
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
async fn watch(path: &Path, pick: Pick) -> Option<f64> {
	let kept = path.with_extension("");
	let media = std::fs::read_dir(&kept)
		.unwrap_or_else(|e| panic!("{}: {e} — run call-pull.rs <its source> to fetch the recording", kept.display()))
		.map(|e| e.expect("a directory entry is readable").path())
		.find(|p| p.file_stem().is_some_and(|s| s == "recording"))
		.unwrap_or_else(|| panic!("{} holds no recording — run call-pull.rs <its source> to fetch it", kept.display()));
	let capture = read(path);
	let footage = match capture.source.contains("drive.google.com") {
		true => Footage::Filmed,
		false => Footage::Screen,
	};
	let pick = match footage {
		Footage::Filmed => Pick::Changes, // a phone clip is all shown, and often silent
		Footage::Screen => pick,
	};
	let spec = Watch {
		title: capture.title.clone(),
		speech: match pick {
			Pick::Changes => Some(capture.chapters.iter().map(|(secs, title, text)| Said { secs: *secs as f64, text: format!("{title}\n{text}") }).collect()),
			Pick::Likely { .. } => None, // the pick places a span by each line's second, and a capture stamps only its chapters, so whisper times them
		},
		footage,
		pick,
		about: capture.summary.clone(),
		frames: kept.join("frames"),
	};
	let watched = Client::default().model(Model::Video).watch(&media, spec).await.unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
	let watched_by = match (&watched.model, pick, &watched.picked_by) {
		(None, _, None) => {
			eprintln!("audio only, nothing shown — {}", path.display());
			return None;
		}
		(None, Pick::Likely { .. }, Some(by)) => format!("nothing: `{by}` named no span where something is shown"),
		(Some(model), Pick::Changes, None) => format!("`{model}`, over {} frames, as `ask_llm`'s `Footage::{footage:?}` picks them", watched.frames_read),
		(Some(model), Pick::Likely { every }, Some(by)) => format!("`{model}`, over {} frames, `{by}` picking where, `Pick::Likely` every {every}s", watched.frames_read),
		_ => unreachable!("`picked_by` is set by `Pick::Likely` alone"),
	};
	let cost = watched.cost_cents as f64 / 100.;

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
		 - watched by: {watched_by} · `scripts/call-watch.rs`\n\
		 - watched: {}\n\
		 - cost: ${cost:.4}\n",
		capture.title,
		today.trim(),
	);
	for Shown { secs, shown, on_screen_text, .. } in &watched.shown {
		out.push_str(&format!("\n- [{}]({}{seek}{secs}) {}\n", stamp(*secs), capture.source, shown.replace('\n', " ")));
		if let Some(text) = on_screen_text {
			out.push_str(&format!("  > {}\n", text.replace('\n', " / ")));
		}
		out.push_str(&format!("  ![](frames/{secs}.jpg)\n"));
	}
	std::fs::write(kept.join("shown.md"), out).expect("the capture's dir is writable");
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
				let title = link.split_once(") ").map_or("", |(_, title)| title);
				chapters.push((t.parse().unwrap_or_else(|e| panic!("{}: `{line}`: {e}", path.display())), title.to_string(), String::new()));
			}
			None =>
				if let Some((_, _, body)) = chapters.last_mut() {
					body.push_str(line);
					body.push('\n');
				},
		}
	}
	assert!(chapters.windows(2).all(|w| w[0].0 < w[1].0), "{}: chapters out of order — run call-pull.rs --check", path.display());
	let summary = text
		.split_once("\n## summary\n")
		.map(|(_, rest)| rest.split("\n## ").next().expect("split yields at least once").trim().to_string())
		.filter(|s| !s.is_empty());
	Capture { title: title.to_string(), source: field("source").trim_matches(['<', '>']).to_string(), summary, chapters }
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
