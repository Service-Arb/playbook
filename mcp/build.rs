//! Bakes the corpus into the binary: the server reads nothing from disk per request, and the image
//! carries only what is listed here (docs/ARCHITECTURE.md, Invariants).

use std::{
	fmt::Write,
	fs,
	path::{Path, PathBuf},
};

const INSTRUCTIONS_MAX: usize = 2048; // Claude Code truncates server instructions past this

fn main() {
	let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("mcp/ sits in the workspace root").to_path_buf();
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

	let skill = fs::read_to_string(&skill).unwrap();
	let instructions = format!("{}\n\n{}", section(&skill, "# service-arb"), section(&skill, "## Answering"));
	assert!(instructions.len() <= INSTRUCTIONS_MAX, "SKILL.md's hook + `## Answering` is {} chars, over {INSTRUCTIONS_MAX}", instructions.len());
	writeln!(out, "pub const INSTRUCTIONS: &str = {instructions:?};").unwrap();
	writeln!(out, "pub const GUIDE_DESCRIPTION: &str = {:?};", format!(
		"How to decide, for one area: what matters, how to choose, what to do when it goes wrong. Read the matching one before advising rather than quoting. Sections: {}.",
		sections.join(", ")
	))
	.unwrap();

	fs::write(PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("corpus.rs"), out).unwrap();
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
