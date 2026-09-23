#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
jiff = "0.2"
---

//! `./scripts/cite-check.rs [<root>]` — hold every block under `structured/` to
//! `structured/README.md`: each citation resolves to a capture in `ref/`, carries that capture's date,
//! stays inside its duration, and sits in its run by effective reliability; no bullet goes unbacked.
//! `<root>` defaults to the checkout this runs in.
//!
//! Prints every failure as `path:line: why` and exits non-zero if there was one.

use std::{
	collections::BTreeMap,
	path::{Path, PathBuf},
};

use jiff::civil::Date;

const ISSUE_COMMENT: &str = "https://github.com/Service-Arb/playbook/issues/";
const SECTIONS: [&str; 3] = ["## info", "## facts", "## plays"];
const DECAY_DAYS: f64 = 180.;
const WORST: f64 = 10.;

/// What a capture in `ref/` says about itself.
struct Capture {
	date: Date,
	duration: Option<u64>,
}

struct Citation {
	line: usize,
	base: u8,
	date: Date,
}

fn main() {
	let root = match std::env::args().nth(1) {
		Some(dir) => PathBuf::from(dir),
		None => repo_root(),
	};
	let today = jiff::Zoned::now().date();
	let captures = captures(&root.join("ref"));

	let mut blocks = Vec::new();
	md_files(&root.join("structured"), &mut blocks);
	blocks.retain(|p| p.file_name().expect("walked files have names") != "README.md");
	assert!(!blocks.is_empty(), "no blocks under {}/structured", root.display());

	let mut errors = Vec::new();
	for path in &blocks {
		let shown = path.strip_prefix(&root).expect("walked from root").display().to_string();
		let text = std::fs::read_to_string(path).expect("walked files are readable");
		for (line, why) in check(&text, &captures, today) {
			errors.push(format!("{shown}:{line}: {why}"));
		}
	}
	for e in &errors {
		eprintln!("{e}");
	}
	if !errors.is_empty() {
		eprintln!("{} citation errors across {} blocks", errors.len(), blocks.len());
		std::process::exit(1);
	}
	eprintln!("{} blocks hold their citations", blocks.len());
}

fn check(text: &str, captures: &BTreeMap<String, Capture>, today: Date) -> Vec<(usize, String)> {
	let mut errors = Vec::new();
	let mut section = None;
	let mut unbacked: Option<usize> = None;
	let mut run: Vec<Citation> = Vec::new();
	let mut fenced = false;

	for (i, raw) in text.lines().enumerate() {
		let line = i + 1;
		let trimmed = raw.trim();
		if trimmed.starts_with("```") {
			fenced = !fenced;
			continue;
		}
		if fenced {
			continue;
		}

		let cited = match link_only(trimmed) {
			Some((label, url)) => match citation(line, label, url, captures) {
				Ok(c) => {
					run.push(c);
					true
				}
				Err(why) => {
					errors.push((line, why));
					true
				}
			},
			None => false,
		};
		if !cited {
			sorted(&run, today, &mut errors);
			run.clear();
		}
		if cited || trimmed.starts_with("// unsourced:") {
			if trimmed == "// unsourced:" {
				errors.push((line, "`// unsourced:` names nobody".into()));
			}
			unbacked = None;
			continue;
		}

		if raw.starts_with('#') {
			if let Some(b) = unbacked.take() {
				errors.push((b, "bullet has no citation or `// unsourced: <who>` before the section ends".into()));
			}
			match SECTIONS.iter().position(|s| *s == raw) {
				Some(pos) if section.is_none_or(|prev| pos > prev) => section = Some(pos),
				Some(_) => errors.push((line, "sections go info, facts, plays, each at most once".into())),
				None => errors.push((line, format!("a block's only headers are {SECTIONS:?}"))),
			}
			continue;
		}
		if !trimmed.is_empty() && section.is_none() {
			errors.push((line, "text above the first section".into()));
			section = Some(0); // reported once; what follows is read as `## info`
		}
		if raw.starts_with("- ") && unbacked.is_none() {
			unbacked = Some(line);
		}
	}
	sorted(&run, today, &mut errors);
	if let Some(b) = unbacked {
		errors.push((b, "bullet has no citation or `// unsourced: <who>` before the section ends".into()));
	}
	errors
}

/// `[label](url)` and nothing else on the line.
fn link_only(line: &str) -> Option<(&str, &str)> {
	let rest = line.strip_prefix('[')?.strip_suffix(')')?;
	let (label, url) = rest.split_once("](")?;
	(!label.contains(']') && !url.contains(' ')).then_some((label, url))
}

