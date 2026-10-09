#!/usr/bin/env -S cargo -Zscript -q
---cargo
[package]
edition = "2024"

[dependencies]
serde_json = "1"
ureq = "3"
---

//! `./scripts/call-pull.rs [<share-url>...]` — write `ref/<platform>/<date>-<title>.md` for every
//! loom or fathom recording we do not have yet. With no arguments it takes the links already written
//! down in `ref/README.md`, so adding a recording to the playbook is pasting its link there.
//! `./scripts/call-pull.rs --check` only holds every capture on disk to the shape, and fails loudly on
//! the first that breaks it.
//!
//! The shape is `docs/ARCHITECTURE.md`'s "Call captures"; this script is its only enforcement.
//!
//! Both platforms' share pages are server-rendered and public, so nothing here needs an account. A
//! loom page carries a *signed* URL for its transcript CDN; the signature expires, which is why it is
//! read out of the page on every run rather than kept.
//!
//! The recording is fetched beside its capture, as `<capture>/recording.<ext>`, and never committed:
//! the host keeps it. Naming a captured recording's link fetches it again, for whatever reads its
//! picture. A recording its platform never transcribed is transcribed here with whisper, from that file.

use std::{
	collections::BTreeMap,
	path::{Path, PathBuf},
	process::Command,
};

const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const LOOM_TRANSCRIPT_CDN: &str = "https://cdn.loom.com/mediametadata/transcription/";
const WHISPER_MODEL: &str = ".local/share/whisper-cpp/models/ggml-base.en.bin";
/// Chapters whose last starts further than this before the last words are taken to have stopped.
const TAIL_SECS: f64 = 600.;
/// A paragraph is closed at the first sentence end past this many words.
const PARAGRAPH_WORDS: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Platform {
	Loom,
	Fathom,
	/// files in a shared folder; the folder is what gets registered, and what names and dates its files
	Drive,
	/// players that publish neither a transcript nor a title worth the name; the registry line names them
	Vimeo,
	Vocaroo,
}

impl Platform {
	const ALL: [Platform; 5] = [Platform::Loom, Platform::Fathom, Platform::Drive, Platform::Vimeo, Platform::Vocaroo];

	fn share(self) -> &'static str {
		match self {
			Platform::Loom => "https://www.loom.com/share/",
			Platform::Fathom => "https://fathom.video/share/",
			Platform::Drive => "https://drive.google.com/file/d/",
			Platform::Vimeo => "https://player.vimeo.com/video/",
			Platform::Vocaroo => "https://vocaroo.com/",
		}
	}

	fn dir(self) -> &'static str {
		match self {
			Platform::Loom => "ref/loom",
			Platform::Fathom => "ref/fathom",
			Platform::Drive => "ref/drive",
			Platform::Vimeo => "ref/vimeo",
			Platform::Vocaroo => "ref/vocaroo",
		}
	}

	fn name(self) -> &'static str {
		match self {
			Platform::Loom => "loom",
			Platform::Fathom => "fathom",
			Platform::Drive => "drive",
			Platform::Vimeo => "vimeo",
			Platform::Vocaroo => "vocaroo",
		}
	}

	/// What goes between a share link and the second it seeks to.
	fn seek(self) -> &'static str {
		match self {
			Platform::Vimeo => "#t=",
			// vocaroo seeks to nothing and ignores it, which leaves the stamp still reading right
			Platform::Loom | Platform::Fathom | Platform::Drive | Platform::Vocaroo => "?t=",
		}
	}
}

/// A recording, as `(platform, id)` — the id being whatever follows the platform's share prefix.
type Rec = (Platform, String);

fn rec_of(url: &str) -> Rec {
	let platform = Platform::ALL
		.into_iter()
		.find(|p| url.starts_with(p.share()))
		.unwrap_or_else(|| panic!("`{url}` is not a share link of any platform this pulls"));
	let id = url[platform.share().len()..].split(['?', '#', '/']).next().expect("split yields at least once");
	assert!(!id.is_empty(), "`{url}` names no recording");
	(platform, id.to_string())
}

/// What a platform gave us, before it is laid out as a capture.
struct Recording {
	title: String,
	recorded: String,
	duration: f64,
	summary: Option<String>,
	chapters: Option<Vec<(u64, String)>>,
	/// `(start secs, text)`. Speaker turns when `turns`, fragments of speech to be joined otherwise.
	phrases: Vec<(f64, String)>,
	turns: bool,
	transcribed_by: String,
	/// where the recording sits, when that is not its own share page
	within: Option<String>,
}

/// What something other than the recording's own page says of it — a drive folder's listing, or the
/// registry line — for recordings whose page says neither what they are nor when.
struct Listed {
	title: String,
	/// a date, then what kind of date it is
	recorded: String,
	within: Option<String>,
}

