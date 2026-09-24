use std::{
	path::{Path, PathBuf},
	process::Command,
};

use clap::{Parser, Subcommand, ValueEnum};
use v_utils::io::{ConfirmResult, confirmation};

const PLATFORMS: [&str; 5] = ["ref/loom", "ref/fathom", "ref/drive", "ref/vimeo", "ref/vocaroo"];
/// Past this, `--execute` asks before spending.
const CONFIRM_USD: f64 = 1.;
/// ¢ per minute of recording, by the `ask_llm` `Footage` `scripts/call-watch.rs` reads it as.
const SCREEN_CENTS_PER_MIN: f64 = 0.1;
const PHONE_CENTS_PER_MIN: f64 = 0.6;

#[derive(Parser)]
struct Cli {
	#[command(subcommand)]
	command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
	/// Redo a machine transcription of every call capture under the paths. Prints each with what it
	/// would cost; only `--execute` runs it.
	ReTranscribe {
		what: What,
		#[arg(required = true)]
		paths: Vec<PathBuf>,
		#[arg(long)]
		execute: bool,
	},
}

#[derive(Clone, Copy, ValueEnum)]
enum What {
	/// whisper's transcript, re-pulled from the kept recording
	Audio,
	/// `shown.md`, re-watched through `scripts/call-watch.rs`
	Picture,
}

/// The fields of a capture this reads.
struct Capture {
	path: PathBuf,
	source: String,
	minutes: f64,
	transcribed_by: String,
	read_by: String,
}

fn main() {
	let Cmd::ReTranscribe { what, paths, execute } = Cli::parse().command;
	let root = repo_root();
	let captures: Vec<Capture> = paths.iter().flat_map(|p| captures_under(&root, p)).map(|p| read(&p)).collect();

	let mut todo = Vec::new();
	let mut usd = 0.;
	for c in &captures {
		let rel = c.path.strip_prefix(&root).expect("captures are found under the root").display();
		match plan(what, c) {
			Ok((cost, why)) => {
				println!("{:>8} {rel} — {why}", format!("${cost:.3}"));
				usd += cost;
				todo.push(c);
			}
			Err(skip) => println!("{:>8} {rel} — {skip}", "skip"),
		}
	}
	println!("{:>8} {} of {} captures", format!("${usd:.2}"), todo.len(), captures.len());
	if !execute || todo.is_empty() {
		return;
	}
	if usd > CONFIRM_USD && !matches!(confirmation(&format!("spend ~${usd:.2}?")).flush_blocking(), ConfirmResult::Yes) {
		return;
	}
	match what {
		What::Picture => {
			for c in &todo {
				let kept = c.path.with_extension("");
				if kept.join("shown.md").exists() {
					std::fs::remove_file(kept.join("shown.md")).expect("the capture's dir is ours");
				}
				if kept.join("frames").exists() {
					std::fs::remove_dir_all(kept.join("frames")).expect("the capture's dir is ours");
				}
			}
			run(Command::new(root.join("scripts/call-watch.rs")).args(todo.iter().map(|c| &c.path)));
		}
		What::Audio => {
			// the kept recording waits where call-pull.rs stages a fetch, which it then finds already there
			for c in &todo {
				let id = c.source.trim_end_matches('/').rsplit('/').next().expect("rsplit yields at least once");
				let staged = root.join("tmp/call-pull").join(id);
				assert!(!staged.exists(), "{} is already staged — a pull was cut short; move it back beside its capture", staged.display());
				std::fs::create_dir_all(staged.parent().expect("joined above")).expect("tmp/ is writable");
				std::fs::rename(c.path.with_extension(""), &staged).expect("tmp/ is on the same filesystem as ref/");
				std::fs::remove_file(&c.path).expect("the capture was just read");
			}
			// a drive, vimeo or vocaroo recording is named by the registry, so only a registry run can pull it
			run(Command::new(root.join("scripts/call-pull.rs")).current_dir(&root));
		}
	}
}

/// `Ok((usd, what it is))` for a capture this would redo, `Err(why not)` for one it leaves.
fn plan(what: What, c: &Capture) -> Result<(f64, String), String> {
	match what {
		What::Audio => {
			if !c.transcribed_by.starts_with("whisper") {
				return Err(format!("transcribed by {}, which a re-pull fetches again rather than transcribes", c.transcribed_by));
			}
			if c.read_by.starts_with("`/") {
				return Err("digested, and a re-pull drops the digest".into());
			}
			Ok((0., format!("{:.0} min through local whisper", c.minutes)))
		}
		What::Picture => {
			let kept = c.path.with_extension("");
			let media = std::fs::read_dir(&kept)
				.unwrap_or_else(|e| panic!("{}: {e} — run call-pull.rs to fetch the recording", kept.display()))
				.map(|e| e.expect("a directory entry is readable").path())
				.find(|p| p.file_stem().is_some_and(|s| s == "recording"))
				.unwrap_or_else(|| panic!("{} holds no recording — run call-pull.rs to fetch it", kept.display()));
			let video = Command::new("ffprobe")
				.args(["-v", "error", "-select_streams", "v", "-show_entries", "stream=index", "-of", "csv=p=0"])
				.arg(&media)
				.output()
				.expect("ffprobe runs");
			assert!(video.status.success(), "ffprobe could not read {}", media.display());
			if video.stdout.iter().all(u8::is_ascii_whitespace) {
				return Err("audio only, nothing shown".into());
			}
			let (kind, rate) = match c.source.contains("drive.google.com") {
				true => ("phone clip", PHONE_CENTS_PER_MIN),
				false => ("screen", SCREEN_CENTS_PER_MIN),
			};
			Ok((c.minutes * rate / 100., format!("{:.1} min of {kind} at {rate}¢/min", c.minutes)))
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

/// Every call capture at or under `path`: the `.md` files sitting directly in a platform dir.
fn captures_under(root: &Path, path: &Path) -> Vec<PathBuf> {
	let path = std::fs::canonicalize(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
	let is_capture = |p: &Path| p.extension().is_some_and(|e| e == "md") && PLATFORMS.iter().any(|d| p.parent() == Some(&root.join(d)));
	let mut out = Vec::new();
	let mut stack = vec![path.clone()];
	//LOOP: bounded by the entries of a finite tree
	while let Some(p) = stack.pop() {
		match p.is_dir() {
			true => stack.extend(std::fs::read_dir(&p).unwrap_or_else(|e| panic!("reading {}: {e}", p.display())).map(|e| e.expect("a directory entry is readable").path())),
			false if is_capture(&p) => out.push(p),
			false => (),
		}
	}
	assert!(!out.is_empty(), "{} holds no call capture", path.display());
	out.sort();
	out
}

fn read(path: &Path) -> Capture {
	let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
	let field = |key: &str| {
		text.lines()
			.find_map(|l| l.strip_prefix(&format!("- {key}: ")))
			.unwrap_or_else(|| panic!("{} states no `{key}:` — run call-pull.rs --check", path.display()))
			.to_string()
	};
	let secs = field("duration").split(':').fold(0u64, |acc, p| acc * 60 + p.parse::<u64>().unwrap_or_else(|e| panic!("{}: duration: {e}", path.display())));
	Capture {
		path: path.to_path_buf(),
		source: field("source").trim_matches(['<', '>']).to_string(),
		minutes: secs as f64 / 60.,
		transcribed_by: field("transcribed by"),
		read_by: field("read by"),
	}
}

fn run(cmd: &mut Command) {
	let status = cmd.status().unwrap_or_else(|e| panic!("running {cmd:?}: {e}"));
	assert!(status.success(), "{cmd:?} failed: {status}");
}
