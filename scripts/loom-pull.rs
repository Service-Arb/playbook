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
//! `./scripts/loom-pull.rs --check` only holds every capture on disk to the shape below, and fails
//! loudly on the first that breaks it.
//!
//! The shape is `docs/ARCHITECTURE.md`'s "Loom captures"; this script is its only enforcement.
//!
//! A loom share page is server-rendered and carries both the video's metadata and a *signed* URL
//! for the transcript CDN, so nothing here needs a loom account. The signature expires, which is
//! why the URL is read out of the page on every run rather than kept.
//!
//! Loom's transcript is taken only when it covers the recording; loom routinely transcribes the
//! first minutes of a long call and stops, and a capture silently missing forty minutes is worse
//! than one that took a while to pull. Anything short is transcribed here with whisper instead.

use std::{
	collections::{BTreeMap, BTreeSet},
	path::{Path, PathBuf},
	process::Command,
};

const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const SHARE: &str = "https://www.loom.com/share/";
const TRANSCRIPT_CDN: &str = "https://cdn.loom.com/mediametadata/transcription/";
const WHISPER_MODEL: &str = ".local/share/whisper-cpp/models/ggml-base.en.bin";
/// A transcript whose last phrase starts before this share of the recording is taken to have stopped.
const COVERED: f64 = 0.95;
/// A paragraph is closed at the first sentence end past this many words.
const PARAGRAPH_WORDS: usize = 80;