const DRIVE_FOLDERS: [&str; 2] = ["https://drive.google.com/drive/folders/", "https://drive.google.com/drive/u/0/folders/"];

fn main() {
	let root = repo_root();
	let args: Vec<String> = std::env::args().skip(1).collect();
	let have = captured(&root);
	if args.iter().any(|a| a == "--check") {
		assert_eq!(args.len(), 1, "--check takes nothing else");
		eprintln!("{} call captures hold their shape", have.len());
		return;
	}
	let wanted: BTreeMap<Rec, Option<Listed>> = match args.is_empty() {
		true => links_in(&root.join("ref/README.md")),
		false => args.iter().map(|a| (rec_of(a), None)).collect(),
	};
	if wanted.is_empty() {
		eprintln!("no recording links in ref/README.md and none given — nothing to pull");
		return;
	}

	let mut undigested = Vec::new();
	for (rec, listed) in &wanted {
		let (platform, id) = rec;
		let url = format!("{}{id}", platform.share());
		if let Some(had) = have.get(rec) {
			// a new file beside the capture, which is left as it is
			let kept = had.with_extension("");
			match recording_in(&kept) {
				// a registry run is after new captures, and the recordings of old ones are the host's
				_ if args.is_empty() => (),
				Some(_) => eprintln!("have {id} — {}", had.display()),
				None => {
					eprintln!("fetching the recording of {}", had.display());
					fetch(&url, &kept);
				}
			}
			continue;
		}
		eprintln!("pulling {} {id}", platform.name());
		// named only once the recording is read, so it waits outside the tree till then
		let staged = root.join("tmp/call-pull").join(id);
		let media = fetch(&url, &staged);
		let mut recording = match platform {
			Platform::Loom => loom(id, &get(&url)),
			Platform::Fathom => fathom(&get(&url)),
			Platform::Drive | Platform::Vimeo | Platform::Vocaroo => {
				let listed = listed.as_ref().unwrap_or_else(|| panic!("{id}: a {} recording is pulled through what ref/README.md says of it", platform.name()));
				heard(*platform, listed, &media)
			}
		};
		if recording.phrases.is_empty() && matches!(platform, Platform::Loom | Platform::Fathom) {
			eprintln!("  {} transcribed nothing", platform.name());
			recording.transcribed_by = format!("whisper-cpp `{}` — {} transcribed nothing", model_name(), platform.name());
			recording.phrases = whisper_phrases(&media).0;
			assert!(!recording.phrases.is_empty(), "{id}: whisper heard nothing either");
			recording.turns = false;
			// the platform's reading, if any, is of a transcript it never produced
			recording.summary = None;
			recording.chapters = None;
		}
		let (name, doc, digested) = render(*platform, id, recording);
		check(&name, &doc);
		let dir = root.join(platform.dir());
		std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("creating {}: {e}", dir.display()));
		let out = dir.join(&name);
		assert!(!out.exists(), "{} is taken, and not by {id} — two recordings share a day and a title", out.display());
		std::fs::rename(&staged, out.with_extension("")).unwrap_or_else(|e| panic!("moving {} beside {}: {e}", staged.display(), out.display()));
		std::fs::write(&out, doc).unwrap_or_else(|e| panic!("writing {}: {e}", out.display()));
		println!("{}", out.display());
		if !digested {
			undigested.push(out);
		}
	}

	if !undigested.is_empty() {
		eprintln!("\nnobody has read these yet — run `/call-digest` over each:");
		for out in &undigested {
			eprintln!("  {}", out.display());
		}
	}
}

/// The script is only ever run from a checkout, and every path it touches is relative to the root
/// of one — so finding it is the first thing, and failing to is not something to guess past.
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

/// Every capture already on disk, by the recording it states, each held to the shape on the way. A
/// file is named after the recording rather than after its id, so what is on disk is a question for
/// the files and not for their names.
fn captured(root: &Path) -> BTreeMap<Rec, PathBuf> {
	let mut found = BTreeMap::new();
	for platform in Platform::ALL {
		let dir = root.join(platform.dir());
		if !dir.exists() {
			continue;
		}
		//LOOP: bounded by the number of files in a finite directory
		for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
			let path = entry.expect("a directory entry is readable").path();
			if path.is_dir() {
				assert!(path.with_extension("md").exists(), "{} sits beside no capture", path.display());
				continue;
			}
			let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
			let rec = check(&path.display().to_string(), &text);
			verbatim(root, &path, &text);
			assert_eq!(rec.0, platform, "{} is a {} capture filed under {}", path.display(), rec.0.name(), platform.dir());
			if let Some(clash) = found.insert(rec.clone(), path.clone()) {
				panic!("{} and {} both claim {}", clash.display(), path.display(), rec.1);
			}
		}
	}
	found
}

