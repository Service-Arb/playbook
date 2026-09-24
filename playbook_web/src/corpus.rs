use std::{collections::HashSet, fmt::Write, sync::LazyLock};

use regex::{Regex, RegexBuilder};

include!(concat!(env!("OUT_DIR"), "/corpus.rs"));

const PER_SOURCE: usize = 2;
const STRUCTURED_SOURCES: usize = 6;
const REF_SOURCES: usize = 6;
const MIN_MATCH: usize = 2; // a lone letter is noise, not relevance
const BROAD: f64 = 0.4; // a pattern hitting this share of the files in scope is an enumeration
const LINE_CAP: usize = 700;
const STRUCTURED_LINES: usize = 12;
const REF_BLOCKS: usize = 2; // paragraphs or bullets shown per chapter
const BLOCK_LINES: usize = 8;
const CHAPTER_CAP: usize = 24_000;

static CITATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\[r(\d) (\d{4}-\d{2}-\d{2})\]\([^)]+\)\s*$").unwrap());

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Tier {
	Approved,
	Suggested,
	Ref,
}

struct Doc {
	path: &'static str,
	tier: Tier,
	lines: Vec<&'static str>,
	/// per line, the index of the header it sits under
	chapter: Vec<Option<usize>>,
	/// per line, where the unit `search` scores starts: a chapter of a capture, a bullet of a note
	unit: Vec<Option<usize>>,
	/// a capture's own date; structured notes date each claim instead
	date: Option<&'static str>,
	source: Option<&'static str>,
}

pub struct Corpus {
	docs: Vec<Doc>,
}

pub struct Found {
	pub text: String,
	pub ids: Vec<String>,
}

pub static CORPUS: LazyLock<Corpus> = LazyLock::new(Corpus::load);

impl Corpus {
	fn load() -> Self {
		let mut docs: Vec<Doc> = FILES
			.iter()
			.filter(|(p, _)| !p.starts_with("skill/"))
			.map(|&(path, text)| {
				let tier = match path {
					p if p.starts_with("structured/approved/") => Tier::Approved,
					p if p.starts_with("structured/suggested/") => Tier::Suggested,
					p if p.starts_with("ref/") => Tier::Ref,
					p => unreachable!("build.rs bakes only structured/ and ref/ besides skill/: {p}"),
				};
				let lines: Vec<&str> = text.lines().collect();
				let starts = |opens: &dyn Fn(&str) -> bool| {
					let mut current = None;
					lines
						.iter()
						.enumerate()
						.map(|(i, l)| {
							if opens(l) {
								current = Some(i);
							}
							current
						})
						.collect::<Vec<_>>()
				};
				let chapter = starts(&|l| l.starts_with('#'));
				// a note and a reading of the screen are lists of separate claims; a capture's chapter is one stretch of talk
				let unit = match tier {
					Tier::Ref if !path.ends_with("/shown.md") => chapter.clone(),
					_ => starts(&|l| l.starts_with('#') || l.starts_with("- ")),
				};
				let meta = |key: &str| lines.iter().find_map(|l| l.strip_prefix(key));
				let date = ["- recorded: ", "- uploaded: ", "- updated: ", "- pulled: "].iter().find_map(|k| meta(k)).map(|d| &d[..10]);
				let source = meta("- source: ").map(|s| s.trim_matches(['<', '>']));
				Doc { path, tier, lines, chapter, unit, date, source }
			})
			.collect();
		// `shown.md` describes its recording, so it carries the recording's date and source
		for i in 0..docs.len() {
			if let Some(dir) = docs[i].path.strip_suffix("/shown.md") {
				let capture = format!("{dir}.md");
				let parent = docs.iter().find(|d| d.path == capture).unwrap_or_else(|| panic!("{} has no capture beside it", docs[i].path));
				(docs[i].date, docs[i].source) = (parent.date, parent.source);
			}
		}
		for d in docs.iter().filter(|d| d.tier == Tier::Ref) {
			let date = d.date.unwrap_or_else(|| panic!("{} states no date", d.path));
			assert!(date.as_bytes()[4] == b'-' && date.as_bytes()[7] == b'-', "{}: `{date}` is not a date", d.path);
		}
		Self { docs }
	}

