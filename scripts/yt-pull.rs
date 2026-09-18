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
//! hands back are already timed.
//!
//! Auto-captions arrive as a rolling two-line window — a few words per cue, re-sent as the window
//! scrolls — so cues are gathered into blocks of at least `BLOCK` seconds. A block is both a
//! readable paragraph and a `&t=` anchor that lands where the words are.
//!
//! Half of what these videos say is said on screen — a dashboard, a search result, a review count —
//! and captions carry none of it, so a capture also holds chapters, a frame per chapter, a summary
//! and the description. Chapters are the uploader's own where youtube has them and the model's
//! reading of the transcript where it does not; the frames come off the video itself.
//!
//! Enrichment that cannot be produced aborts the video and writes nothing. The md file existing is
//! what makes the next run skip it, so a half-written one would never be repaired.

use std::{
	collections::BTreeSet,
	path::{Path, PathBuf},
	process::Command,
};

const WATCH: &str = "https://www.youtube.com/watch?v=";
const BLOCK: f64 = 30.;
/// Into a chapter rather than onto its first frame, which is still the transition out of the last.
const SHOT_INTO: f64 = 8.;
const SUMMARY: &str = "SUMMARY";
const SECTIONS: &str = "SECTIONS";

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
		match video(id, &tmp, &out_dir.join(id)) {
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

fn video(id: &str, tmp: &Path, shots: &Path) -> Option<String> {
	// a stale caption file or video from an earlier run would be picked up as this one's
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
		"%(title)s\u{1f}%(upload_date)s\u{1f}%(duration)s\u{1f}%(channel)s\u{1f}%(chapters)j\u{1f}%(description)j",
		"-o",
		tmp.join("%(id)s").to_str().expect("the temp path is utf-8"),
		&format!("{WATCH}{id}"),
	]);
	let meta: Vec<&str> = meta.trim().split('\u{1f}').collect();
	assert_eq!(meta.len(), 6, "yt-dlp was asked for six fields on {id}, and answered {meta:?}");
	let (title, upload, duration, channel) = (meta[0], meta[1], meta[2], meta[3]);
	let duration: f64 = duration.parse().unwrap_or_else(|e| panic!("yt-dlp stated {id}'s duration as {duration:?}: {e}"));

	let captions = captions(tmp)?;
	let blocks = blocks(&captions);
	assert!(!blocks.is_empty(), "{id} has a caption track that carries no words");

	let chapters = chapters(meta[4], id);
	let answer = ask(&prompt(title, &blocks, chapters.is_none()));
	let sections = chapters.unwrap_or_else(|| derived(&answer, id, duration));
	let shot_at = frames(id, tmp, shots, &sections);

	let mut out = format!(
		"# {title}\n\
		 \n\
		 - source: <{WATCH}{id}>\n\
		 - channel: {channel}\n\
		 - uploaded: {}\n\
		 - duration: {}\n\
		 - pulled by: `scripts/yt-pull.rs`\n\
		 \n\
		 ## summary\n\
		 \n",
		dashed(upload),
		hms(duration)
	);
	for line in marked(&answer, SUMMARY, id) {
		out.push_str(line);
		out.push('\n');
	}

	out.push_str("\n## chapters\n\n");
	for ((at, heading), shot) in sections.iter().zip(&shot_at) {
		out.push_str(&format!("- [{}]({WATCH}{id}&t={}) {heading}\n  ![]({id}/{shot}.jpg)\n", hms(*at), *at as u64));
	}

	// `NA` is youtube's answer for a video whose uploader wrote nothing under it
	let description: String = match meta[5].trim() {
		"NA" => String::new(),
		field => serde_json::from_str(field).unwrap_or_else(|e| panic!("yt-dlp states a description as a json string, and answered {field:?}: {e}")),
	};
	if !description.trim().is_empty() {
		out.push_str(&format!("\n## description\n\n```\n{}\n```\n", description.trim()));
		let links = urls_in(&description);
		if !links.is_empty() {
			out.push('\n');
			for link in &links {
				out.push_str(&format!("- <{link}>\n"));
			}
		}
	}

	out.push_str("\n## transcript\n\n");
	for (at, words) in &blocks {
		// the whole link on every block, so a line stays citable once it is copied out of this file
		out.push_str(&format!("[{}]({WATCH}{id}&t={}) {words}\n\n", hms(*at), *at as u64));
	}
	let _ = std::fs::remove_dir_all(tmp);
	Some(out)
}