/// A digest only moves headers, so its prose has to be the prose of the capture it was run on — the
/// committed one. An uncommitted pull has nothing to be held to, which is why pulls are committed first.
fn verbatim(root: &Path, path: &Path, text: &str) {
	let prose = |t: &str| -> Vec<String> {
		let body = t.split_once("\n## transcript\n").expect("checked to have one").1;
		body.lines().filter(|l| !l.trim().is_empty() && !l.starts_with("### ")).map(str::to_string).collect()
	};
	if !text.lines().any(|l| l.starts_with("- read by: `/")) {
		return; // a fresh pull is the platform's text by construction; a re-pull may differ from the last one
	}
	let rel = path.strip_prefix(root).expect("captures live under the root");
	let out = Command::new("git").arg("-C").arg(root).arg("show").arg(format!("HEAD:{}", rel.display())).output().expect("git runs");
	if !out.status.success() {
		return; // not committed yet, so nothing to hold it to
	}
	let committed = String::from_utf8(out.stdout).expect("captures are utf-8");
	if !committed.contains("\n## transcript\n") {
		return; // committed in a shape before this one, and re-pulled since
	}
	let (now, then) = (prose(text), prose(&committed));
	if let Some((i, (a, b))) = now.iter().zip(&then).enumerate().find(|(_, (a, b))| a != b) {
		panic!("{}: paragraph {} of the transcript is not what was committed —\n  now: {a}\n  was: {b}", path.display(), i + 1);
	}
	assert_eq!(now.len(), then.len(), "{}: the transcript has {} paragraphs, and {} were committed", path.display(), now.len(), then.len());
}

/// Hold a capture to `docs/ARCHITECTURE.md`'s "Call captures", returning the recording it states.
fn check(name: &str, text: &str) -> Rec {
	let field = |key: &str| {
		text.lines()
			.find_map(|l| l.strip_prefix(&format!("- {key}: ")))
			.unwrap_or_else(|| panic!("{name} states no `{key}:` — it was not written by this script"))
	};
	let (platform, id) = rec_of(field("source").trim_start_matches('<').trim_end_matches('>'));
	let duration = secs(field("duration"));
	let silent = field("transcribed by").ends_with("— nobody speaks");
	let read_by = field("read by");
	let unchaptered = read_by.contains("no chapters");

	let sections: Vec<&str> = text.lines().filter(|l| l.starts_with("## ")).collect();
	assert_eq!(sections.last(), Some(&"## transcript"), "{name}: `## transcript` has to be its last section");
	assert!(!sections.contains(&"## chapters"), "{name}: chapters are `###` headers inside `## transcript`, not a section");
	let transcript = text.split_once("\n## transcript\n").expect("found as a section above").1;

	let lines: Vec<&str> = transcript.lines().collect();
	if let Some(w) = lines.windows(2).find(|w| !w[0].trim().is_empty() && !w[1].trim().is_empty()) {
		panic!("{name}: `{}` runs straight into the next line — paragraphs and headers are a blank line apart", w[0]);
	}

	let link = format!("]({}{id}{}", platform.share(), platform.seek());
	let mut chapters = Vec::new();
	let mut last = None;
	let mut first = true;
	//LOOP: bounded by the lines of a finite file
	for line in transcript.lines().filter(|l| !l.trim().is_empty()) {
		let Some(header) = line.strip_prefix("### [") else {
			assert!(!first, "{name}: transcript text before the first chapter header");
			assert!(!line.contains(&link), "{name}: only chapter headers carry a timestamp — `{line}`");
			continue;
		};
		first = false;
		let (stamp, rest) = header.split_once(&link).unwrap_or_else(|| panic!("{name}: `{line}` is not `### [MM:SS]({}{id}?t=<secs>) <title>`", platform.share()));
		let (t, title) = rest.split_once(')').unwrap_or_else(|| panic!("{name}: `{line}` never closes its link"));
		let t: u64 = t.parse().unwrap_or_else(|e| panic!("{name}: `{line}` links to a second that is not one: {e}"));
		assert_eq!(secs(stamp), t, "{name}: `{line}` says one time and links to another");
		assert!(t <= duration, "{name}: `{line}` is past the end of the recording");
		assert!(last < Some(t), "{name}: `{line}` does not come after the chapter before it");
		assert!(unchaptered || !title.trim().is_empty(), "{name}: `{line}` has no title, and the capture says it is chaptered");
		last = Some(t);
		chapters.push((t, title.trim().to_string()));
	}
	assert_eq!(first, silent, "{name}: a transcript is empty exactly when `transcribed by:` ends `— nobody speaks`");

	// a platform's own summary is verbatim and need not agree with its chapters; ours has to
	if read_by.starts_with("`/") {
		let summary = text.split_once("\n## summary\n").unwrap_or_else(|| panic!("{name}: digested, and no `## summary`")).1;
		let summary = summary.split_once("\n## ").expect("`## transcript` follows").0;
		let topics: Vec<(u64, String)> = summary
			.lines()
			.filter_map(|l| l.strip_prefix("### "))
			.map(|l| {
				let (topic, at) = l.trim().rsplit_once(' ').unwrap_or_else(|| panic!("{name}: `### {l}` is not `### <topic> <stamp>`"));
				(secs(at), topic.to_string())
			})
			.collect();
		assert_eq!(topics, chapters, "{name}: the summary's topics and the transcript's chapters differ");
	}
	(platform, id)
}

