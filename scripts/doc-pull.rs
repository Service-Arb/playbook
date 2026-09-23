#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
ureq = "3"
---

//! `./scripts/doc-pull.rs [<doc-url>...]` — write `ref/docs/<title>.md` for every google doc we do
//! not have yet. With no arguments it takes the links already written down in `ref/README.md`.
//!
//! A doc shared by link exports itself as markdown to anyone, so this is one request per doc. A doc
//! is edited in place and publishes no date of its own, so the capture carries the day it was pulled;
//! a stale one is re-pulled by deleting it.

use std::{
	collections::BTreeMap,
	path::{Path, PathBuf},
	process::Command,
};

const DOC: &str = "https://docs.google.com/document/d/";

fn main() {
	let root = repo_root();
	let out_dir = root.join("ref/docs");
	std::fs::create_dir_all(&out_dir).expect("ref/docs is ours to create");

	let args: Vec<String> = std::env::args().skip(1).collect();
	let wanted: Vec<String> = match args.is_empty() {
		true => links_in(&root.join("ref/README.md")),
		false => args.iter().map(|a| id_of(a)).collect(),
	};
	let have = captured(&out_dir);
	for id in &wanted {
		if let Some(had) = have.get(id) {
			eprintln!("have {id} — {}", had.display());
			continue;
		}
		eprintln!("pulling {id}");
		let page = get(&format!("{DOC}{id}/edit"));
		let at = page.find("<title>").expect("a doc page has a title") + "<title>".len();
		let title = page[at..].split('<').next().expect("split yields at least once");
		let title = title.trim_end().strip_suffix(" - Google\u{a0}Docs").unwrap_or_else(|| panic!("{id}: `{title}` is not a doc's title — is it shared by link?"));
		let body = get(&format!("{DOC}{id}/export?format=md"));
		let today = String::from_utf8(Command::new("date").arg("+%F").output().expect("date runs").stdout).expect("date prints ascii");
		let doc = format!(
			"# {title}\n\n- source: <{DOC}{id}>\n- pulled: {} — a doc publishes no date, and is edited in place\n- pulled by: `scripts/doc-pull.rs`\n\n---\n\n{}\n",
			today.trim(),
			body.trim()
		);
		let out = out_dir.join(format!("{}.md", slug(title)));
		assert!(!out.exists(), "{} is taken, and not by {id}", out.display());
		std::fs::write(&out, doc).unwrap_or_else(|e| panic!("writing {}: {e}", out.display()));
		println!("{}", out.display());
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

fn id_of(arg: &str) -> String {
	let tail = arg.strip_prefix(DOC).unwrap_or_else(|| panic!("`{arg}` is not a google doc link"));
	tail.split(['/', '?', '#']).next().expect("split yields at least once").to_string()
}

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
		found.insert(id_of(source.trim_end_matches('>')), path);
	}
	found
}

/// The registry is `ref/README.md` and nothing else, as for every puller here.
fn links_in(readme: &Path) -> Vec<String> {
	let text = std::fs::read_to_string(readme).unwrap_or_else(|e| panic!("reading {}: {e}", readme.display()));
	let mut found: Vec<String> = text
		.match_indices(DOC)
		.map(|(at, _)| id_of(text[at..].split_whitespace().next().expect("split yields at least once").trim_end_matches([')', ']', ',', '.', '"', '`', '>'])))
		.collect();
	found.sort();
	found.dedup();
	found
}

fn get(url: &str) -> String {
	ureq::get(url)
		.call()
		.unwrap_or_else(|e| panic!("GET {url}: {e}"))
		.body_mut()
		.with_config()
		.limit(64 * 1024 * 1024)
		.read_to_string()
		.expect("google answers in utf-8")
}

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
