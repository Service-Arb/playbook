---
name: loom-digest
description: Fill in the summary, chapters or transcript loom never wrote for a capture in ref/loom/. Use when loom-pull.rs says a recording was left unread, when a ref/loom/ file's `read by:` line says `nothing yet`, or when asked to summarise / chapter / transcribe a loom recording in this repo.
---

# loom-digest

`scripts/loom-pull.rs` writes what loom's own AI made of a recording. Loom does not always make it.
A capture it left short says so:

```md
- read by: nothing yet — loom wrote no summary and no chapters, and `/loom-digest` writes them here
```

Your job is to write the missing sections, in the shape loom writes them, so a reader cannot tell
which recordings loom read and which you did — except from that one line, which you rewrite.

## Pick the target

With a path argument, that file. Without one:

```bash
grep -l 'read by: nothing yet' ref/loom/*.md
```

Do one file per run. These are long; a run that batches them writes worse summaries.

## What to write

Three sections, in this order, between the header and `## transcript`. Write only the ones the
`read by:` line says are missing.

### `## summary`

One paragraph, first person, as the person recording speaks — loom's own summaries are written from
inside the recording (`In this video, I walk you through…`), and matching that is what keeps the
corpus uniform. Name what was actually done and what it was for. No preamble, no "this video
covers".

### `## chapters`

One line per topic shift, `[MM:SS](<source>?t=<seconds>) Title Case Name`. The stamp is the second
the shift happens, and the link is the capture's own `source:` URL with `?t=` on it. Loom writes
about one chapter every two to four minutes and always opens at `00:00`; a two-hour call gets tens
of them, not five.

```md
- [00:00](https://www.loom.com/share/<id>?t=0) Introduction to Money Making Model
- [02:02](https://www.loom.com/share/<id>?t=122) Address Pages vs Service Areas
```

The seconds in the link and the stamp must agree. Check a few by hand.

### `## transcript`

Only when the section is empty — loom transcribed nothing. Get the audio and transcribe it:

```bash
yt-dlp -x --audio-format wav -o /tmp/<id>.%(ext)s 'https://www.loom.com/share/<id>'
whisper-cli -f /tmp/<id>.wav -oj -of /tmp/<id>
```

Then write one block per phrase, blank line between them, the whole link on every line so a line
stays citable once it is copied out:

```md
[00:00:00](https://www.loom.com/share/<id>?t=0) Okay, so now that we have gotten through that…
```

Stamps are `HH:MM:SS`; `?t=` is that in seconds. Never paraphrase here — a transcript is `ref/`, and
`ref/` is verbatim.

## Then rewrite the header line

```md
- read by: `/loom-digest`, <today's date>
```

State what you wrote if it was not all of it: `- read by: `/loom-digest`, 2026-09-18 — chapters only`.

## What not to do

- do not touch the transcript when one is already there, and do not touch `# title`, `source:`,
  `recorded:`, `duration:` or `pulled by:` in any case
- do not write a claim the recording does not make. This is `ref/` — a reading of a source that the
  source does not support poisons everything downstream of it
- do not promote anything into `structured/`. That is a separate step, and a human's
- a re-pull (deleting the file and running `loom-pull.rs`) throws your work away, which is the right
  trade: loom having caught up is worth more than what you wrote