/// The registry is `ref/README.md` and nothing else — a capture quotes its own source URL, and a
/// transcript quotes every link that was said out loud, so scanning the tree would make the
/// pullers feed on their own output.
fn links_in(readme: &Path) -> BTreeMap<Rec, Option<Listed>> {
	let text = std::fs::read_to_string(readme).unwrap_or_else(|e| panic!("reading {}: {e}", readme.display()));
	let link_at = |at: usize| text[at..].split_whitespace().next().expect("split yields at least once").trim_end_matches([')', ']', ',', '.', '"', '`', '>']);
	let mut found = BTreeMap::new();
	for platform in Platform::ALL {
		//LOOP: bounded by the number of occurrences in a finite file
		for (at, _) in text.match_indices(platform.share()) {
			let rec = rec_of(link_at(at));
			let listed = match platform {
				Platform::Loom | Platform::Fathom | Platform::Drive => None,
				Platform::Vimeo | Platform::Vocaroo => Some(registered(&text, at, &rec)),
			};
			found.insert(rec, listed);
		}
	}
	for prefix in DRIVE_FOLDERS {
		//LOOP: bounded by the number of occurrences in a finite file
		for (at, _) in text.match_indices(prefix) {
			let folder = link_at(at)[prefix.len()..].split(['?', '#', '/']).next().expect("split yields at least once");
			for (id, listed) in drive_folder(folder) {
				found.insert((Platform::Drive, id), Some(listed));
			}
		}
	}
	found
}

/// A recording as its registry line has it: `- [<date>](<url>) — <what it is>`. The date is the day
/// the link was found, unless the host itself publishes one.
fn registered(text: &str, at: usize, (platform, id): &Rec) -> Listed {
	let line = text[..at].rsplit('\n').next().expect("rsplit yields at least once").to_string() + text[at..].split('\n').next().expect("split yields at least once");
	let shape = || panic!("register {} links as `- [<date>](<url>) — <what it is>`, not `{line}`", platform.name());
	let date = line.split_once("- [").unwrap_or_else(shape).1.split(']').next().expect("split yields at least once");
	let title = line.split_once(") — ").unwrap_or_else(shape).1.trim();
	let json = Command::new("yt-dlp").args(["-q", "--no-warnings", "-J"]).arg(format!("{}{id}", platform.share())).output().expect("yt-dlp runs");
	assert!(json.status.success(), "yt-dlp could not read {id}: {}", String::from_utf8_lossy(&json.stderr));
	let meta: serde_json::Value = serde_json::from_slice(&json.stdout).expect("yt-dlp -J prints json");
	let recorded = match meta["upload_date"].as_str() {
		Some(d) => format!("{}-{}-{} — uploaded, as {} has it", &d[..4], &d[4..6], &d[6..], platform.name()),
		None => format!("{date} — first seen, as the registry has it; {} states no date", platform.name()),
	};
	Listed { title: title.to_string(), recorded, within: None }
}

/// A shared folder's recordings, read off the embeddable view drive serves without a session. What
/// is not a recording is printed for the registry, the way any link a puller finds is.
fn drive_folder(folder: &str) -> Vec<(String, Listed)> {
	let url = format!("https://drive.google.com/drive/folders/{folder}");
	let page = get(&format!("https://drive.google.com/embeddedfolderview?id={folder}"));
	let unescape = |s: &str| s.replace("&amp;", "&").replace("&#39;", "'").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">");
	let folder_title = unescape(between(&page, "<title>", '<', "the folder's title").trim());
	let mut out = Vec::new();
	//LOOP: bounded by the entries of a finite page
	for entry in page.split("<div class=\"flip-entry\" id=\"entry-").skip(1) {
		let id = entry.split('"').next().expect("split yields at least once").to_string();
		let mime = between(entry, "/type/", '"', "a file's type");
		let title = unescape(between(entry, "flip-entry-title\">", '<', "a file's title").trim());
		let modified = between(entry, "flip-entry-last-modified\"><div>", '<', "a file's date");
		if !(mime.starts_with("audio/") || mime.starts_with("video/")) {
			eprintln!("not a recording, not pulled — {folder_title} / {title} ({mime}): https://drive.google.com/file/d/{id}");
			continue;
		}
		out.push((id, Listed {
			title,
			recorded: format!("{} — last modified on drive, which records no other date", drive_date(&modified)),
			within: Some(format!("<{url}> — {folder_title}")),
		}));
	}
	assert_eq!(page.matches("flip-entry-title").count(), page.matches("class=\"flip-entry\" id=").count(), "{folder}: drive listed entries this did not read");
	out
}