fn citation(line: usize, label: &str, url: &str, captures: &BTreeMap<String, Capture>) -> Result<Citation, String> {
	let parsed = label
		.strip_prefix('r')
		.and_then(|l| l.split_once(' '))
		.and_then(|(b, d)| Some((b.parse::<u8>().ok().filter(|b| *b <= 7)?, d.parse::<Date>().ok()?)));
	let Some((base, date)) = parsed else {
		return Err(format!("a link alone on a line is a citation, and `{label}` is not `r<0-7> <YYYY-MM-DD>`"));
	};

	if url.starts_with(ISSUE_COMMENT) {
		// a test's result lives on github and not in ref/, so there is no capture to hold its date to
		if !url.contains("#issuecomment-") {
			return Err("an `r0` cites the issue comment that closed the test, not the issue".into());
		}
		if base != 0 {
			return Err(format!("an issue comment is a test of our own, so `r0`, not `r{base}`"));
		}
		return Ok(Citation { line, base, date });
	}
	if base == 0 {
		return Err(format!("`r0` is a test of our own, cited by its comment under {ISSUE_COMMENT}"));
	}

	let key = source_key(url).ok_or_else(|| format!("`{url}` is no source this checker knows"))?;
	let capture = captures.get(&key).ok_or_else(|| format!("`{url}` has no capture in ref/"))?;
	if key.starts_with("skool:") && base != 1 {
		return Err(format!("skool course material is `r1`, not `r{base}`"));
	}
	if date != capture.date {
		return Err(format!("dated {date}, but the capture is dated {}", capture.date));
	}
	if let Some(t) = param(url, "t") {
		let t: u64 = t.trim_end_matches('s').parse().map_err(|_| format!("`t={t}` is not a second"))?;
		match capture.duration {
			Some(d) if t > d => return Err(format!("`t={t}` is past the capture's end at {d}s")),
			Some(_) => {}
			None => return Err("`t=` on a capture that has no duration".into()),
		}
	}
	Ok(Citation { line, base, date })
}

/// A run leads with its best effective reliability, and a tie goes to the newer one.
fn sorted(run: &[Citation], today: Date, errors: &mut Vec<(usize, String)>) {
	let effective = |c: &Citation| {
		let age = (today - c.date).get_days() as f64;
		assert!(age >= 0., "cited {} is in the future", c.date);
		(c.base as f64 + age / DECAY_DAYS).min(WORST)
	};
	for pair in run.windows(2) {
		let (a, b) = (&pair[0], &pair[1]);
		let order = effective(a).total_cmp(&effective(b)).then(b.date.cmp(&a.date));
		if order.is_gt() {
			errors.push((b.line, format!("outranks the citation above it ({:.2} vs {:.2} effective)", effective(b), effective(a))));
		}
	}
}

/// The identity a url shares with the `source:` of its capture.
fn source_key(url: &str) -> Option<String> {
	let after = |prefix: &str| url.split_once(prefix).map(|(_, rest)| rest.split(['?', '#', '&', '/']).next().expect("split yields one").to_string());
	if let Some(id) = after("loom.com/share/") {
		return Some(format!("loom:{id}"));
	}
	if let Some(id) = after("fathom.video/share/") {
		return Some(format!("fathom:{id}"));
	}
	if let Some(id) = after("chatgpt.com/share/") {
		return Some(format!("chatgpt:{id}"));
	}
	if let Some(id) = after("youtu.be/") {
		return Some(format!("youtube:{id}"));
	}
	if url.contains("youtube.com/watch") {
		return Some(format!("youtube:{}", param(url, "v")?));
	}
	if url.contains("skool.com/") && url.contains("/classroom") {
		return Some(format!("skool:{}", param(url, "md")?));
	}
	None
}

fn param<'a>(url: &'a str, name: &str) -> Option<&'a str> {
	let query = url.split_once('?')?.1.split('#').next().expect("split yields one");
	query.split('&').find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
}

/// Every capture under `ref/` that states a source this checker can cite, by that source.
fn captures(ref_dir: &Path) -> BTreeMap<String, Capture> {
	let mut files = Vec::new();
	md_files(ref_dir, &mut files);
	let mut out = BTreeMap::new();
	for path in files {
		let text = std::fs::read_to_string(&path).expect("walked files are readable");
		let field = |name: &str| text.lines().find_map(|l| l.strip_prefix("- ")?.strip_prefix(name)?.strip_prefix(": "));
		let Some(key) = field("source").and_then(|s| source_key(s.trim_matches(['<', '>']))) else {
			continue; // a registry or index file, which states no citable source
		};
		let stamp = ["recorded", "uploaded", "updated", "pulled"]
			.iter()
			.find_map(|f| field(f))
			.unwrap_or_else(|| panic!("{} states a source but no date", path.display()));
		let date: Date = stamp[..10].parse().unwrap_or_else(|e| panic!("{}: `{stamp}` does not open on a date: {e}", path.display()));
		let duration = field("duration").map(|d| {
			let parts: Vec<u64> = d.split(':').map(|p| p.parse().unwrap_or_else(|e| panic!("{}: duration `{d}`: {e}", path.display()))).collect();
			parts.iter().fold(0, |acc, p| acc * 60 + p)
		});
		if out.insert(key.clone(), Capture { date, duration }).is_some() {
			panic!("{key} is captured twice, second at {}", path.display());
		}
	}
	out
}

fn md_files(dir: &Path, out: &mut Vec<PathBuf>) {
	for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
		let path = entry.expect("dir entries are readable").path();
		if path.is_dir() {
			md_files(&path, out);
		} else if path.extension().is_some_and(|e| e == "md") {
			out.push(path);
		}
	}
	out.sort();
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
