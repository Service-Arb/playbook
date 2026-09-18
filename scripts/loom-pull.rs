#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
serde_json = "1"
ureq = "3"
---

//! `./scripts/loom-pull.rs [<share-url-or-id>...]` — write `ref/loom/<date>-<title>.md` for every
//! loom recording we do not have yet. With no arguments it takes the links already written down in
//! `ref/README.md`, so adding a recording to the playbook is pasting its link there.
//!
//! A loom share page is server-rendered and carries both the video's metadata and a *signed* URL
//! for the transcript CDN, so nothing here needs a loom account or a download of the video. The
//! signature expires, which is why the URL is read out of the page on every run rather than kept.
//!
//! The page also carries what loom's own AI made of the recording — a summary and a list of
//! chapters — and those are the two things that make a wall of transcript answerable without
//! reading it, so they are written above it.
//!
//! The transcript is emitted one timestamped line per phrase, because `?t=<seconds>` on the share
//! URL is what makes a note traceable back to the second it came from.
//!
//! Loom does not do any of the three for every recording. What it skipped is named in the capture's
//! header and left empty, because a capture that states what is missing is a capture `/loom-digest`
//! can be pointed at — and a puller that guessed at it would be writing a reading.

use std::{
	collections::{BTreeMap, BTreeSet},
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
	let have = captured(&out_dir);

	let mut undigested = Vec::new();
	for id in &wanted {
		if let Some(had) = have.get(id) {
			eprintln!("have {id} — {}", had.display());
			continue;
		}
		eprintln!("pulling {id}");
		let page = get(&format!("{SHARE}{id}"));
		let transcript = signed_transcript_url(&page, id).map(|url| get(&url));
		let (name, doc, digested) = render(id, &page, transcript.as_deref());
		let out = out_dir.join(&name);
		assert!(!out.exists(), "{} is taken, and not by {id} — two recordings share a day and a title", out.display());
		std::fs::write(&out, doc).expect("ref/loom is ours to write");
		println!("{}", out.display());
		if !digested {
			undigested.push(out);
		}
	}

	if !undigested.is_empty() {
		eprintln!("\nloom left these unread — run `/loom-digest` over each:");
		for out in &undigested {
			eprintln!("  {}", out.display());
		}
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

/// Every capture already on disk, by the id it states. A file is named after the recording rather
/// than after its id, so what is on disk is a question for the files and not for their names.
fn captured(dir: &Path) -> BTreeMap<String, PathBuf> {
	let mut found = BTreeMap::new();
	//LOOP: bounded by the number of files in a finite directory
	for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
		let path = entry.expect("a directory entry is readable").path();
		let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
		let source = text
			.lines()
			.find_map(|l| l.strip_prefix("- source: <"))
			.unwrap_or_else(|| panic!("{} states no source — it was not written by this script", path.display()));
		let id = id_of(source.trim_end_matches('>'));
		if let Some(clash) = found.insert(id.clone(), path.clone()) {
			panic!("{} and {} both claim {id}", clash.display(), path.display());
		}
	}
	found
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

/// What loom's AI wrote, read out of the recording's own node in the apollo state the page ships —
/// `description` there is the summary, and loom's marketing copy under the same name further down
/// the page is what the anchor exists to skip. Both keys are `null` for a recording loom never got
/// to, which is a state and not a failure.
fn ai(page: &str, id: &str, key: &str) -> Option<String> {
	let node = page
		.find(&format!("\"RegularUserVideo:{id}\""))
		.unwrap_or_else(|| panic!("{id} has no node of its own on its own page — loom's payload moved, re-read the page"));
	let at = node + page[node..].find(&format!("\"{key}\":\""))? + key.len() + 4;
	let rest = &page[at..];
	let mut out = String::new();
	let mut chars = rest.chars();
	//LOOP: bounded by the length of a finite page
	while let Some(c) = chars.next() {
		match c {
			'"' => return Some(out).filter(|v: &String| !v.trim().is_empty()),
			// the value sits inside a JSON string literal, and loom writes its newlines escaped
			'\\' => match chars.next().expect("a JSON escape carries what it escapes") {
				'n' => out.push('\n'),
				'"' => out.push('"'),
				'\\' => out.push('\\'),
				'/' => out.push('/'),
				'u' => {
					let hex: String = chars.by_ref().take(4).collect();
					let code = u32::from_str_radix(&hex, 16).unwrap_or_else(|e| panic!("loom wrote `\\u{hex}` on the page: {e}"));
					out.push(char::from_u32(code).unwrap_or_else(|| panic!("loom wrote `\\u{hex}`, which is not a character")));
				}
				other => panic!("loom wrote an escape this does not read: `\\{other}`"),
			},
			c => out.push(c),
		}
	}
	panic!("`{key}` never closes on the loom page")
}

/// `None` for a recording loom never transcribed, which is a state and not a failure.
fn signed_transcript_url(page: &str, id: &str) -> Option<String> {
	let at = page.find(TRANSCRIPT_CDN)?;
	assert!(page.contains(id), "{id} is not the page it was asked for");
	// the URL sits inside a JSON string literal, so it ends at the quote or at the escape before one
	Some(page[at..].split(['"', '\\']).next().expect("split yields at least once").to_string())
}

fn hms(secs: f64) -> String {
	let s = secs as u64;
	format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// `MM:SS` or `H:MM:SS`, which is how loom writes a chapter's start.
fn secs(stamp: &str) -> u64 {
	stamp.split(':').fold(0, |acc, part| {
		acc * 60 + part.parse::<u64>().unwrap_or_else(|e| panic!("loom wrote `{stamp}` as a chapter's start: {e}"))
	})
}

/// A filename that says what the recording is. The id stays in the file, so this one is free to
/// read like the title it came from, and the date in front of it is what keeps two calls of the
/// same name apart.
fn slug(title: &str) -> String {
	let mut out = String::new();
	for c in title.chars() {
		match c {
			c if c.is_ascii_alphanumeric() => out.push(c.to_ascii_lowercase()),
			_ if out.ends_with('-') => (),
			_ => out.push('-'),
		}
	}
	out.trim_matches('-').to_string()
}

fn phrases_of(transcript: &str) -> Vec<serde_json::Value> {
	let parsed: serde_json::Value = serde_json::from_str(transcript).expect("the transcript CDN answers json");
	parsed["phrases"].as_array().expect("a loom transcript is an array of phrases").clone()
}

/// The capture, its filename, and whether loom had already made sense of it.
fn render(id: &str, page: &str, transcript: Option<&str>) -> (String, String, bool) {
	let title = between(page, "<title>", '<', "the recording's title");
	let title = title.strip_suffix(" | Loom").unwrap_or(&title);
	let recorded = between(page, r#""uploadDate": ""#, '"', "the recording's date");
	let duration: f64 = between(page, r#""durationMs":"#, ',', "the recording's duration")
		.parse::<f64>()
		.expect("loom states durationMs as a number")
		/ 1000.;
	let summary = ai(page, id, "description");
	let chapters = ai(page, id, "chapters");
	let digested = summary.is_some() && chapters.is_some() && transcript.is_some();

	let mut out = format!(
		"# {title}\n\
		 \n\
		 - source: <{SHARE}{id}>\n\
		 - recorded: {recorded}\n\
		 - duration: {}\n\
		 - read by: {}\n\
		 - pulled by: `scripts/loom-pull.rs`\n\
		 \n",
		hms(duration),
		match digested {
			true => "loom".to_string(),
			false => format!(
				"nothing yet — loom wrote no {}, and `/loom-digest` writes them here",
				[("transcript", transcript.is_none()), ("summary", summary.is_none()), ("chapters", chapters.is_none())]
					.iter()
					.filter(|(_, missing)| *missing)
					.map(|(what, _)| *what)
					.collect::<Vec<_>>()
					.join(" and no ")
			),
		},
	);
	if let Some(summary) = &summary {
		out.push_str(&format!("## summary\n\n{}\n\n", summary.trim()));
	}
	if let Some(chapters) = &chapters {
		out.push_str("## chapters\n\n");
		for chapter in chapters.lines().filter(|l| !l.trim().is_empty()) {
			let (stamp, name) = chapter.trim().split_once(' ').unwrap_or_else(|| panic!("loom wrote `{chapter}` as a chapter"));
			out.push_str(&format!("- [{stamp}]({SHARE}{id}?t={}) {name}\n", secs(stamp)));
		}
		out.push('\n');
	}
	out.push_str("## transcript\n\n");
	for phrase in transcript.iter().flat_map(|t| phrases_of(t)) {
		let ts = phrase["ts"].as_f64().expect("a loom phrase is stamped");
		let value = phrase["value"].as_str().expect("a loom phrase carries text");
		// the whole link on every line, so a line stays citable once it is copied out of this file
		out.push_str(&format!("[{}]({SHARE}{id}?t={}) {value}\n\n", hms(ts), ts as u64));
	}
	(format!("{}-{}.md", &recorded[..10], slug(title)), out, digested)
}
