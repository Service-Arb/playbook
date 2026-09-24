#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
ask_llm = { version = "3.4", default-features = false }
glass_pumpkin = "=2.0.0-rc0" # social_networks' lock; rc1 breaks grammers-crypto, which asks for `2.0.0-rc0`
jiff = "0.2"
social_networks_adapters = { path = "/home/v/s/social_networks/social_networks_adapters", features = ["youtube-reads"] }
tokio = { version = "1", features = ["full"] }
v_utils_macros = "=2.12.5" # social_networks' lock; later ones call into a v_utils newer than 2.17.6
---

//! `./scripts/yt-pull.rs sync` — list every video of every channel `ref/README.md` links, in
//! `ref/youtube/README.md`, under a header per person: the link text names their directory.
//! `./scripts/yt-pull.rs transcribe` — capture every video that list has unticked, into
//! `ref/youtube/<who>/<id>.md`, ticking it as it goes.
//!
//! Youtube is read by `social_networks_adapters`, linked; this only decides how a capture is filed.
//!
//! Auto-captions arrive a few words per cue, so cues are gathered into blocks of at least `BLOCK` seconds. A block is both a
//! readable paragraph and a `&t=` anchor that lands where the words are.
//!
//! Half of what these videos say is said on screen — a dashboard, a search result, a review count —
//! and captions carry none of it, so a capture also holds what the picture shows, read by `ask_llm`'s
//! `Client::watch` with the captions as its speech, each line beside the frame it was read off. Then
//! chapters, a summary and the description. Chapters are the uploader's own where youtube has them and
//! the model's reading of the transcript where it does not.
//!
//! Enrichment that cannot be produced aborts the video and writes nothing. The md file existing is
//! what makes the next run skip it, so a half-written one would never be repaired.

use std::{
	collections::{BTreeMap, BTreeSet},
	path::{Path, PathBuf},
};

use ask_llm::{Client, Footage, Model, Said, Shown, Watch};
use jiff::civil::Date;
use social_networks_adapters::youtube::{self, Chapter, Cue};

const WATCH: &str = "https://www.youtube.com/watch?v=";
const INDEX: &str = "ref/youtube/README.md";
const BLOCK: f64 = 30.;
const SUMMARY: &str = "SUMMARY";
const SECTIONS: &str = "SECTIONS";

struct Index {
	checked: String,
	sections: Vec<Section>,
}

struct Section {
	who: String,
	channel: String,
	entries: Vec<Entry>,
}

#[derive(Clone)]
struct Entry {
	id: String,
	uploaded: Date,
	title: String,
}

#[tokio::main]
async fn main() {
	let root = repo_root();
	match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
		[cmd] if cmd == "sync" => sync(&root).await,
		[cmd] if cmd == "transcribe" => transcribe(&root).await,
		_ => panic!("usage: yt-pull.rs <sync|transcribe>"),
	}
}

async fn sync(root: &Path) {
	let channels = channels_in(&root.join("ref/README.md"));
	assert!(!channels.is_empty(), "no youtube channel linked in ref/README.md — nothing to sync");
	// youtube's flat listing dates nothing, so a video's date is asked for once and read back from here after
	let known: BTreeMap<String, Entry> = match root.join(INDEX).exists() {
		true => parse(root).sections.into_iter().flat_map(|s| s.entries).map(|e| (e.id.clone(), e)).collect(),
		false => BTreeMap::new(),
	};
	let mut sections = Vec::new();
	for (who, channel) in channels {
		let mut ids: BTreeSet<String> = youtube::uploads(&channel).await.unwrap_or_else(|e| panic!("listing {channel}: {e:?}")).into_iter().collect();
		ids.extend(captured(&root.join("ref/youtube").join(&who))); // one taken down upstream stays, as its capture does
		let unknown: Vec<&str> = ids.iter().filter(|id| !known.contains_key(*id)).map(String::as_str).collect();
		eprintln!("{who}: {} video(s), {} not dated yet", ids.len(), unknown.len());
		let mut entries: Vec<Entry> = ids.iter().filter_map(|id| known.get(id).cloned()).collect();
		let listed = youtube::listing(&unknown).await.unwrap_or_else(|e| panic!("dating {unknown:?}: {e:?}"));
		entries.extend(listed.into_iter().map(|l| Entry { id: l.id, uploaded: l.uploaded, title: l.title }));
		entries.sort_by(|a, b| b.uploaded.cmp(&a.uploaded).then(a.id.cmp(&b.id)));
		sections.push(Section { who, channel, entries });
	}
	write(root, &Index { checked: jiff::Zoned::now().date().to_string(), sections });
}