	pub fn search(&self, pattern: &str, scope: Option<&str>) -> Result<Found, String> {
		let re = RegexBuilder::new(pattern).case_insensitive(true).size_limit(1 << 20).build().map_err(|e| e.to_string())?;
		if re.is_match("") {
			return Err("the pattern matches the empty string. Search for words.".into());
		}
		let scope = scope.unwrap_or("");
		if !(scope.is_empty() || scope.starts_with("structured") || scope.starts_with("ref")) {
			return Err("scope is a path prefix under `structured/` or `ref/`".into());
		}
		let in_scope: Vec<&Doc> = self.docs.iter().filter(|d| d.path.starts_with(scope)).collect();
		if in_scope.is_empty() {
			return Err(format!("nothing sits under `{scope}`"));
		}

		struct Hit<'a> {
			doc: &'a Doc,
			/// every matching line of the unit, with how many distinct terms it holds
			lines: Vec<(usize, usize)>,
			terms: HashSet<String>,
			count: usize,
		}
		impl Hit<'_> {
			/// distinct terms across the unit, then how often they occur in it
			fn score(&self) -> (usize, usize) {
				(self.terms.len(), self.count)
			}
		}
		let mut per_doc: Vec<Vec<Hit>> = Vec::new();
		for doc in &in_scope {
			let mut units: Vec<Hit> = Vec::new();
			for (i, line) in doc.lines.iter().enumerate() {
				let found: Vec<String> = re.find_iter(line).filter(|m| m.len() >= MIN_MATCH).map(|m| m.as_str().to_lowercase()).collect();
				if found.is_empty() {
					continue;
				}
				let distinct = found.iter().collect::<HashSet<_>>().len();
				let hit = match units.iter_mut().find(|h| doc.unit[h.lines[0].0] == doc.unit[i]) {
					Some(hit) => hit,
					None => {
						units.push(Hit { doc, lines: vec![], terms: HashSet::new(), count: 0 });
						units.last_mut().unwrap()
					}
				};
				hit.lines.push((i, distinct));
				hit.count += found.len();
				hit.terms.extend(found);
			}
			if !units.is_empty() {
				units.sort_by_key(|h| std::cmp::Reverse(h.score()));
				units.truncate(PER_SOURCE);
				per_doc.push(units);
			}
		}
		if in_scope.len() >= 5 && per_doc.len() as f64 > in_scope.len() as f64 * BROAD {
			return Err(format!("the pattern hits {} of the {} files in scope. Narrow it.", per_doc.len(), in_scope.len()));
		}
		if per_doc.is_empty() {
			return Ok(Found {
				text: "no match. If rephrasing finds nothing either, the corpus does not cover this: say so.".into(),
				ids: vec![],
			});
		}

		let (mut structured, mut refs): (Vec<_>, Vec<_>) = per_doc.into_iter().partition(|h| h[0].doc.tier != Tier::Ref);
		structured.sort_by_key(|h| (h[0].doc.tier, std::cmp::Reverse(h[0].score())));
		// more of the pattern's terms first; between equals the more reliable, the course, then speech over a reading of the screen
		refs.sort_by_key(|h| {
			let (terms, count) = h[0].score();
			(std::cmp::Reverse(terms), !h[0].doc.path.contains("/course/"), h[0].doc.path.ends_with("/shown.md"), std::cmp::Reverse(count), std::cmp::Reverse(h[0].doc.date))
		});
		let dropped = structured.len().saturating_sub(STRUCTURED_SOURCES) + refs.len().saturating_sub(REF_SOURCES);
		structured.truncate(STRUCTURED_SOURCES);
		refs.truncate(REF_SOURCES);
		refs.sort_by_key(|h| std::cmp::Reverse(h[0].doc.date)); // picked by relevance, shown newest first

		let mut text = String::new();
		let mut ids = Vec::new();
		for hit in structured.iter().chain(&refs).flatten() {
			let doc = hit.doc;
			let first = hit.lines[0].0;
			let header = doc.chapter[first].map_or("", |c| doc.lines[c]);
			let end = doc.chapter_end(first);
			let blocks: Vec<(usize, usize)> = match doc.tier {
				Tier::Ref => {
					let mut body: Vec<(usize, usize)> = hit.lines.iter().copied().filter(|(l, _)| !doc.lines[*l].starts_with('#')).collect();
					body.sort_by_key(|&(l, distinct)| (std::cmp::Reverse(distinct), l));
					let mut shown: Vec<usize> = body.iter().take(REF_BLOCKS).map(|&(l, _)| l).collect();
					if shown.is_empty() {
						// a chapter that matched only in its header shows its opening paragraph
						shown.push((first..end).find(|&i| !doc.lines[i].starts_with('#') && !doc.lines[i].trim().is_empty()).unwrap_or(first));
					}
					let mut blocks: Vec<(usize, usize)> = shown.into_iter().map(|l| doc.block(l, end)).collect();
					blocks.sort();
					blocks.dedup();
					blocks
				}
				_ => {
					let from = doc.unit[first].filter(|&u| !doc.lines[u].starts_with('#')).unwrap_or(first);
					let to = (from + 1..end).find(|&i| doc.unit[i] != doc.unit[first]).unwrap_or(end);
					vec![(from, to.min(from + STRUCTURED_LINES))]
				}
			};
			let at = blocks[0].0;
			writeln!(text, "{}:{} · {header} · {}", doc.path, at + 1, doc.reliability(first)).unwrap();
			if let Some(source) = doc.source {
				writeln!(text, "source: {source}").unwrap();
			}
			for (from, to) in blocks {
				for i in from..to {
					writeln!(text, "{:>5} | {}", i + 1, clip(doc.lines[i], &re)).unwrap();
				}
			}
			text.push('\n');
			ids.push(format!("{}:{}", doc.path, at + 1));
		}
		if dropped > 0 {
			writeln!(text, "{dropped} more files matched. Narrow the pattern, or pass a `scope`, to reach them.").unwrap();
		}
		Ok(Found { text, ids })
	}

	pub fn read(&self, path: &str, header: &str, line: Option<usize>) -> Result<Found, String> {
		let doc = self.docs.iter().find(|d| d.path == path).ok_or_else(|| format!("no file `{path}`; take paths from `search`"))?;
		let start = match line {
			Some(l) => {
				let l = l.checked_sub(1).filter(|l| *l < doc.lines.len()).ok_or_else(|| format!("{path} has {} lines", doc.lines.len()))?;
				doc.chapter[l].ok_or_else(|| format!("line {} of {path} sits above its first header", l + 1))?
			}
			None => {
				let want = header.to_lowercase();
				let found: Vec<usize> = (0..doc.lines.len()).filter(|&i| doc.lines[i].starts_with('#') && doc.lines[i].to_lowercase().contains(&want)).collect();
				match found[..] {
					[one] => one,
					[] => return Err(format!("no header in {path} holds `{header}`. Its headers:\n{}", doc.headers())),
					_ => {
						let list: Vec<String> = found.iter().map(|&i| format!("line {}: {}", i + 1, doc.lines[i])).collect();
						return Err(format!("`{header}` is ambiguous in {path}; pass the `line` of one:\n{}", list.join("\n")));
					}
				}
			}
		};
		let end = doc.chapter_end(start);
		let mut text = format!("{path}:{} · {}\n", start + 1, doc.reliability(start));
		if let Some(source) = doc.source {
			writeln!(text, "source: {source}").unwrap();
		}
		for i in start..end {
			writeln!(text, "{:>5} | {}", i + 1, doc.lines[i]).unwrap();
			if text.len() > CHAPTER_CAP {
				writeln!(text, "… the chapter runs to line {end}. `search` inside it, with scope `{path}`, for the rest.").unwrap();
				break;
			}
		}
		Ok(Found { text, ids: vec![format!("{path}:{}", start + 1)] })
	}

	pub fn guide(&self, section: &str) -> Result<Found, String> {
		let path = format!("skill/service-arb/sections/{section}.md");
		FILES
			.iter()
			.find(|(p, _)| *p == path)
			.map(|(_, text)| Found { text: text.to_string(), ids: vec![section.to_owned()] })
			.ok_or_else(|| format!("no section `{section}`. {GUIDE_DESCRIPTION}"))
	}
}

