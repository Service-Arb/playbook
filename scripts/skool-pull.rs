#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
clap = { version = "4", features = ["derive"] }
glass_pumpkin = "=2.0.0-rc0" # social_networks' lock; rc1 breaks grammers-crypto, which asks for `2.0.0-rc0`
jiff = "0.2"
serde = { version = "1", features = ["derive"] }
social_networks_adapters = "=0.3.23"
tokio = { version = "1", features = ["full"] }
v_utils = { version = "=2.17.6", features = ["xdg", "cli"] }
v_utils_macros = "=2.12.5" # social_networks' lock; later ones call into a v_utils newer than 2.17.6
---

//! `./scripts/skool-pull.rs` — mirror each group's classroom under `ref/skool_<group>/course/`, one
//! directory per module and one file per lesson, both numbered in the order skool serves them.
//!
//! Skool is read by `social_networks_adapters`, linked; this only decides how a capture is filed.
//!
//! Credentials are `social_networks`' own: `~/.config/social_networks`'s `[skool]` section, reading
//! `$DEFAULT_MAIL` / `$DEFAULT_PASSWORD`, with a browser-minted cookie cached between runs.
//!
//! The group's *conversations* are not this script's — `recon posts skool:<slug>` already writes
//! those to the rolodex's `venues/` tree.
//!
//! A capture is identified by the id it states and not by where it sits, so a lesson
//! upstream renames or reorders is moved rather than pulled twice. What it says is never edited:
//! `ref/skool_<group>/README.md` is rewritten every run with the day of the run and with every lesson
//! skool has touched since we captured it, and re-pulling one is deleting its file.
//!
//! Every link the classroom says out loud — the lesson's video, the links in its body, whatever sits
//! in its resources — is collected, and the ones missing from `ref/README.md` are printed at the end
//! to go into it. A lesson skool hosts itself carries a mux URL that is signed and dies within the
//! hour, so it is written down but is not a link anything can follow later.

use std::{
	collections::{BTreeMap, BTreeSet},
	path::{Path, PathBuf},
};

use social_networks_adapters::{
	reach::{VenueRef, VenueSource},
	skool::{Course, Skool, SkoolCredentials, Video},
};
use v_utils::macros::{MyConfigPrimitives, Settings};

// each group, and the directory it was given by hand
const GROUPS: &[(&str, &str)] = &[("gmp-passive-profits-5347", "ref/skool_gmbpp"), ("20kmodropservicingblueprint", "ref/skool_cheap")];

/// The `[skool]` section of `~/.config/social_networks`, read as `recon` reads it.
#[derive(Clone, Debug, Default, MyConfigPrimitives, Settings)]
#[settings(config_name = "social_networks")]
struct Config {
	#[settings(skip)]
	#[serde(default)]
	skool: Option<SkoolCredentials>,
}

#[tokio::main]
async fn main() {
	let root = repo_root();
	let config = Config::try_build(SettingsFlags::default()).unwrap_or_else(|e| panic!("reading ~/.config/social_networks: {e}"));
	let creds = config.skool.expect("a skool classroom is only readable by a member, so this needs a `[skool]` section in ~/.config/social_networks");
	let mut skool = Skool::try_new(Some(creds)).unwrap_or_else(|e| panic!("{e:?}"));
	let mut links: BTreeSet<String> = BTreeSet::new();
	for (group, out_root) in GROUPS {
		let courses = skool.classroom(&VenueRef::new(VenueSource::Skool, *group)).await.unwrap_or_else(|e| panic!("reading {group}'s classroom: {e:?}"));
		links.extend(pull(&root, group, out_root, &courses));
	}

	let registry = std::fs::read_to_string(root.join("ref/README.md")).expect("ref/README.md is the registry");
	let missing: Vec<&String> = links.iter().filter(|link| !registry.contains(link.as_str())).collect();
	if !missing.is_empty() {
		// the registry is written by hand on purpose, so this stops at saying what to put in it
		eprintln!("\nlinks the classroom says and `ref/README.md` does not — paste them under `## Sources`, then re-run the puller of their platform:");
		for link in missing {
			eprintln!("  {link}");
		}
	}
}