fn dashed(upload: &str) -> String {
	assert_eq!(upload.len(), 8, "yt-dlp states an upload date as YYYYMMDD, and answered {upload:?}");
	format!("{}-{}-{}", &upload[..4], &upload[4..6], &upload[6..])
}

fn urls_in(text: &str) -> BTreeSet<String> {
	text.split_whitespace()
		.map(|t| t.trim_end_matches([')', ']', ',', '.', '"', '`', '>']))
		.filter(|t| t.starts_with("http://") || t.starts_with("https://"))
		.map(str::to_string)
		.collect()
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

fn blocks(captions: &serde_json::Value) -> Vec<(f64, String)> {
	let events = captions["events"].as_array().expect("a json3 caption track is a list of events");
	let mut out = Vec::new();
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
			out.push((block, std::mem::take(&mut words)));
			start = None;
		}
	}
	if !words.is_empty() {
		out.push((start.expect("words are only pushed after a block starts"), words));
	}
	out
}

/// `NA` is youtube's answer for a video the uploader never chaptered, which is most of them.
fn chapters(field: &str, id: &str) -> Option<Vec<(f64, String)>> {
	if field.trim() == "NA" {
		return None;
	}
	let parsed: serde_json::Value = serde_json::from_str(field).unwrap_or_else(|e| panic!("yt-dlp states {id}'s chapters as json, and answered {field:?}: {e}"));
	let list = parsed.as_array().expect("yt-dlp states chapters as a list");
	let out: Vec<(f64, String)> = list
		.iter()
		.map(|c| {
			(
				c["start_time"].as_f64().expect("a youtube chapter is stamped"),
				c["title"].as_str().expect("a youtube chapter is titled").to_string(),
			)
		})
		.collect();
	assert!(!out.is_empty(), "{id} carries a chapter list with nothing in it");
	Some(out)
}

fn prompt(title: &str, blocks: &[(f64, String)], want_sections: bool) -> String {
	let mut ask = format!(
		"Below is the transcript of a youtube video titled {title:?}, one paragraph per {} seconds, \
		 each stamped with the second it starts at.\n\n\
		 Answer with the block(s) below and nothing else — no preamble, no closing line, no code fences. \
		 Open every block with its own name alone on a line, even when only one is asked for.\n\n\
		 {SUMMARY}\n\
		 Three to five lines, each opening with `- `, saying what this video actually covers. Name the \
		 tools, the numbers and the steps it gives. Write about the subject, not about the video.\n",
		BLOCK as u64
	);
	if want_sections {
		ask.push_str(&format!(
			"\n{SECTIONS}\n\
			 One line per section, `<seconds> <heading>`, seconds being a whole-number offset into the \
			 video and heading a handful of lowercase words. Six to twelve sections, in order. Open a \
			 section where the speaker turns to a new thing, so a frame grabbed a few seconds in shows \
			 whatever is on screen for it.\n"
		));
	}
	ask.push_str("\n--- transcript ---\n");
	for (at, words) in blocks {
		ask.push_str(&format!("{} {words}\n", *at as u64));
	}
	ask
}

/// Reached through the `claude` CLI the way `ask_llm`'s claude backend does, so the answer bills the
/// subscription rather than credits — which is also why the keys that would redirect it are dropped.
fn ask(prompt: &str) -> String {
	let out = Command::new("claude")
		.args(["-p", prompt])
		.args(["--model", "haiku"])
		.args(["--tools", ""]) // an answer, not an agent
		.arg("--safe-mode") // this checkout's CLAUDE.md, hooks and MCP servers are not part of the question
		.arg("--no-session-persistence")
		.env_remove("ANTHROPIC_API_KEY")
		.env_remove("CLAUDE_TOKEN")
		.output()
		.unwrap_or_else(|e| panic!("claude: {e} — is it on PATH?"));
	if !out.status.success() {
		panic!("claude exited {}:\n{}", out.status, String::from_utf8_lossy(&out.stderr));
	}
	String::from_utf8(out.stdout).expect("claude prints utf-8")
}

