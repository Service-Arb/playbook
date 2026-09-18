#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
jiff = "0.2"
---

//! `./scripts/chatgpt-pull.rs [<share-url-or-id>...]` — write `ref/research/<id>.md` for every
//! chatgpt share we do not have yet. With no arguments it takes the links already written down
//! in `ref/README.md`.
//!
//! The share page ships its conversation as an interned react-flight blob and the `/backend-api`
//! route is behind cloudflare, so the page is rendered in headless chromium and the turns are read
//! out of the DOM. A short window virtualises the thread and silently drops the turns that never
//! scroll into view — hence the tall one.
//!
//! A chatgpt share states no date anywhere, so the pull date is the only age this can carry, and
//! the raw file is the thing to re-pull when that age starts to matter.

use std::{
	collections::BTreeSet,
	path::{Path, PathBuf},
	process::Command,
};

const SHARE: &str = "https://chatgpt.com/share/";
const ROLE: &str = "data-message-author-role=\"";
const DISCLAIMER: &str = "data-testid=\"thread-disclaimer\"";

fn main() {
	let root = repo_root();
	let out_dir = root.join("ref/research");
	std::fs::create_dir_all(&out_dir).expect("ref/research is ours to create");

	let args: Vec<String> = std::env::args().skip(1).collect();
	let wanted: BTreeSet<String> = match args.is_empty() {
		true => links_in(&root.join("ref/README.md")),
		false => args.iter().map(|a| id_of(a)).collect(),
	};
	if wanted.is_empty() {
		eprintln!("no chatgpt share links in ref/README.md and none given — nothing to pull");
		return;
	}

	for id in &wanted {
		let out = out_dir.join(format!("{id}.md"));
		if out.exists() {
			eprintln!("have {id}");
			continue;
		}
		eprintln!("rendering {id}");
		let dom = render(&format!("{SHARE}{id}"));
		std::fs::write(&out, document(id, &dom)).expect("ref/research is ours to write");
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

/// chatgpt renders the thread windowed, so a turn that never enters the viewport is never in the
/// DOM. The window is made taller than any conversation rather than scripting a scroll.
fn render(url: &str) -> String {
	let out = Command::new("chromium")
		.args([
			"--headless",
			"--disable-gpu",
			"--no-sandbox",
			"--window-size=1280,20000",
			"--virtual-time-budget=60000",
			"--dump-dom",
			url,
		])
		.output()
		.unwrap_or_else(|e| panic!("chromium: {e}"));
	let dom = String::from_utf8(out.stdout).expect("chromium dumps utf-8");
	if !dom.contains(ROLE) {
		panic!("{url} rendered without a single message — the share is private, deleted, or chatgpt's DOM moved");
	}
	dom
}

fn document(id: &str, dom: &str) -> String {
	let title = tag_text(dom, "<title>").unwrap_or_else(|| panic!("{id} rendered without a title"));
	let title = title.strip_suffix(" - ChatGPT").unwrap_or(&title);
	let today = jiff::Zoned::now().date();

	let mut out = format!(
		"# {title}\n\
		 \n\
		 - source: <{SHARE}{id}>\n\
		 - pulled: {today} (chatgpt states no date of its own — re-pull to re-date)\n\
		 - pulled by: `scripts/chatgpt-pull.rs`\n\
		 \n"
	);
	//LOOP: one pass over the turns, which the DOM lists in order
	for turn in dom.split(ROLE).skip(1) {
		let role = turn.split('"').next().expect("split yields at least once");
		let body = turn.split_once('>').expect("an attribute sits inside a tag").1;
		// a turn ends where the next one begins; after the last one sits the page itself, and the
		// disclaimer is the first thing chatgpt puts below the thread
		let body = body.split(DISCLAIMER).next().expect("split yields at least once");
		out.push_str(&format!("## {role}\n\n{}\n\n", text_of(body)));
	}
	out
}

fn tag_text(dom: &str, open: &str) -> Option<String> {
	let at = dom.find(open)? + open.len();
	let rest = &dom[at..];
	Some(unescape(&rest[..rest.find('<')?]))
}

/// The DOM flattened to the little markdown a turn actually carries. Anything not named here is
/// chatgpt's own chrome, and is meant to come out as nothing.
fn text_of(html: &str) -> String {
	let mut out = String::new();
	let mut rest = html;
	//LOOP: consumes at least one byte per turn of the loop
	while let Some(at) = rest.find('<') {
		out.push_str(&rest[..at]);
		let tail = &rest[at..];
		// a turn is cut out of the DOM mid-tag at both ends, so the last `<` of the slice can have no
		// `>` — and half a tag is not text
		let Some(close) = tail.find('>') else {
			rest = "";
			break;
		};
		let tag = &tail[1..close];
		let name = tag.trim_start_matches('/').split([' ', '>', '/']).next().expect("split yields at least once").to_ascii_lowercase();
		if (name == "script" || name == "style") && !tag.starts_with('/') {
			let end = format!("</{name}>");
			// an unclosed script is the end of anything readable, so there is nothing after it to keep
			let Some(at) = tail.find(&end) else {
				rest = "";
				break;
			};
			rest = &tail[at + end.len()..];
			continue;
		}
		match name.as_str() {
			"li" if !tag.starts_with('/') => out.push_str("\n- "),
			"code" | "pre" => out.push('`'),
			"td" | "th" if !tag.starts_with('/') => out.push_str(" | "),
			"br" | "p" | "div" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote" | "ul" | "ol" | "table" => out.push('\n'),
			_ => {}
		}
		rest = &tail[close + 1..];
	}
	out.push_str(rest);

	let mut text = String::new();
	//LOOP: one pass over the flattened lines
	for line in unescape(&out).lines() {
		let line = line.trim();
		// a blank line is a paragraph break, and any run of them is the same one break
		if line.is_empty() {
			if !text.ends_with("\n\n") && !text.is_empty() && !text.ends_with("-\n") {
				text.push('\n');
			}
			continue;
		}
		// chatgpt's own turn separators, which say nothing a `##` heading has not already said
		if line == "ChatGPT said:" || line == "You said:" {
			continue;
		}
		// a list item wraps its text in a block of its own, so the bullet lands on its own line first
		if text.ends_with("-\n") {
			text.truncate(text.len() - 1);
			text.push(' ');
		}
		text.push_str(line);
		text.push('\n');
	}
	text.trim().to_string()
}

fn unescape(s: &str) -> String {
	// the entities chatgpt's DOM actually emits; a numeric one it does not, and a `&` left alone is
	// a `&` the page meant literally
	s.replace("&nbsp;", " ")
		.replace("&lt;", "<")
		.replace("&gt;", ">")
		.replace("&quot;", "\"")
		.replace("&#x27;", "'")
		.replace("&#39;", "'")
		.replace("&amp;", "&")
}