async fn transcribe(root: &Path) {
	let index = parse(root);
	let tmp = std::env::temp_dir().join("yt-pull");
	let mut mute = Vec::new();
	for section in &index.sections {
		let dir = root.join("ref/youtube").join(&section.who);
		std::fs::create_dir_all(&dir).expect("ref/youtube is ours to create under");
		for Entry { id, .. } in &section.entries {
			let out = dir.join(format!("{id}.md"));
			if out.exists() {
				continue;
			}
			eprintln!("pulling {}/{id}", section.who);
			// a video youtube never captioned is a real absence, not a broken run — the rest of the
			// channel is still worth having, so it is named at the end rather than aborting here
			match video(id, &tmp, &dir.join(id)).await {
				Some(doc) => {
					std::fs::write(&out, doc).expect("ref/youtube is ours to write");
					write(root, &index);
					println!("{}", out.display());
				}
				None => mute.push(id.clone()),
			}
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

/// `[<who>](<channel>)` for every youtube link in the registry. The registry is `ref/README.md` and
/// nothing else — a capture quotes its own source URL, and a transcript quotes every link that was
/// said out loud, so scanning the tree would make the pullers feed on their own output.
fn channels_in(readme: &Path) -> Vec<(String, String)> {
	let text = std::fs::read_to_string(readme).unwrap_or_else(|e| panic!("reading {}: {e}", readme.display()));
	let mut found = Vec::new();
	//LOOP: bounded by the number of occurrences in a finite file
	for (at, _) in text.match_indices("](https://www.youtube.com/") {
		let who = &text[text[..at].rfind('[').expect("a markdown link opens its text with `[`") + 1..at];
		assert!(
			!who.is_empty() && who.chars().all(|c| c.is_ascii_lowercase()),
			"a youtube link in ref/README.md names the directory its videos go in, as its text, and reads `{who}`"
		);
		let channel = &text[at + 2..];
		found.push((who.to_string(), channel[..channel.find(')').expect("a markdown link closes with `)`")].to_string()));
	}
	found
}

fn captured(dir: &Path) -> Vec<String> {
	if !dir.exists() {
		return Vec::new();
	}
	std::fs::read_dir(dir)
		.unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
		.map(|e| e.expect("a directory entry is readable").path())
		.filter(|p| p.extension().is_some_and(|e| e == "md"))
		.map(|p| p.file_stem().expect("filtered on an extension").to_str().expect("a video id is ascii").to_string())
		.collect()
}

/// Every tick is read off the disk, so a capture pulled or deleted is reflected on the next write.
fn write(root: &Path, index: &Index) {
	let spent = index
		.sections
		.iter()
		.flat_map(|s| s.entries.iter().map(move |e| root.join("ref/youtube").join(&s.who).join(format!("{}.md", e.id))))
		.filter(|p| p.exists())
		.map(|p| {
			let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("reading {}: {e}", p.display()));
			let cost = text.lines().find_map(|l| l.strip_prefix("- cost: $")).unwrap_or_else(|| panic!("{} states no cost — delete it to re-pull", p.display()));
			cost.trim().parse::<f64>().unwrap_or_else(|e| panic!("{}: `{cost}`: {e}", p.display()))
		})
		.fold((0f64, 0usize), |(usd, n), c| (usd + c, n + 1));
	let mut out = format!(
		"# YouTube, as the channels serve it\n\
		 \n\
		 - checked: {}\n\
		 - written by: `scripts/yt-pull.rs sync`, which lists every video; `transcribe` ticks each it captures\n\
		 \n\
		 - spent: ${:.2} over {} captures, as their `cost:` lines state\n\
		 \n\
		 A directory per person, named by the text of their channel's link in `../README.md`. In it,\n\
		 `<id>.md` is a video's capture — summary, chapters, what is shown, description, then the captions\n\
		 in citable blocks — and `<id>/<secs>.jpg` the frames its `## shown` lines were read off.\n\
		 \n\
		 A line per video, newest first: the day youtube says it was uploaded, linked to the video, then\n\
		 its title — linked to the capture once there is one. `scripts/yt-pull.rs transcribe` captures\n\
		 every unticked line.\n",
		index.checked,
		spent.0,
		spent.1,
	);
	for Section { who, channel, entries } in &index.sections {
		out.push_str(&format!("\n## {who} — <{channel}>\n\n"));
		for Entry { id, uploaded, title } in entries {
			match root.join("ref/youtube").join(who).join(format!("{id}.md")).exists() {
				true => out.push_str(&format!("- [x] [{uploaded}]({WATCH}{id}) [{title}]({who}/{id}.md)\n")),
				false => out.push_str(&format!("- [ ] [{uploaded}]({WATCH}{id}) {title}\n")),
			}
		}
	}
	std::fs::write(root.join(INDEX), out).expect("ref/youtube is ours to write");
}

fn parse(root: &Path) -> Index {
	let path = root.join(INDEX);
	let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e} — run `yt-pull.rs sync` first", path.display()));
	let mut checked = None;
	let mut sections: Vec<Section> = Vec::new();
	for line in text.lines() {
		if let Some(day) = line.strip_prefix("- checked: ") {
			checked = Some(day.to_string());
		} else if let Some(header) = line.strip_prefix("## ") {
			let (who, channel) = header.split_once(" — ").unwrap_or_else(|| panic!("{INDEX}: a header is `<who> — <channel>`, and reads {header:?}"));
			sections.push(Section { who: who.to_string(), channel: channel.trim_matches(['<', '>']).to_string(), entries: Vec::new() });
		} else if let Some(rest) = line.strip_prefix("- [x] [").or_else(|| line.strip_prefix("- [ ] [")) {
			let section = sections.last_mut().unwrap_or_else(|| panic!("{INDEX}: {line:?} sits under no channel's header"));
			let bad = format!("{INDEX}: a video's line is `- [ ] [<uploaded>]({WATCH}<id>) <title>`, and reads {line:?}");
			let (uploaded, rest) = rest.split_once("](").unwrap_or_else(|| panic!("{bad}"));
			let (url, title) = rest.split_once(") ").unwrap_or_else(|| panic!("{bad}"));
			let id = url.strip_prefix(WATCH).unwrap_or_else(|| panic!("{bad}")).to_string();
			let title = match line.starts_with("- [x]") {
				true => title.strip_prefix('[').and_then(|t| t.strip_suffix(&format!("]({}/{id}.md)", section.who))).unwrap_or_else(|| panic!("{bad}")),
				false => title,
			};
			let uploaded = uploaded.parse().unwrap_or_else(|e| panic!("{bad}: {e}"));
			section.entries.push(Entry { id, uploaded, title: title.to_string() });
		}
	}
	Index { checked: checked.unwrap_or_else(|| panic!("{INDEX} states no `checked:` day")), sections }
}

fn hms(secs: f64) -> String {
	let s = secs as u64;
	format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

async fn video(id: &str, tmp: &Path, shots: &Path) -> Option<String> {
	let video = youtube::video(id).await.unwrap_or_else(|e| panic!("reading {id}: {e:?}"));
	let blocks = blocks(&video.captions?);
	assert!(!blocks.is_empty(), "{id} has a caption track that carries no words");

	let answer = Client::default()
		.model(Model::Fast)
		.ask(prompt(&video.title, &blocks, video.chapters.is_none()))
		.await
		.unwrap_or_else(|e| panic!("summarising {id}: {e:?}"));
	let sections: Vec<(f64, String)> = match video.chapters {
		Some(chapters) => chapters.into_iter().map(|Chapter { at, title }| (at, title)).collect(),
		None => derived(&answer.text, id, video.duration),
	};

	// a video from an earlier run would be picked up as this one's
	if tmp.exists() {
		std::fs::remove_dir_all(tmp).unwrap_or_else(|e| panic!("clearing {}: {e}", tmp.display()));
	}
	std::fs::create_dir_all(tmp).expect("the temp dir is ours to create");
	let media = tmp.join("video");
	youtube::download(id, &media).await.unwrap_or_else(|e| panic!("pulling {id}: {e:?}"));
	let spec = Watch {
		title: video.title.clone(),
		speech: Some(blocks.iter().map(|(secs, text)| Said { secs: *secs, text: text.clone() }).collect()),
		footage: Footage::Screen,
		frames: shots.to_path_buf(),
	};
	let watched = Client::default().model(Model::Video).watch(&media, spec).await.unwrap_or_else(|e| panic!("watching {id}: {e:?}"));
	std::fs::remove_dir_all(tmp).unwrap_or_else(|e| panic!("removing {}: {e}", tmp.display()));
	let watched_by = watched.model.as_deref().unwrap_or_else(|| panic!("{id} downloaded with no picture"));

	let mut out = format!(
		"# {}\n\
		 \n\
		 - source: <{WATCH}{id}>\n\
		 - channel: {}\n\
		 - uploaded: {}\n\
		 - duration: {}\n\
		 - pulled by: `scripts/yt-pull.rs`\n\
		 - read by: `{}` summary and chapters, `{watched_by}` what is shown, over {} frames where the picture changes\n\
		 - cost: ${:.4}\n\
		 \n\
		 ## summary\n\
		 \n",
		video.title,
		video.channel,
		video.uploaded,
		hms(video.duration),
		answer.model,
		watched.frames_read,
		(answer.cost_cents + watched.cost_cents) as f64 / 100.,
	);
	for line in marked(&answer.text, SUMMARY, id) {
		out.push_str(line);
		out.push('\n');
	}

	out.push_str("\n## chapters\n\n");
	for (at, heading) in &sections {
		out.push_str(&format!("- [{}]({WATCH}{id}&t={}) {heading}\n", hms(*at), *at as u64));
	}

	out.push_str("\n## shown\n");
	for Shown { secs, shown, on_screen_text, .. } in &watched.shown {
		out.push_str(&format!("\n- [{}]({WATCH}{id}&t={secs}) {}\n", hms(*secs as f64), shown.replace('\n', " ")));
		if let Some(text) = on_screen_text {
			out.push_str(&format!("  > {}\n", text.replace('\n', " / ")));
		}
		out.push_str(&format!("  ![]({id}/{secs}.jpg)\n"));
	}

	if let Some(description) = &video.description {
		out.push_str(&format!("\n## description\n\n```\n{}\n```\n", description.trim()));
		let links = urls_in(description);
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
	Some(out)
}

fn urls_in(text: &str) -> BTreeSet<String> {
	text.split_whitespace()
		.map(|t| t.trim_end_matches([')', ']', ',', '.', '"', '`', '>']))
		.filter(|t| t.starts_with("http://") || t.starts_with("https://"))
		.map(str::to_string)
		.collect()
}

fn blocks(cues: &[Cue]) -> Vec<(f64, String)> {
	let mut out = Vec::new();
	let mut start = None;
	let mut words = String::new();
	//LOOP: one pass over a finite caption track
	for Cue { at, text } in cues {
		let at = *at;
		let block = *start.get_or_insert(at);
		if !words.is_empty() {
			words.push(' ');
		}
		words.push_str(text);
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
			 section where the speaker turns to a new thing.\n"
		));
	}
	ask.push_str("\n--- transcript ---\n");
	for (at, words) in blocks {
		ask.push_str(&format!("{} {words}\n", *at as u64));
	}
	ask
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
