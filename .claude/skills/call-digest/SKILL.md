---
name: call-digest
description: Fill in the summary or chapters the platform never wrote for a call capture in ref/loom/ or ref/fathom/. Use when call-pull.rs says a recording was left unread, when a capture's `read by:` line says `nothing yet`, or when asked to summarise / chapter a recording in this repo.
---

# call-digest

`scripts/call-pull.rs` writes what the platform's own AI made of a recording, and always a full
transcript. The platform does not always write a summary or chapters. A capture missing them says so:

```md
- read by: nothing yet — loom wrote no summary and no chapters, and `/call-digest` writes them here
```

Write what is missing, in loom's shape, so the only thing that tells a reader which recordings you
read is that one line. The capture's shape is "Call captures" in `docs/ARCHITECTURE.md`.

## Pick the target

With a path argument, that file. Without one:

```bash
grep -l 'read by: nothing yet' ref/loom/*.md ref/fathom/*.md
```

Do one file per run. These are long; a run that batches them writes worse summaries.

## `## summary`

Goes between the header and `## transcript`. A paragraph, then a `### <name> <stamp>` section per
topic with bullets under it — `ref/loom/2026-05-30-gmb-passive-profits-group-call.md` is the shape. The stamp
drops the leading zero: `0:00`, `3:34`, `1:02:27`.

A one-recorder video gets a first-person paragraph (`In this video, I walk you through…`); a call
gets it in the third person, named (`Eric explains…`, `Aidan reports…`). Say what was decided and
what is meant to happen next. No preamble, no "this video covers".

## Chapters

An unchaptered capture has an untitled header over every paragraph:

```md
### [03:34](https://www.loom.com/share/<id>?t=214)
```

Choose the ones where a topic starts and give them a title:

```md
### [03:34](https://www.loom.com/share/<id>?t=214) Spoofing location on an android
``` Delete every other untitled header. The
paragraphs underneath stay exactly as they are. Loom writes a chapter every two to four minutes, so
a two-hour call gets tens of them, not five. The first header stays, so nothing sits above it.

The summary's `###` topics and the chapters are the same list, with the same names and times.

## Then rewrite the header line

```md
- read by: `/call-digest`, <today's date>
```

If you did not write all of it, say what you wrote: ``- read by: `/call-digest`, 2026-09-18 — chapters only``.

Run `scripts/call-pull.rs --check`. A hook runs it after every edit here as well.

## What not to do

- do not change a word of the transcript. Moving headers is all you do there. `ref/` is verbatim
- do not touch `# title`, `source:`, `recorded:`, `duration:`, `transcribed by:` or `pulled by:`
- do not write a claim the recording does not make. A reading of a source that the source does not
  support poisons everything downstream of it
- do not promote anything into `structured/`. That is a separate step, and a human's
- a re-pull (deleting the file and running `call-pull.rs`) throws your work away. That is the right
  trade: the platform having caught up is worth more than what you wrote
