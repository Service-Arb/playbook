#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
social_networks_adapters = { path = "/home/v/s/social_networks/social_networks_adapters" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
# a cargo script carries no lockfile, and the version this resolves to on its own breaks
# grammers-crypto — social_networks' own lock pins it here
glass_pumpkin = "=2.0.0-rc0"
---

//! `./scripts/skool-pull.rs` — write `ref/skool_gmbpp/course/<lesson>.md` for every classroom lesson
//! we do not have yet.
//!
//! Skool is the one source here that needs a session, so the reading lives in `social_networks` and
//! this only files what it hands back. Credentials come from `~/.config/social_networks`'s
//! `[skool]` section, i.e. `$DEFAULT_MAIL` / `$DEFAULT_PASSWORD`, and a browser-minted cookie is
//! cached between runs.
//!
//! The group's *conversations* are not this script's: `recon posts skool:<slug>` already writes
//! those to the rolodex's `venues/` tree, and paying for them twice would be the only thing
//! duplicating it achieved.
//!
//! A lesson skool hosts itself carries a mux URL that is signed and dies within the hour, so it is
//! written down as the lesson's video but is not a link anything can follow later. A lesson whose
//! video is somebody's pasted loom link is the useful case, and those are printed at the end to be
//! pasted into `ref/README.md` for `loom-pull.rs`.

use std::path::PathBuf;

use social_networks_adapters::skool::{Skool, SkoolCredentials};
use social_networks_adapters::reach::{VenueRef, VenueSource};

// one group, and the directory it was given by hand
const SLUG: &str = "gmp-passive-profits-5347";
const OUT: &str = "ref/skool_gmbpp/course";

#[tokio::main]
async fn main() {
	let root = repo_root();
	let out_dir = root.join(OUT);
	std::fs::create_dir_all(&out_dir).expect("the course dir is ours to create");

	let creds = SkoolCredentials {
		email: std::env::var("DEFAULT_MAIL").expect("`[skool]` in ~/.config/social_networks reads the address from $DEFAULT_MAIL"),
		password: std::env::var("DEFAULT_PASSWORD").expect("`[skool]` in ~/.config/social_networks reads the password from $DEFAULT_PASSWORD"),
	};
	let mut skool = Skool::try_new(Some(creds)).expect("a skool session");
	let at = VenueRef::new(VenueSource::Skool, SLUG);

	let lessons = skool.classroom(&at).await.expect("the classroom is readable to a member");
	eprintln!("{} lessons in `{SLUG}`", lessons.len());

	let mut looms = Vec::new();
	let mut written = 0usize;
	for lesson in &lessons {
		if let Some(video) = &lesson.video
			&& video.contains("loom.com/share/")
		{
			looms.push(video.clone());
		}
		let out = out_dir.join(format!("{}.md", lesson.id));
		if out.exists() {
			continue;
		}
		let video = lesson.video.as_deref().unwrap_or("none — this lesson is text");
		std::fs::write(
			&out,
			format!(
				"# {}\n\
				 \n\
				 - source: <{}>\n\
				 - module: {}\n\
				 - updated: {}\n\
				 - video: {video}\n\
				 - pulled by: `scripts/skool-pull.rs`\n\
				 \n\
				 {}\n",
				lesson.title,
				lesson.permalink,
				lesson.module,
				lesson.at.strftime("%Y-%m-%d"),
				lesson.body
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
