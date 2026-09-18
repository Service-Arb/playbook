#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
serde_json = "1"
---

//! `./scripts/yt-pull.rs [<channel-or-video-url>...]` — write `ref/youtube/<id>.md` for every video
//! we do not have yet. With no arguments it takes the youtube links already written down in
//! `ref/README.md`, so adding a channel to the playbook is pasting its link there.
//!
//! yt-dlp does the talking: it is the only thing that tracks youtube's player, and the captions it
//! hands back are already timed. Nothing here downloads a video.
//!
//! Auto-captions arrive as a rolling two-line window — a few words per cue, re-sent as the window
//! scrolls — so cues are gathered into blocks of at least `BLOCK` seconds. A block is both a
//! readable paragraph and a `&t=` anchor that lands where the words are.

use std::{
	collections::BTreeSet,
	path::{Path, PathBuf},
	process::Command,
};

const WATCH: &str = "https://www.youtube.com/watch?v=";
const BLOCK: f64 = 30.;

fn main() {
	let root = repo_root();
	let out_dir = root.join("ref/youtube");
	std::fs::create_dir_all(&out_dir).expect("ref/youtube is ours to create");

	let args: Vec<String> = std::env::args().skip(1).collect();
	let sources: BTreeSet<String> = match args.is_empty() {
		true => links_in(&root.join("ref/README.md")),
		false => args.into_iter().collect(),
	};
	if sources.is_empty() {
		eprintln!("no youtube links in ref/README.md and none given — nothing to pull");
		return;
	}

	let mut ids = BTreeSet::new();
	for source in &sources {
		ids.extend(expand(source));
	}
	eprintln!("{} video(s) across {} source(s)", ids.len(), sources.len());

	let tmp = std::env::temp_dir().join("yt-pull");
	let mut mute = Vec::new();
	for id in &ids {
		let out = out_dir.join(format!("{id}.md"));
		if out.exists() {
			continue;
		}
		eprintln!("pulling {id}");
		// a video youtube never captioned is a real absence, not a broken run — the rest of the
		// channel is still worth having, so it is named at the end rather than aborting here
		match video(id, &tmp) {
			Some(doc) => {
				std::fs::write(&out, doc).expect("ref/youtube is ours to write");
				println!("{}", out.display());
			}
			None => mute.push(id.clone()),
		}
	}
	if !mute.is_empty() {
		eprintln!("no captions on youtube's side, nothing written: {}", mute.join(" "));
	}
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

/// The registry is `ref/README.md` and nothing else — a capture quotes its own source URL, and a
/// transcript quotes every link that was said out loud, so scanning the tree would make the
/// pullers feed on their own output.
fn links_in(readme: &Path) -> BTreeSet<String> {
	let text = std::fs::read_to_string(readme).unwrap_or_else(|e| panic!("reading {}: {e}", readme.display()));
	let mut found = BTreeSet::new();
	//LOOP: bounded by the number of occurrences in a finite file
	for (at, _) in text.match_indices("https://www.youtube.com/") {
		let link = text[at..].split_whitespace().next().expect("split yields at least once");
		let link = link.trim_end_matches([')', ']', ',', '.', '"', '`', '>']);
		if !link.is_empty() {
			found.insert(link.to_string());
		}
	}
	found
}

/// A channel or playlist is the ids under it; a watch link is itself. The flat listing is one
/// request and carries no per-video metadata, which is why it is only ever used for the ids.
fn expand(source: &str) -> Vec<String> {
	if let Some(id) = source.split("watch?v=").nth(1) {
		return vec![id.split('&').next().expect("split yields at least once").to_string()];
	}
	let out = yt_dlp(&["--flat-playlist", "--print", "%(id)s", source]);
	out.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

fn yt_dlp(args: &[&str]) -> String {
	let out = Command::new("yt-dlp")
		.arg("--no-update")
		.args(args)
		.output()
		.unwrap_or_else(|e| panic!("yt-dlp: {e} — is it on PATH?"));
	if !out.status.success() {
		panic!("yt-dlp {args:?} failed:\n{}", String::from_utf8_lossy(&out.stderr));
	}
	String::from_utf8(out.stdout).expect("yt-dlp prints utf-8")
}

fn hms(secs: f64) -> String {
	let s = secs as u64;
	format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

fn video(id: &str, tmp: &Path) -> Option<String> {
	// a stale caption file from an earlier run would be picked up as this one's
	let _ = std::fs::remove_dir_all(tmp);
	std::fs::create_dir_all(tmp).expect("the temp dir is ours to create");

	let meta = yt_dlp(&[
		"--skip-download",
		// `--print` alone implies `--simulate`, and a simulated run writes no caption file
		"--no-simulate",
		"--write-auto-subs",
		"--write-subs",
		"--sub-langs",
		"en.*",
		"--sub-format",
		"json3",
		"--print",
		"%(title)s\u{1f}%(upload_date)s\u{1f}%(duration)s\u{1f}%(channel)s",
		"-o",
		tmp.join("%(id)s").to_str().expect("the temp path is utf-8"),
		&format!("{WATCH}{id}"),
	]);
	let meta: Vec<&str> = meta.trim().split('\u{1f}').collect();
	assert_eq!(meta.len(), 4, "yt-dlp was asked for four fields on {id}, and answered {meta:?}");
	let (title, upload, duration, channel) = (meta[0], meta[1], meta[2], meta[3]);

	let captions = captions(tmp)?;
	let events = captions["events"].as_array().expect("a json3 caption track is a list of events");

	let mut out = format!(
		"# {title}\n\
		 \n\
		 - source: <{WATCH}{id}>\n\
		 - channel: {channel}\n\
		 - uploaded: {}\n\
		 - duration: {}\n\
		 - pulled by: `scripts/yt-pull.rs`\n\
		 \n",
		dashed(upload),
		hms(duration.parse::<f64>().unwrap_or_else(|e| panic!("yt-dlp stated {id}'s duration as {duration:?}: {e}")))
	);

	let mut start = None;
	let mut words = String::new();
	//LOOP: one pass over a finite caption track
	for event in events {
		let Some(segs) = event["segs"].as_array() else { continue };
		let text: String = segs.iter().filter_map(|s| s["utf8"].as_str()).collect();
		// the rolling window re-sends the newline between its two lines as a cue of its own
		if text.trim().is_empty() {
			continue;
		}
		let at = event["tStartMs"].as_f64().expect("a json3 event is stamped") / 1000.;
		let block = *start.get_or_insert(at);
		if !words.is_empty() {
			words.push(' ');
		}
		words.push_str(text.trim());
		if at - block >= BLOCK {
			out.push_str(&paragraph(id, block, &words));
			start = None;
			words.clear();
		}
	}
	if !words.is_empty() {
		out.push_str(&paragraph(id, start.expect("words are only pushed after a block starts"), &words));
	}
	Some(out)
}

fn paragraph(id: &str, at: f64, words: &str) -> String {
	// the whole link on every block, so a line stays citable once it is copied out of this file
	format!("[{}]({WATCH}{id}&t={}) {words}\n\n", hms(at), at as u64)
}

fn dashed(upload: &str) -> String {
	assert_eq!(upload.len(), 8, "yt-dlp states an upload date as YYYYMMDD, and answered {upload:?}");
	format!("{}-{}-{}", &upload[..4], &upload[4..6], &upload[6..])
}

/// yt-dlp names the track by the language it found, and asks for both the uploader's and youtube's
/// own. A hand-written track is the better read, and sorting puts its shorter name first.
fn captions(tmp: &Path) -> Option<serde_json::Value> {
	let mut tracks: Vec<PathBuf> = std::fs::read_dir(tmp)
		.expect("the temp dir was just created")
		.map(|e| e.expect("a readable dir yields readable entries").path())
		.filter(|p| p.extension().is_some_and(|e| e == "json3"))
		.collect();
	tracks.sort();
	let track = tracks.first()?;
	Some(serde_json::from_str(&std::fs::read_to_string(track).expect("yt-dlp just wrote it")).expect("yt-dlp writes json3 as json"))
}