/// Drive writes a date as `11/23/25`, drops the year for this one (`Mar 12`), and gives only a time
/// for today.
fn drive_date(shown: &str) -> String {
	let today = String::from_utf8(Command::new("date").arg("+%F").output().expect("date runs").stdout).expect("date prints ascii");
	let year = &today[..4];
	const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
	let num = |s: &str| s.parse::<u32>().unwrap_or_else(|e| panic!("drive wrote `{shown}` as a date: {e}"));
	match shown.split(['/', ' ']).collect::<Vec<_>>()[..] {
		[m, d, y] if y.len() == 2 => format!("20{y}-{:02}-{:02}", num(m), num(d)),
		[mon, d] if MONTHS.contains(&mon) => format!("{year}-{:02}-{:02}", MONTHS.iter().position(|x| *x == mon).expect("contained") + 1, num(d)),
		[_, ampm] if ampm == "AM" || ampm == "PM" => today.trim().to_string(),
		_ => panic!("drive wrote `{shown}` as a date, in a shape this does not read"),
	}
}

/// A recording nothing transcribed, named and dated by `listed`.
fn heard(platform: Platform, listed: &Listed, media: &Path) -> Recording {
	let (phrases, duration) = whisper_phrases(media);
	let silence = match phrases.is_empty() {
		true => " — nobody speaks",
		false => "",
	};
	Recording {
		title: listed.title.clone(),
		recorded: listed.recorded.clone(),
		duration,
		summary: None,
		chapters: None,
		phrases,
		turns: false,
		transcribed_by: format!("whisper-cpp `{}` — {} transcribes nothing{silence}", model_name(), platform.name()),
		within: listed.within.clone(),
	}
}

/// The recording as its host serves it, into `dir/recording.<ext>` — video at 720p where there is a
/// choice, and audio where the host holds nothing else.
fn fetch(url: &str, dir: &Path) -> PathBuf {
	std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("creating {}: {e}", dir.display()));
	let status = Command::new("yt-dlp")
		.args(["-q", "--no-warnings", "--no-progress", "-N", "8", "-S", "res:720", "-o"])
		.arg(dir.join("recording.%(ext)s"))
		.arg(url)
		.status()
		.expect("yt-dlp runs");
	assert!(status.success(), "yt-dlp could not fetch {url}: {status}");
	recording_in(dir).unwrap_or_else(|| panic!("yt-dlp fetched {url} and wrote no {}/recording.*", dir.display()))
}

fn recording_in(dir: &Path) -> Option<PathBuf> {
	let entries = match std::fs::read_dir(dir) {
		Ok(e) => e,
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
		Err(e) => panic!("reading {}: {e}", dir.display()),
	};
	entries
		.map(|e| e.expect("a directory entry is readable").path())
		.find(|p| p.file_stem().is_some_and(|s| s == "recording") && p.extension().is_some_and(|e| e != "part" && e != "ytdl"))
}

fn model_name() -> String {
	Path::new(WHISPER_MODEL).file_stem().expect("the model is a file").to_string_lossy().into_owned()
}

fn get(url: &str) -> String {
	ureq::get(url)
		.header("User-Agent", UA)
		.call()
		.unwrap_or_else(|e| panic!("GET {}: {e}", &url[..url.len().min(80)]))
		.body_mut()
		.with_config()
		// the word-level transcript of a two-hour call runs to several MB, well past ureq's default
		.limit(128 * 1024 * 1024)
		.read_to_string()
		.expect("the platforms answer in utf-8")
}

/// Cut out the value that starts at `from` and runs to the next `end`. Absence is a shape change on
/// the platform's side, and a transcript assembled past one would be silently wrong.
fn between(page: &str, from: &str, end: char, what: &str) -> String {
	let at = page.find(from).unwrap_or_else(|| panic!("no `{from}` on the page — {what} moved, re-read the page")) + from.len();
	let rest = &page[at..];
	let to = rest.find(end).unwrap_or_else(|| panic!("`{from}` never closes on the page — {what} moved, re-read the page"));
	rest[..to].to_string()
}