/// One group's classroom into `out`, returning every link it says out loud.
fn pull(root: &Path, group: &str, out_root: &str, courses: &[Course]) -> BTreeSet<String> {
	let out_dir = root.join(out_root).join("course");
	std::fs::create_dir_all(&out_dir).expect("the course dir is ours to create");

	let today = jiff::Zoned::now().date();
	let have = captured(&out_dir);

	let mut index = format!(
		"# The classroom, as skool serves it\n\
		 \n\
		 - source: <https://www.skool.com/{group}/classroom>\n\
		 - checked: {today}\n\
		 - written by: `scripts/skool-pull.rs`, every run\n\
		 \n\
		 A lesson skool has touched since we captured it is marked `stale` — delete its file and run\n\
		 this again to re-pull it.\n"
	);
	let mut links: BTreeSet<String> = BTreeSet::new();
	let mut written = 0usize;
	let mut stale = 0usize;

	for (m, course) in courses.iter().enumerate() {
		let module_dir = out_dir.join(format!("{:02}-{}", m + 1, slug(&course.title)));
		std::fs::create_dir_all(&module_dir).expect("a module dir is ours to create");
		index.push_str(&format!("\n## {}\n\n", course.title));

		written += place(
			&module_dir.join("README.md"),
			&have,
			&course.id,
			&format!(
				"# {}\n\
				 \n\
				 - source: <{}>\n\
				 - id: {}\n\
				 - updated: {}\n\
				 - pulled by: `scripts/skool-pull.rs`\n\
				 \n\
				 {}\n",
				course.title,
				course.permalink,
				course.id,
				day(course.at),
				course.body,
			),
		) as usize;

		for (l, lesson) in course.lessons.iter().enumerate() {
			let id = lesson.id.as_str();
			links.extend(urls_in(&lesson.body));
			if let Some(resources) = &lesson.resources {
				links.extend(urls_in(resources));
			}
			// a mux URL is skool's own player and not a source anything else can be pointed at
			if let Some(Video::Link(link)) = &lesson.video {
				links.insert(link.clone());
			}

			let at = day(lesson.at);
			let doc = format!(
				"# {}\n\
				 \n\
				 - source: <{}>\n\
				 - id: {}\n\
				 - module: {}\n\
				 - updated: {}\n\
				 - video: {}\n\
				 - pulled by: `scripts/skool-pull.rs`\n\
				 \n\
				 {}\n{}",
				lesson.title,
				lesson.permalink,
				id,
				lesson.module,
				at,
				match &lesson.video {
					// the signed URL dies within the hour; the playback id is the part a re-read can act on
					Some(Video::Mux { playback_id, .. }) => format!("mux:{playback_id}"),
					Some(Video::Link(link)) => link.clone(),
					None => "none — this lesson is text".to_string(),
				},
				lesson.body,
				lesson.resources.as_ref().map(|r| format!("\n## resources\n\n```json\n{r}\n```\n")).unwrap_or_default(),
			);
			// a capture is never edited, so what skool changed under one is said in the index instead —
			// read before the capture is placed, since placing it is what moves it
			let behind = have.get(id).is_some_and(|had| captured_at(had) != at);
			stale += behind as usize;

			let out = module_dir.join(format!("{:02}-{}.md", l + 1, slug(&lesson.title)));
			written += place(&out, &have, id, &doc) as usize;
			index.push_str(&format!(
				"- [{}]({}){}\n",
				lesson.title,
				relative(&root.join(out_root), &out),
				match behind {
					true => format!(" — **stale**: skool says {at}"),
					false => String::new(),
				}
			));
		}
	}

	prune(&out_dir);
	std::fs::write(root.join(out_root).join("README.md"), index).expect("the index is ours to write");
	eprintln!("{written} new, {stale} stale — see {out_root}/README.md");
	links
}