/// The lines of one named block of the answer. A missing block is the model having answered some
/// other shape, and everything below it would be built out of the wrong text.
fn marked<'a>(answer: &'a str, mark: &str, id: &str) -> Vec<&'a str> {
	let mut lines = answer.lines().skip_while(|l| l.trim() != mark);
	lines
		.next()
		.unwrap_or_else(|| panic!("the model was asked for a `{mark}` block on {id}, and answered:\n{answer}"));
	let block: Vec<&str> = lines
		.map(str::trim)
		.take_while(|l| !matches!(*l, SUMMARY | SECTIONS))
		.filter(|l| !l.is_empty())
		.collect();
	assert!(!block.is_empty(), "the `{mark}` block on {id} is empty:\n{answer}");
	block
}

fn derived(answer: &str, id: &str, duration: f64) -> Vec<(f64, String)> {
	let sections: Vec<(f64, String)> = marked(answer, SECTIONS, id)
		.iter()
		.map(|line| {
			let (at, heading) = line.split_once(' ').unwrap_or_else(|| panic!("a `{SECTIONS}` line on {id} is `<seconds> <heading>`, and reads {line:?}"));
			let at: f64 = at.parse().unwrap_or_else(|e| panic!("a `{SECTIONS}` line on {id} opens with a whole number of seconds, and reads {line:?}: {e}"));
			assert!(at < duration, "the model placed a section of {id} at {at}s, past its {duration}s");
			(at, heading.trim().to_string())
		})
		.collect();
	assert!(sections.windows(2).all(|w| w[0].0 < w[1].0), "the model's sections for {id} do not run in order: {sections:?}");
	sections
}

/// One capped pull per video: youtube binds a media URL to the player client that asked for it, so
/// ffmpeg seeking that URL answers 403 and the frames have to come off a local file, which is then
/// dropped with the rest of the temp dir. The seconds it ends up grabbing at are what names the
/// files, so they are handed back rather than recomputed where the markdown points at them.
fn frames(id: &str, tmp: &Path, shots: &Path, sections: &[(f64, String)]) -> Vec<u64> {
	let video = tmp.join("video");
	yt_dlp(&[
		// the default client hands back URLs that 403 on download, and the mobile ones are offered
		// nothing above 360p, at which a dashboard in a screen-share stops being readable
		"--extractor-args",
		"youtube:player_client=web_embedded",
		// the floor is the point of the frames and the ceiling is what keeps the pull cheap; a video
		// offering neither is better refused here than captured with nothing legible in it
		"-f",
		"bv*[height<=720][height>=480]",
		"--no-part",
		"-q",
		"-o",
		video.to_str().expect("the temp path is utf-8"),
		&format!("{WATCH}{id}"),
	]);
	let duration = ffprobe(&video);
	std::fs::create_dir_all(shots).expect("ref/youtube is ours to create under");
	let mut grabbed = Vec::new();
	for (at, _) in sections {
		let at = (at + SHOT_INTO).min(duration - 1.) as u64;
		let shot = shots.join(format!("{at}.jpg"));
		let status = Command::new("ffmpeg")
			.args(["-nostdin", "-loglevel", "error", "-y", "-ss"])
			.arg(at.to_string())
			.arg("-i")
			.arg(&video)
			.args(["-frames:v", "1", "-q:v", "4"])
			.arg(&shot)
			.status()
			.unwrap_or_else(|e| panic!("ffmpeg: {e} — is it on PATH?"));
		assert!(status.success() && shot.exists(), "no frame at {at}s of {id}");
		grabbed.push(at);
	}
	grabbed
}

/// The stream's own length, not youtube's: `-ss` past the last frame writes nothing, and the two
/// disagree by a second often enough to matter on the closing section.
fn ffprobe(video: &Path) -> f64 {
	let out = Command::new("ffprobe")
		.args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"])
		.arg(video)
		.output()
		.unwrap_or_else(|e| panic!("ffprobe: {e} — is it on PATH?"));
	assert!(out.status.success(), "ffprobe {}:\n{}", video.display(), String::from_utf8_lossy(&out.stderr));
	let text = String::from_utf8(out.stdout).expect("ffprobe prints utf-8");
	text.trim().parse().unwrap_or_else(|e| panic!("ffprobe stated a duration of {text:?}: {e}"))
}
