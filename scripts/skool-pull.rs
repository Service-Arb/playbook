#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
serde_json = "1"
---

//! `./scripts/skool-pull.rs` — write `ref/skool_gmbpp/course/<lesson>.md` for every classroom lesson
//! we do not have yet.
//!
//! Skool is the one source here that needs a session, so the reading lives in `social_networks` and
//! this shells out to it, the way the other pullers shell out to yt-dlp and chromium. Depending on
//! that crate directly would mean resolving its tree without its lockfile, which picks versions its
//! own pins exist to avoid.
//!
//! Credentials are `social_networks`' own: `~/.config/social_networks`'s `[skool]` section, reading
//! `$DEFAULT_MAIL` / `$DEFAULT_PASSWORD`, with a browser-minted cookie cached between runs.
//!
//! The group's *conversations* are not this script's — `recon posts skool:<slug>` already writes
//! those to the rolodex's `venues/` tree.
//!
//! A lesson skool hosts itself carries a mux URL that is signed and dies within the hour, so it is
//! written down but is not a link anything can follow later. A lesson whose video is somebody's
//! pasted loom link is the useful case, and those are printed at the end to go into `ref/README.md`.

use std::{path::PathBuf, process::Command};

// one group, and the directory it was given by hand
const SLUG: &str = "gmp-passive-profits-5347";
const OUT: &str = "ref/skool_gmbpp/course";
const RECON: &str = "/home/v/s/social_networks/Cargo.toml";

fn main() {
	let root = repo_root();
	let out_dir = root.join(OUT);
	std::fs::create_dir_all(&out_dir).expect("the course dir is ours to create");

	let lessons = classroom();
	eprintln!("{} lessons in `{SLUG}`", lessons.len());

	let mut looms = Vec::new();
	let mut written = 0usize;
	for lesson in &lessons {
		let id = lesson["id"].as_str().expect("a lesson carries an id");
		let video = lesson["video"].as_str();
		if let Some(video) = video
			&& video.contains("loom.com/share/")
		{
			looms.push(video.to_string());
		}

		let out = out_dir.join(format!("{id}.md"));
		if out.exists() {
			continue;
		}
		let text = |key: &str| lesson[key].as_str().unwrap_or_else(|| panic!("lesson {id} carries no `{key}`")).to_string();
		// skool states the day it last changed; the time of day says nothing a re-read asks
		let at = text("at");
		std::fs::write(
			&out,
			format!(
				"# {}\n\
				 \n\
				 - source: <{}>\n\
				 - module: {}\n\
				 - updated: {}\n\
				 - video: {}\n\
				 - pulled by: `scripts/skool-pull.rs`\n\
				 \n\
				 {}\n",
				text("title"),
				text("permalink"),
				text("module"),
				at.split('T').next().expect("split yields at least once"),
				video.unwrap_or("none — this lesson is text"),
				text("body"),
			),
		)
		.expect("the course dir is ours to write");
		println!("{}", out.display());
		written += 1;
	}
	eprintln!("{written} new, {} already had", lessons.len() - written);

	if !looms.is_empty() {
		// the registry is written by hand on purpose, so this stops at saying what to put in it
		eprintln!("\nloom recordings in the classroom — paste under `## Sources` in ref/README.md, then run loom-pull.rs:");
		for loom in &looms {
			eprintln!("  {loom}");
		}
	}
}

/// `recon` prints the lessons as json on stdout and everything else on stderr, so stdout is the
/// whole answer and a failure to parse it is a failure to read the classroom.
fn classroom() -> Vec<serde_json::Value> {
	let out = Command::new("cargo")
		.args(["r", "-q", "--manifest-path", RECON, "--bin", "recon", "--", "classroom", &format!("skool:{SLUG}")])
		.output()
		.unwrap_or_else(|e| panic!("cargo: {e}"));
	if !out.status.success() {
		panic!("recon classroom failed:\n{}", String::from_utf8_lossy(&out.stderr));
	}
	let stdout = String::from_utf8(out.stdout).expect("recon prints utf-8");
	serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("recon's stdout is not a list of lessons: {e}\n{}", &stdout[..stdout.len().min(400)]))
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