fn main() {
	let root = repo_root();
	let out_dir = root.join("ref/loom");
	std::fs::create_dir_all(&out_dir).expect("ref/loom is ours to create");

	let args: Vec<String> = std::env::args().skip(1).collect();
	let have = captured(&out_dir);
	if args.iter().any(|a| a == "--check") {
		assert_eq!(args.len(), 1, "--check takes nothing else");
		eprintln!("{} loom captures hold their shape", have.len());
		return;
	}
	let wanted: BTreeSet<String> = match args.is_empty() {
		true => links_in(&root.join("ref/README.md")),
		false => args.iter().map(|a| id_of(a)).collect(),
	};
	if wanted.is_empty() {
		eprintln!("no loom links in ref/README.md and none given — nothing to pull");
		return;
	}

	let mut undigested = Vec::new();
	for id in &wanted {
		if let Some(had) = have.get(id) {
			eprintln!("have {id} — {}", had.display());
			continue;
		}
		eprintln!("pulling {id}");
		let page = get(&format!("{SHARE}{id}"));
		let (name, doc, digested) = render(id, &page);
		check(&name, &doc);
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

/// Every capture already on disk, by the id it states, each held to the shape on the way. A file is
/// named after the recording rather than after its id, so what is on disk is a question for the
/// files and not for their names.
fn captured(dir: &Path) -> BTreeMap<String, PathBuf> {
	let mut found = BTreeMap::new();
	//LOOP: bounded by the number of files in a finite directory
	for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
		let path = entry.expect("a directory entry is readable").path();
		let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
		let id = check(&path.display().to_string(), &text);
		if let Some(clash) = found.insert(id.clone(), path.clone()) {
			panic!("{} and {} both claim {id}", clash.display(), path.display());
		}
	}
	found
}

/// Hold a capture to `docs/ARCHITECTURE.md`'s "Loom captures", returning the id it states.
fn check(name: &str, text: &str) -> String {
	let field = |key: &str| {
		text.lines()
			.find_map(|l| l.strip_prefix(&format!("- {key}: ")))
			.unwrap_or_else(|| panic!("{name} states no `{key}:` — it was not written by this script"))
	};
	let source = field("source");
	let id = id_of(source.trim_start_matches('<').trim_end_matches('>'));
	let duration = secs(field("duration"));
	field("transcribed by");
	let unchaptered = field("read by").contains("no chapters");

	let sections: Vec<&str> = text.lines().filter(|l| l.starts_with("## ")).collect();
	assert_eq!(sections.last(), Some(&"## transcript"), "{name}: `## transcript` has to be its last section");
	assert!(!sections.contains(&"## chapters"), "{name}: chapters are `###` headers inside `## transcript`, not a section");
	let transcript = text.split_once("\n## transcript\n").expect("found as a section above").1;

	let link = format!("]({SHARE}{id}?t=");
	let mut last = None;
	let mut first = true;
	//LOOP: bounded by the lines of a finite file
	for line in transcript.lines().filter(|l| !l.trim().is_empty()) {
		let Some(header) = line.strip_prefix("### [") else {
			assert!(!first, "{name}: transcript text before the first chapter header");
			assert!(!line.contains(&link), "{name}: only chapter headers carry a timestamp — `{line}`");
			continue;
		};
		first = false;
		let (stamp, rest) = header.split_once(&link).unwrap_or_else(|| panic!("{name}: `{line}` is not `### [MM:SS]({SHARE}{id}?t=<secs>) <title>`"));
		let (t, title) = rest.split_once(')').unwrap_or_else(|| panic!("{name}: `{line}` never closes its link"));
		let t: u64 = t.parse().unwrap_or_else(|e| panic!("{name}: `{line}` links to a second that is not one: {e}"));
		assert_eq!(secs(stamp), t, "{name}: `{line}` says one time and links to another");
		assert!(t <= duration, "{name}: `{line}` is past the end of the recording");
		assert!(last < Some(t), "{name}: `{line}` does not come after the chapter before it");
		assert!(unchaptered || !title.trim().is_empty(), "{name}: `{line}` has no title, and the capture says it is chaptered");
		last = Some(t);
	}
	assert!(!first, "{name}: the transcript is empty");
	id
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

/// Loom's phrases, `(start secs, text)`. `None` for a recording loom never transcribed, which is a
/// state and not a failure.
fn loom_phrases(page: &str, id: &str) -> Option<Vec<(f64, String)>> {
	let at = page.find(TRANSCRIPT_CDN)?;
	assert!(page.contains(id), "{id} is not the page it was asked for");
	// the URL sits inside a JSON string literal, so it ends at the quote or at the escape before one
	let url = page[at..].split(['"', '\\']).next().expect("split yields at least once");
	let parsed: serde_json::Value = serde_json::from_str(&get(url)).expect("the transcript CDN answers json");
	let phrases = parsed["phrases"].as_array().expect("a loom transcript is an array of phrases");
	Some(
		phrases
			.iter()
			.map(|p| {
				let ts = p["ts"].as_f64().expect("a loom phrase is stamped");
				(ts, p["value"].as_str().expect("a loom phrase carries text").to_string())
			})
			.collect(),
	)
}

/// The recording's audio through whisper, `(start secs, text)` per segment.
fn whisper_phrases(id: &str) -> Vec<(f64, String)> {
	let model = PathBuf::from(std::env::var("HOME").expect("a user session has HOME")).join(WHISPER_MODEL);
	assert!(model.exists(), "{} is missing — whisper needs a model to transcribe {id}", model.display());
	let dir = std::env::temp_dir().join(format!("loom-pull-{id}"));
	std::fs::create_dir_all(&dir).expect("the temp dir is writable");
	let wav = dir.join("audio.wav");
	let base = dir.join("audio");
	let run = |cmd: &mut Command| {
		let status = cmd.status().unwrap_or_else(|e| panic!("running {cmd:?}: {e}"));
		assert!(status.success(), "{cmd:?} failed: {status}");
	};
	if !wav.exists() {
		run(Command::new("yt-dlp")
			.args(["-q", "-x", "--audio-format", "wav", "--postprocessor-args", "ExtractAudio:-ar 16000 -ac 1", "-o"])
			.arg(dir.join("audio.%(ext)s"))
			.arg(format!("{SHARE}{id}")));
	}
	eprintln!("  transcribing {id} with whisper — minutes per hour of audio");
	run(Command::new("whisper-cli").arg("-m").arg(&model).arg("-f").arg(&wav).args(["-np", "-oj", "-of"]).arg(&base));
	let json = std::fs::read_to_string(base.with_extension("json")).expect("whisper-cli -oj writes <of>.json");
	let parsed: serde_json::Value = serde_json::from_str(&json).expect("whisper-cli writes json");
	let segments = parsed["transcription"].as_array().expect("whisper json carries `transcription`");
	let phrases = segments
		.iter()
		.map(|s| {
			let ms = s["offsets"]["from"].as_f64().expect("a whisper segment carries its offset");
			(ms / 1000., s["text"].as_str().expect("a whisper segment carries text").trim().to_string())
		})
		.filter(|(_, t)| !t.is_empty())
		.collect();
	std::fs::remove_dir_all(&dir).expect("the temp dir is ours");
	phrases
}

fn hms(secs: f64) -> String {
	let s = secs as u64;
	format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// `MM:SS` for anything under an hour, `H:MM:SS` past it — how loom writes a chapter's start.
fn stamp(secs: u64) -> String {
	match secs / 3600 {
		0 => format!("{:02}:{:02}", secs / 60, secs % 60),
		h => format!("{h}:{:02}:{:02}", (secs % 3600) / 60, secs % 60),
	}
}

/// `MM:SS`, `H:MM:SS` or `HH:MM:SS`.
fn secs(stamp: &str) -> u64 {
	stamp.split(':').fold(0, |acc, part| acc * 60 + part.parse::<u64>().unwrap_or_else(|e| panic!("`{stamp}` is not a time: {e}")))
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

/// Phrases joined into prose. Each paragraph is `(start secs, text)`.
fn paragraphs(phrases: &[(f64, String)]) -> Vec<(f64, String)> {
	let mut out: Vec<(f64, String)> = Vec::new();
	let mut open = false;
	for (ts, text) in phrases {
		match open {
			true => {
				let last = &mut out.last_mut().expect("open implies one").1;
				last.push(' ');
				last.push_str(text.trim());
			}
			false => out.push((*ts, text.trim().to_string())),
		}
		let last = &out.last().expect("pushed above").1;
		// ponytail: word count + sentence end, speaker turns if loom ever exposes them
		open = !(last.split_whitespace().count() >= PARAGRAPH_WORDS && last.ends_with(['.', '?', '!']));
	}
	out
}

/// The capture, its filename, and whether it needs `/loom-digest`.
fn render(id: &str, page: &str) -> (String, String, bool) {
	let title = between(page, "<title>", '<', "the recording's title");
	let title = title.strip_suffix(" | Loom").unwrap_or(&title);
	let recorded = between(page, r#""uploadDate": ""#, '"', "the recording's date");
	let duration: f64 = between(page, r#""durationMs":"#, ',', "the recording's duration")
		.parse::<f64>()
		.expect("loom states durationMs as a number")
		/ 1000.;
	let (phrases, transcribed_by, loom_read_it) = match loom_phrases(page, id) {
		Some(p) if p.last().is_some_and(|(ts, _)| *ts >= duration * COVERED) => (p, "loom".to_string(), true),
		short => {
			let reach = short.as_ref().and_then(|p| p.last()).map_or("nothing".to_string(), |(ts, _)| format!("up to {}", hms(*ts)));
			eprintln!("  loom transcribed {reach} of {}", hms(duration));
			let model = Path::new(WHISPER_MODEL).file_stem().expect("the model is a file").to_string_lossy().into_owned();
			(whisper_phrases(id), format!("whisper-cpp `{model}` — loom transcribed {reach} of {}", hms(duration)), false)
		}
	};
	// what loom's AI wrote is a reading of loom's transcript, so it stops where that stopped
	let summary = ai(page, id, "description").filter(|_| loom_read_it);
	let chapters: Option<Vec<(u64, String)>> = ai(page, id, "chapters").filter(|_| loom_read_it).map(|c| {
		c.lines()
			.filter(|l| !l.trim().is_empty())
			.map(|l| {
				let (at, name) = l.trim().split_once(' ').unwrap_or_else(|| panic!("loom wrote `{l}` as a chapter"));
				(secs(at), name.to_string())
			})
			.collect()
	});

	let missing: Vec<&str> = [("summary", summary.is_none()), ("chapters", chapters.is_none())]
		.iter()
		.filter(|(_, m)| *m)
		.map(|(w, _)| *w)
		.collect();
	let mut out = format!(
		"# {title}\n\
		 \n\
		 - source: <{SHARE}{id}>\n\
		 - recorded: {recorded}\n\
		 - duration: {}\n\
		 - transcribed by: {transcribed_by}\n\
		 - read by: {}\n\
		 - pulled by: `scripts/loom-pull.rs`\n\
		 \n",
		hms(duration),
		match missing.is_empty() {
			true => "loom".to_string(),
			false => format!("nothing yet — loom wrote no {}, and `/loom-digest` writes them here", missing.join(" and no ")),
		},
	);
	if let Some(summary) = &summary {
		out.push_str(&format!("## summary\n\n{}\n\n", summary.trim()));
	}
	out.push_str("## transcript\n");
	let header = |t: u64, name: &str| format!("\n### [{}]({SHARE}{id}?t={t}) {name}\n", stamp(t)).replace(" \n", "\n");
	match &chapters {
		Some(chapters) => {
			let mut next = chapters.iter().peekable();
			let mut body = Vec::new();
			let mut started = false; // the first chapter opens the text wherever loom put it, so nothing sits above it
			//LOOP: bounded by the phrases of a finite transcript
			for (ts, text) in &phrases {
				while let Some((t, name)) = next.next_if(|(t, _)| !started || (*t as f64) <= *ts) {
					flush(&mut out, &mut body);
					out.push_str(&header(*t, name));
					started = true;
				}
				body.push((*ts, text.clone()));
			}
			flush(&mut out, &mut body);
			assert!(next.peek().is_none(), "{id}: loom chaptered past the last phrase of its transcript");
		}
		// no chapters to hang the text on, so every paragraph gets its own untitled anchor — the
		// times `/loom-digest` needs to place chapters, and deletes once it has
		None =>
			for (ts, text) in paragraphs(&phrases) {
				out.push_str(&header(ts as u64, ""));
				out.push_str(&format!("\n{text}\n"));
			},
	}
	(format!("{}-{}.md", &recorded[..10], slug(title)), out, missing.is_empty())
}

fn flush(out: &mut String, body: &mut Vec<(f64, String)>) {
	for (_, text) in paragraphs(body) {
		out.push_str(&format!("\n{text}\n"));
	}
	body.clear();
}
