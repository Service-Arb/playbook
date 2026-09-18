#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
serde_json = "1"
ureq = "3"
---

//! `./scripts/loom-pull.rs [<share-url-or-id>...]` — write `ref/loom/<id>.md` for every loom
//! recording we do not have yet. With no arguments it takes the links already written down in
//! `ref/README.md`, so adding a recording to the playbook is pasting its link there.
//!
//! A loom share page is server-rendered and carries both the video's metadata and a *signed* URL
//! for the transcript CDN, so nothing here needs a loom account or a download of the video. The
//! signature expires, which is why the URL is read out of the page on every run rather than kept.
//!
//! The transcript is emitted one timestamped line per phrase, because `?t=<seconds>` on the share
//! URL is what makes a note traceable back to the second it came from.

use std::{
	collections::BTreeSet,
	path::{Path, PathBuf},
};

const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const SHARE: &str = "https://www.loom.com/share/";
const TRANSCRIPT_CDN: &str = "https://cdn.loom.com/mediametadata/transcription/";

fn main() {
	let root = repo_root();
	let out_dir = root.join("ref/loom");
	std::fs::create_dir_all(&out_dir).expect("ref/loom is ours to create");

	let args: Vec<String> = std::env::args().skip(1).collect();
	let wanted: BTreeSet<String> = match args.is_empty() {
		true => links_in(&root.join("ref/README.md")),
		false => args.iter().map(|a| id_of(a)).collect(),
	};
	if wanted.is_empty() {
		eprintln!("no loom links in ref/README.md and none given — nothing to pull");
		return;
	}

	for id in &wanted {
		let out = out_dir.join(format!("{id}.md"));
		if out.exists() {
			eprintln!("have {id}");
			continue;
		}
		eprintln!("pulling {id}");
		let page = get(&format!("{SHARE}{id}"));
		let doc = render(id, &page, &get(&signed_transcript_url(&page, id)));
		std::fs::write(&out, doc).expect("ref/loom is ours to write");
		println!("{}", out.display());
	}
}

/// The script is only ever run from a checkout, and every path it touches is relative to the root
/// of one — so finding it is the first thing, and failing to is not something to guess past.
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

fn id_of(arg: &str) -> String {
	let tail = arg.rsplit('/').next().expect("rsplit yields at least once");
	tail.split(['?', '#']).next().expect("split yields at least once").to_string()
}

/// The registry is `ref/README.md` and nothing else — a capture quotes its own source URL, and a
/// transcript quotes every link that was said out loud, so scanning the tree would make the
/// pullers feed on their own output.
fn links_in(readme: &Path) -> BTreeSet<String> {
	let text = std::fs::read_to_string(readme).unwrap_or_else(|e| panic!("reading {}: {e}", readme.display()));
	let mut found = BTreeSet::new();
	//LOOP: bounded by the number of occurrences in a finite file
	for (at, _) in text.match_indices(SHARE) {
		let link = text[at..].split_whitespace().next().expect("split yields at least once");
		let link = link.trim_end_matches([')', ']', ',', '.', '"', '`', '>']);
		if !link.is_empty() {
			found.insert(id_of(link));
		}
	}
	found
}

fn get(url: &str) -> String {
	ureq::get(url)
		.header("User-Agent", UA)
		.call()
		.unwrap_or_else(|e| panic!("GET {}: {e}", &url[..url.len().min(80)]))
		.body_mut()
		.with_config()
		// the word-level transcript of a two-hour call runs to several MB, well past ureq's default
		.limit(128 * 1024 * 1024)
		.read_to_string()
		.expect("loom answers in utf-8")
}

/// Cut out the value that starts at `from` and runs to the next `end`. Absence is a shape change on
/// loom's side, and a transcript assembled past one would be silently wrong.
fn between(page: &str, from: &str, end: char, what: &str) -> String {
	let at = page.find(from).unwrap_or_else(|| panic!("no `{from}` on the loom page — {what} moved, re-read the page")) + from.len();
	let rest = &page[at..];
	let to = rest.find(end).unwrap_or_else(|| panic!("`{from}` never closes on the loom page — {what} moved, re-read the page"));
	rest[..to].to_string()
}

fn signed_transcript_url(page: &str, id: &str) -> String {
	let at = page
		.find(TRANSCRIPT_CDN)
		.unwrap_or_else(|| panic!("{id} carries no transcript URL — loom has most likely not transcribed it yet"));
	// the URL sits inside a JSON string literal, so it ends at the quote or at the escape before one
	page[at..].split(['"', '\\']).next().expect("split yields at least once").to_string()
}

fn hms(secs: f64) -> String {
	let s = secs as u64;
	format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

fn render(id: &str, page: &str, transcript: &str) -> String {
	let title = between(page, "<title>", '<', "the recording's title");
	let title = title.strip_suffix(" | Loom").unwrap_or(&title);
	let recorded = between(page, r#""uploadDate": ""#, '"', "the recording's date");
	let duration: f64 = between(page, r#""durationMs":"#, ',', "the recording's duration")
		.parse::<f64>()
		.expect("loom states durationMs as a number")
		/ 1000.;

	let parsed: serde_json::Value = serde_json::from_str(transcript).expect("the transcript CDN answers json");
	let phrases = parsed["phrases"].as_array().expect("a loom transcript is an array of phrases");

	let mut out = format!(
		"# {title}\n\
		 \n\
		 - source: <{SHARE}{id}>\n\
		 - recorded: {recorded}\n\
		 - duration: {}\n\
		 - pulled by: `scripts/loom-pull.rs`\n\
		 \n",
		hms(duration)
	);
	for phrase in phrases {
		let ts = phrase["ts"].as_f64().expect("a loom phrase is stamped");
		let value = phrase["value"].as_str().expect("a loom phrase carries text");
		// the whole link on every line, so a line stays citable once it is copied out of this file
		out.push_str(&format!("[{}]({SHARE}{id}?t={}) {value}\n\n", hms(ts), ts as u64));
	}
	out
}