/// Write `doc` where it belongs, moving a capture of the same lesson that sits somewhere else — a
/// rename or a reorder upstream changes the path and nothing about what was captured.
fn place(out: &Path, have: &BTreeMap<String, PathBuf>, id: &str, doc: &str) -> bool {
	match have.get(id) {
		Some(had) if had == out => false,
		Some(had) => {
			assert!(
				!out.exists(),
				"{} belongs at {}, and another capture is already there — delete its `course/` and run this again",
				had.display(),
				out.display()
			);
			std::fs::rename(had, out).unwrap_or_else(|e| panic!("moving {} to {}: {e}", had.display(), out.display()));
			eprintln!("moved {} → {}", had.display(), out.display());
			false
		}
		None => {
			std::fs::write(out, doc).expect("the course dir is ours to write");
			println!("{}", out.display());
			true
		}
	}
}

/// Every capture already on disk, by the id it states. Reading the tree rather than trusting its
/// shape is what lets the shape change without re-pulling the whole classroom.
fn captured(dir: &Path) -> BTreeMap<String, PathBuf> {
	let mut found = BTreeMap::new();
	//LOOP: bounded by the number of files in a finite tree
	for entry in walk(dir) {
		let text = std::fs::read_to_string(&entry).unwrap_or_else(|e| panic!("reading {}: {e}", entry.display()));
		let id = line(&text, "- id: ").unwrap_or_else(|| panic!("{} states no id — it was not written by this script", entry.display()));
		if let Some(clash) = found.insert(id.to_string(), entry.clone()) {
			panic!("{} and {} both claim {id}", clash.display(), entry.display());
		}
	}
	found
}

/// A module skool renamed leaves the directory it used to be, and an empty one says a lesson is
/// missing when nothing is.
fn prune(dir: &Path) {
	//LOOP: bounded by the number of entries in a finite tree
	for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
		let path = entry.expect("a directory entry is readable").path();
		if path.is_dir() && std::fs::read_dir(&path).expect("a directory we just listed is readable").next().is_none() {
			std::fs::remove_dir(&path).unwrap_or_else(|e| panic!("removing {}: {e}", path.display()));
		}
	}
}

fn walk(dir: &Path) -> Vec<PathBuf> {
	let mut out = Vec::new();
	//LOOP: bounded by the number of entries in a finite tree
	for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
		let path = entry.expect("a directory entry is readable").path();
		match path.is_dir() {
			true => out.extend(walk(&path)),
			false => out.push(path),
		}
	}
	out
}

fn line<'a>(text: &'a str, key: &str) -> Option<&'a str> {
	text.lines().find_map(|l| l.strip_prefix(key)).map(str::trim)
}

fn captured_at(path: &Path) -> String {
	let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
	line(&text, "- updated: ")
		.unwrap_or_else(|| panic!("{} states no updated date — it was not written by this script", path.display()))
		.to_string()
}

fn relative(base: &Path, path: &Path) -> String {
	path.strip_prefix(base).expect("every capture sits under the course dir").display().to_string()
}

/// skool states the moment it last changed; the time of day says nothing a re-read asks
fn day(at: jiff::Timestamp) -> String {
	at.strftime("%F").to_string()
}

/// A filename that says what the thing is. The id stays in the file, so this one is free to read
/// like the title it came from.
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

/// Every `http(s)://` run in a blob of text. A lesson's links are the half of it that outlives the
/// lesson, and they are the reason `ref/README.md` grows.
fn urls_in(text: &str) -> Vec<String> {
	let mut found = Vec::new();
	//LOOP: bounded by the number of occurrences in a finite string
	for (at, _) in text.match_indices("http") {
		let rest = &text[at..];
		if !rest.starts_with("http://") && !rest.starts_with("https://") {
			continue;
		}
		let link = rest.split_whitespace().next().expect("split yields at least once");
		let link = link.trim_end_matches([')', ']', ',', '.', '"', '\'', '`', '>', '\\']);
		if !link.is_empty() {
			found.push(link.to_string());
		}
	}
	found
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