fn loom(id: &str, page: &str) -> Recording {
	let title = between(page, "<title>", '<', "the recording's title");
	// `durationMs` is the raw upload; this is what is left after trimming in loom's editor, and all anyone can play
	let duration: f64 = between(page, r#""duration": "PT"#, 'S', "the recording's published duration")
		.parse()
		.expect("loom states an ISO duration in seconds");
	Recording {
		title: title.strip_suffix(" | Loom").unwrap_or(&title).to_string(),
		recorded: between(page, r#""uploadDate": ""#, '"', "the recording's date"),
		duration,
		summary: loom_ai(page, id, "description"),
		chapters: loom_ai(page, id, "chapters").map(|c| {
			c.lines()
				.filter(|l| !l.trim().is_empty())
				.map(|l| {
					let (at, name) = l.trim().split_once(' ').unwrap_or_else(|| panic!("loom wrote `{l}` as a chapter"));
					(secs(at), name.to_string())
				})
				.collect()
		}),
		phrases: loom_phrases(page, id).unwrap_or_default(), // none is a recording loom never transcribed, which `render` whispers
		turns: false,
		transcribed_by: "loom".to_string(),
		within: None,
	}
}

/// What loom's AI wrote, read out of the recording's own node in the apollo state the page ships —
/// `description` there is the summary, and loom's marketing copy under the same name further down
/// the page is what the anchor exists to skip. Both keys are `null` for a recording loom never got
/// to, which is a state and not a failure.
fn loom_ai(page: &str, id: &str, key: &str) -> Option<String> {
	let node = page
		.find(&format!("\"RegularUserVideo:{id}\""))
		.unwrap_or_else(|| panic!("{id} has no node of its own on its own page — loom's payload moved, re-read the page"));
	let at = node + page[node..].find(&format!("\"{key}\":\""))? + key.len() + 4;
	let rest = &page[at..];
	let mut out = String::new();
	let mut chars = rest.chars();
	//LOOP: bounded by the length of a finite page
	while let Some(c) = chars.next() {
		match c {
			'"' => return Some(out).filter(|v: &String| !v.trim().is_empty()),
			// the value sits inside a JSON string literal, and loom writes its newlines escaped
			'\\' => match chars.next().expect("a JSON escape carries what it escapes") {
				'n' => out.push('\n'),
				'"' => out.push('"'),
				'\\' => out.push('\\'),
				'/' => out.push('/'),
				'u' => {
					let hex: String = chars.by_ref().take(4).collect();
					let code = u32::from_str_radix(&hex, 16).unwrap_or_else(|e| panic!("loom wrote `\\u{hex}` on the page: {e}"));
					out.push(char::from_u32(code).unwrap_or_else(|| panic!("loom wrote `\\u{hex}`, which is not a character")));
				}
				other => panic!("loom wrote an escape this does not read: `\\{other}`"),
			},
			c => out.push(c),
		}
	}
	panic!("`{key}` never closes on the loom page")
}

fn loom_phrases(page: &str, id: &str) -> Option<Vec<(f64, String)>> {
	let at = page.find(LOOM_TRANSCRIPT_CDN)?;
	assert!(page.contains(id), "{id} is not the page it was asked for");
	// the URL sits inside a JSON string literal, so it ends at the quote or at the escape before one
	let url = page[at..].split(['"', '\\']).next().expect("split yields at least once");
	let parsed: serde_json::Value = serde_json::from_str(&get(url)).expect("the transcript CDN answers json");
	let phrases = parsed["phrases"].as_array().expect("a loom transcript is an array of phrases");
	Some(
		phrases
			.iter()
			.map(|p| {
				let ts = p["ts"].as_f64().expect("a loom phrase is stamped");
				(ts, p["value"].as_str().expect("a loom phrase carries text").to_string())
			})
			.collect(),
	)
}

/// A fathom share page is an inertia app: everything it knows sits html-escaped in `data-page`. Its
/// summary and chapters are behind a login; the transcript, with speakers, is not.
fn fathom(page: &str) -> Recording {
	let data = between(page, r#"data-page=""#, '"', "fathom's page state")
		.replace("&quot;", "\"")
		.replace("&#39;", "'")
		.replace("&lt;", "<")
		.replace("&gt;", ">")
		.replace("&amp;", "&");
	let data: serde_json::Value = serde_json::from_str(&data).expect("fathom's page state is json");
	let props = &data["props"];
	let text = |v: &serde_json::Value, what: &str| v.as_str().unwrap_or_else(|| panic!("fathom's page carries no {what}")).to_string();
	let copy: serde_json::Value = serde_json::from_str(&get(&text(&props["copyTranscriptUrl"], "transcript url"))).expect("fathom's transcript endpoint answers json");
	let plain = text(&copy["plain_text"], "plain transcript");
	// `M:SS - Speaker` then the turn indented beneath it, turns a blank line apart, all after a `---`
	let body = plain.split_once("\n---\n").unwrap_or_else(|| panic!("fathom's transcript has no `---` under its title")).1;
	let phrases = body
		.split("\n\n")
		.filter(|turn| !turn.trim().is_empty())
		.map(|turn| {
			let (head, said) = turn.trim().split_once('\n').unwrap_or_else(|| panic!("fathom wrote `{turn}` as a turn"));
			let (at, speaker) = head.split_once(" - ").unwrap_or_else(|| panic!("fathom wrote `{head}` as a turn's head"));
			let said = said.lines().map(str::trim).collect::<Vec<_>>().join(" ");
			(secs(at) as f64, format!("**{}**: {said}", speaker.trim()))
		})
		.collect();
	Recording {
		title: text(&props["call"]["title"], "title"),
		recorded: text(&props["call"]["started_at"], "start time"),
		duration: props["duration"].as_f64().expect("fathom states the duration in seconds"),
		summary: None,
		chapters: None,
		phrases,
		turns: true,
		transcribed_by: "fathom".to_string(),
		within: None,
	}
}

/// The recording's audio through whisper, `(start secs, text)` per segment, and how long the audio runs.
fn whisper_phrases(media: &Path) -> (Vec<(f64, String)>, f64) {
	let model = PathBuf::from(std::env::var("HOME").expect("a user session has HOME")).join(WHISPER_MODEL);
	assert!(model.exists(), "{} is missing — whisper needs a model to transcribe {}", model.display(), media.display());
	let probe = |args: &[&str]| {
		let out = Command::new("ffprobe").args(["-v", "error"]).args(args).args(["-of", "csv=p=0"]).arg(media).output().expect("ffprobe runs");
		assert!(out.status.success(), "ffprobe could not read {}: {}", media.display(), String::from_utf8_lossy(&out.stderr));
		String::from_utf8(out.stdout).expect("ffprobe prints ascii").trim().to_string()
	};
	let duration: f64 = probe(&["-show_entries", "format=duration"]).parse().expect("ffprobe prints the duration in seconds");
	if probe(&["-select_streams", "a", "-show_entries", "stream=index"]).is_empty() {
		return (Vec::new(), duration); // a phone clip recorded with the microphone off
	}
	let dir = std::env::temp_dir().join(format!("call-pull-{}", std::process::id()));
	std::fs::create_dir_all(&dir).expect("the temp dir is writable");
	let wav = dir.join("audio.wav");
	let base = dir.join("audio");
	let run = |cmd: &mut Command| {
		let status = cmd.status().unwrap_or_else(|e| panic!("running {cmd:?}: {e}"));
		assert!(status.success(), "{cmd:?} failed: {status}");
	};
	run(Command::new("ffmpeg").args(["-v", "error", "-y", "-i"]).arg(media).args(["-vn", "-ar", "16000", "-ac", "1"]).arg(&wav));
	eprintln!("  transcribing {} with whisper — minutes per hour of audio", media.display());
	run(Command::new("whisper-cli").arg("-m").arg(&model).arg("-f").arg(&wav).args(["-np", "-sns", "-oj", "-of"]).arg(&base));
	let json = std::fs::read_to_string(base.with_extension("json")).expect("whisper-cli -oj writes <of>.json");
	let parsed: serde_json::Value = serde_json::from_str(&json).expect("whisper-cli writes json");
	let segments = parsed["transcription"].as_array().expect("whisper json carries `transcription`");
	let phrases = segments
		.iter()
		.map(|s| {
			let ms = s["offsets"]["from"].as_f64().expect("a whisper segment carries its offset");
			(ms / 1000., s["text"].as_str().expect("a whisper segment carries text").trim().to_string())
		})
		.filter(|(_, t): &(f64, String)| !hallucinated(t))
		.collect();
	std::fs::remove_dir_all(&dir).expect("the temp dir is ours");
	(phrases, duration)
}

/// What whisper writes over silence and music rather than over speech: `[BLANK_AUDIO]` and its kin,
/// `♪`, and a lone "you", repeated.
// ponytail: a list of the known ones; silero VAD ahead of whisper if new kinds keep turning up
fn hallucinated(segment: &str) -> bool {
	let words: Vec<String> = segment
		.split_whitespace()
		.map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
		.filter(|w| !w.is_empty())
		.collect();
	let t = segment.trim();
	(t.starts_with('[') && t.ends_with(']')) || words.is_empty() || words.iter().all(|w| w == "you")
}

fn hms(secs: f64) -> String {
	let s = secs as u64;
	format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// `MM:SS` for anything under an hour, `H:MM:SS` past it — how loom writes a chapter's start.
fn stamp(secs: u64) -> String {
	match secs / 3600 {
		0 => format!("{:02}:{:02}", secs / 60, secs % 60),
		h => format!("{h}:{:02}:{:02}", (secs % 3600) / 60, secs % 60),
	}
}

/// `M:SS`, `MM:SS`, `H:MM:SS` or `HH:MM:SS`.
fn secs(stamp: &str) -> u64 {
	stamp.split(':').fold(0, |acc, part| acc * 60 + part.parse::<u64>().unwrap_or_else(|e| panic!("`{stamp}` is not a time: {e}")))
}

/// A filename that says what the recording is. The id stays in the file, so this one is free to
/// read like the title it came from, and the date in front of it is what keeps two calls of the
/// same name apart.
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

/// Phrases joined into prose, `(start secs, text)` per paragraph. Speaker turns already are paragraphs.
fn paragraphs(phrases: &[(f64, String)], turns: bool) -> Vec<(f64, String)> {
	if turns {
		return phrases.to_vec();
	}
	let mut out: Vec<(f64, String)> = Vec::new();
	let mut open = false;
	for (ts, text) in phrases {
		match open {
			true => {
				let last = &mut out.last_mut().expect("open implies one").1;
				last.push(' ');
				last.push_str(text.trim());
			}
			false => out.push((*ts, text.trim().to_string())),
		}
		let last = &out.last().expect("pushed above").1;
		// ponytail: word count + sentence end, speaker turns where the platform exposes none
		open = !(last.split_whitespace().count() >= PARAGRAPH_WORDS && last.ends_with(['.', '?', '!']));
	}
	out
}

/// The capture, its filename, and whether it needs `/call-digest`.
fn render(platform: Platform, id: &str, mut r: Recording) -> (String, String, bool) {
	let share = platform.share();
	// loom chapters a long call's first hour or so and stops; a reading that stops short is no reading.
	// measured against the last words rather than the recording, since calls are left running after everyone's gone
	if r.chapters.as_ref().is_some_and(|c| {
		let spoken = r.phrases.last().expect("chapters come with a transcript").0;
		c.last().is_none_or(|(t, _)| (*t as f64) < spoken - TAIL_SECS)
	}) {
		eprintln!("  {}'s chapters stop short of the end — leaving its reading to `/call-digest`", platform.name());
		r.summary = None;
		r.chapters = None;
	}

	let missing: Vec<&str> = [("summary", r.summary.is_none()), ("chapters", r.chapters.is_none())]
		.iter()
		.filter(|(_, m)| *m)
		.map(|(w, _)| *w)
		.collect();
	let mut out = format!(
		"# {}\n\
		 \n\
		 - source: <{share}{id}>\n\
		 - recorded: {}\n\
		 - duration: {}\n\
		 - transcribed by: {}\n\
		 - read by: {}\n\
		 {}\
		 - pulled by: `scripts/call-pull.rs`\n\
		 \n",
		r.title,
		r.recorded,
		hms(r.duration),
		r.transcribed_by,
		match (r.phrases.is_empty(), missing.is_empty()) {
			(true, _) => "nobody — nothing is said".to_string(),
			(false, true) => platform.name().to_string(),
			(false, false) => format!("nothing yet — {} wrote no {}, and `/call-digest` writes them here", platform.name(), missing.join(" and no ")),
		},
		r.within.as_ref().map_or(String::new(), |w| format!("- within: {w}\n")),
	);
	if let Some(summary) = &r.summary {
		out.push_str(&format!("## summary\n\n{}\n\n", summary.trim()));
	}
	out.push_str("## transcript\n");
	let seek = platform.seek();
	let header = |t: u64, name: &str| format!("\n### [{}]({share}{id}{seek}{t}) {name}\n", stamp(t)).replace(" \n", "\n");
	let flush = |out: &mut String, body: &mut Vec<(f64, String)>| {
		for (_, text) in paragraphs(body, r.turns) {
			out.push_str(&format!("\n{text}\n"));
		}
		body.clear();
	};
	match &r.chapters {
		Some(chapters) => {
			let mut next = chapters.iter().peekable();
			let mut body = Vec::new();
			let mut started = false; // the first chapter opens the text wherever the platform put it, so nothing sits above it
			//LOOP: bounded by the phrases of a finite transcript
			for (ts, text) in &r.phrases {
				while let Some((t, name)) = next.next_if(|(t, _)| !started || (*t as f64) <= *ts) {
					flush(&mut out, &mut body);
					out.push_str(&header(*t, name));
					started = true;
				}
				body.push((*ts, text.clone()));
			}
			flush(&mut out, &mut body);
			assert!(next.peek().is_none(), "{id}: chaptered past the last phrase of its transcript");
		}
		// no chapters to hang the text on, so every paragraph gets its own untitled anchor — the
		// times `/call-digest` needs to place chapters, and deletes once it has
		None => {
			let mut last = None;
			for (ts, text) in paragraphs(&r.phrases, r.turns) {
				let t = ts as u64;
				// two turns inside one second would repeat an anchor; the second joins the first
				if last != Some(t) {
					out.push_str(&header(t, ""));
				}
				out.push_str(&format!("\n{text}\n"));
				last = Some(t);
			}
		}
	}
	// a folder's file names repeat where a platform's titles don't, and the id is what tells them apart
	let tail = match platform {
		Platform::Drive => format!("-{}", &id[..6]),
		Platform::Loom | Platform::Fathom | Platform::Vimeo | Platform::Vocaroo => String::new(),
	};
	(format!("{}-{}{tail}.md", &r.recorded[..10], slug(&r.title)), out, missing.is_empty() || r.phrases.is_empty())
}