impl Doc {
	/// the line after the last one of the chapter holding `line`
	fn chapter_end(&self, line: usize) -> usize {
		(line + 1..self.lines.len()).find(|&i| self.lines[i].starts_with('#')).unwrap_or(self.lines.len())
	}

	/// the paragraph or bullet holding `line`, within a chapter ending at `end`
	fn block(&self, line: usize, end: usize) -> (usize, usize) {
		let opens = |i: usize| self.lines[i].starts_with("- ") || self.lines[i - 1].trim().is_empty() || self.lines[i - 1].starts_with('#');
		let from = (1..=line).rev().find(|&i| opens(i)).unwrap_or(0);
		let to = (line + 1..end).find(|&i| self.lines[i].trim().is_empty() || self.lines[i].starts_with("- ") || self.lines[i].starts_with('#')).unwrap_or(end);
		(from, to.min(from + BLOCK_LINES).max(line + 1))
	}

	fn headers(&self) -> String {
		self.lines.iter().enumerate().filter(|(_, l)| l.starts_with('#')).map(|(i, l)| format!("line {}: {l}", i + 1)).collect::<Vec<_>>().join("\n")
	}

	/// what a claim at `line` is worth, and when it was said
	fn reliability(&self, line: usize) -> String {
		match self.tier {
			Tier::Ref if self.path.contains("/course/") => format!("r1 course · {}", self.date.expect("asserted on load")),
			Tier::Ref => format!("r2–7 by who speaks · {}", self.date.expect("asserted on load")),
			_ => (line..self.chapter_end(line))
				.find_map(|i| CITATION.captures(self.lines[i]))
				.map_or("unsourced".into(), |c| format!("r{} {}", &c[1], &c[2])),
		}
	}
}

/// A transcript paragraph is one line and can run to kilobytes; keep the part around the match.
fn clip(line: &str, re: &Regex) -> String {
	if line.len() <= LINE_CAP {
		return line.to_owned();
	}
	let (from, to) = match re.find(line) {
		Some(m) => (m.start().saturating_sub(LINE_CAP / 2), (m.end() + LINE_CAP / 2).min(line.len())),
		None => (0, LINE_CAP),
	};
	let from = (0..=from).rev().find(|&i| line.is_char_boundary(i)).unwrap();
	let to = (to..=line.len()).find(|&i| line.is_char_boundary(i)).unwrap();
	format!("{}{}{}", if from > 0 { "…" } else { "" }, &line[from..to], if to < line.len() { "…" } else { "" })
}
