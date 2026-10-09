//! Bakes the corpus and the connect pages' stylesheet into the binary: the server reads nothing from
//! disk per request, and the image carries only what is listed here (docs/ARCHITECTURE.md, Invariants).

use std::{
	fmt::Write,
	fs,
	path::{Path, PathBuf},
	process::Command,
};

use sha2::{Digest, Sha256};

const INSTRUCTIONS_MAX: usize = 2048; // Claude Code truncates server instructions past this

fn main() {
	let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).parent().expect("playbook_web/ sits in the workspace root").to_path_buf();
	let mut files = Vec::new();
	for dir in ["skill/service-arb/sections", "structured/approved", "structured/suggested", "ref"] {
		println!("cargo:rerun-if-changed={}", root.join(dir).display());
		walk(&root.join(dir), &mut files);
	}
	let skill = root.join("skill/service-arb/SKILL.md");
	println!("cargo:rerun-if-changed={}", skill.display());
	files.sort();

	let mut out = String::from("pub static FILES: &[(&str, &str)] = &[\n");
	let mut sections = Vec::new();
	for f in &files {
		let rel = f.strip_prefix(&root).unwrap().to_str().expect("corpus paths are utf-8");
		if let Some(name) = rel.strip_prefix("skill/service-arb/sections/") {
			sections.push(name.trim_end_matches(".md").to_owned());
		}
		writeln!(out, "\t({rel:?}, include_str!({:?})),", f.display().to_string()).unwrap();
	}
	out.push_str("];\n");

	// the image carries no scripts, so the decay the skill links to is inlined
	let cite_check = root.join("scripts/cite-check.rs");
	println!("cargo:rerun-if-changed={}", cite_check.display());
	let cite_check = fs::read_to_string(&cite_check).unwrap();
	let decay_days = cite_check
		.lines()
		.find_map(|l| l.strip_prefix("const DECAY_DAYS: f64 = ")?.strip_suffix(".;"))
		.expect("cite-check.rs states `const DECAY_DAYS: f64 = <n>.;`");
	let skill = fs::read_to_string(&skill).unwrap();
	let instructions = format!("{}\n\n{}", section(&skill, "# service-arb"), section(&skill, "## Answering"));
	let decay_link = "[`DECAY_DAYS`](../../scripts/cite-check.rs)";
	assert!(instructions.contains(decay_link), "SKILL.md's `## Answering` no longer links {decay_link}");
	let instructions = instructions.replace(decay_link, decay_days);
	assert!(instructions.len() <= INSTRUCTIONS_MAX, "SKILL.md's hook + `## Answering` is {} chars, over {INSTRUCTIONS_MAX}", instructions.len());
	writeln!(out, "pub const INSTRUCTIONS: &str = {instructions:?};").unwrap();
	writeln!(out, "pub const GUIDE_DESCRIPTION: &str = {:?};", format!(
		"How to decide, for one area: what matters, how to choose, what to do when it goes wrong. Read the matching one before advising rather than quoting. Sections: {}.",
		sections.join(", ")
	))
	.unwrap();

	let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
	fs::write(out_dir.join("corpus.rs"), out).unwrap();
	stylesheet(&root.join("playbook_web/src/pages.rs"), &out_dir);
}

/// Tailwind over the kit's class inventory and `pages.rs`, under a name that changes with it, so it is cached for good.
/// The inventory and tokens are written out because tailwind can neither scan nor `@import` a crate unpacked from crates.io.
fn stylesheet(pages: &Path, out: &Path) {
	println!("cargo:rerun-if-changed={}", pages.display());
	fs::write(out.join("uikit-classes.txt"), ev_lib_classes::CLASS_INVENTORY).unwrap();
	fs::write(out.join("tokens.css"), ev_lib_classes::TOKENS_CSS).unwrap();
	let entry = format!("@import \"tailwindcss\";\n@import \"./tokens.css\";\n@source \"./uikit-classes.txt\";\n@source {:?};\n", pages.display().to_string());
	fs::write(out.join("entry.css"), entry).unwrap();
	let css = out.join("connect.css");
	let status = Command::new("tailwindcss")
		.arg("-i")
		.arg(out.join("entry.css"))
		.arg("-o")
		.arg(&css)
		.arg("--minify")
		.status()
		.expect("tailwindcss (v4) is on PATH: the dev shell and the nix build carry it");
	assert!(status.success(), "tailwindcss failed on {}", out.join("entry.css").display());
	let hash = Sha256::digest(fs::read(&css).unwrap());
	let hash: String = hash[..8].iter().map(|b| format!("{b:02x}")).collect();
	fs::write(
		out.join("stylesheet.rs"),
		format!("pub(crate) const CSS: &str = include_str!({:?});\npub(crate) const CSS_FILE: &str = \"connect.{hash}.css\";\n", css.display().to_string()),
	)
	.unwrap();
}

/// The text under `header`, up to the next header.
fn section<'a>(md: &'a str, header: &str) -> &'a str {
	let start = md.find(&format!("\n{header}\n")).unwrap_or_else(|| panic!("SKILL.md has no `{header}`")) + header.len() + 2;
	let rest = &md[start..];
	rest[..rest.find("\n#").unwrap_or(rest.len())].trim()
}

/// `README.md`s are indices and format rules, not knowledge. `ref/cases/` states no source and no date.
fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
	for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
		let path = entry.unwrap().path();
		if path.ends_with("ref/cases") {
			continue;
		}
		if path.is_dir() {
			walk(&path, files);
		} else if path.extension().is_some_and(|e| e == "md") && path.file_name().unwrap() != "README.md" {
			files.push(path);
		}
	}
}
