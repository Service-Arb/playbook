<!--Reference: https://matklad.github.io/2021/02/06/ARCHITECTURE.md.html-->
# Architecture

## Overview

A knowledge base about service arbitrage — running Google Business Profiles into
subcontracted trade work. Not code. The repository's whole job is that every claim
in it can be traced to the moment someone said it, and dated.

Sources are captured raw, then distilled by hand or by agent into topic notes.
Capture and distillation never happen in the same file.

```
   loom · youtube · chatgpt · skool · discord
                     │
                     │  scripts/*.rs, social_networks
                     ▼
                   ref/                        raw · verbatim · dated
                     │
         ┌───────────┴────────────┐
         │ an agent reads it      │ the human's own notes, re-sourced
         ▼                        ▼
 structured/suggested/ ──────► structured/approved/
                  the human promotes │
                                     ▼
                                secondary/     plans, comparisons, decisions

        every bullet on the right links back to a second on the left
```

## Layers

**`ref/`** — raw, verbatim, never hand-edited. One file per source, stating its own
source id, where it came from and when it was recorded. A capture is identified by
that id and not by its path, so a file is free to be named and placed after what it
*says* — and a source that reorders or renames upstream is moved, not pulled twice.
Re-running a puller over an existing capture is a no-op; a stale one is re-pulled by
deleting it.

```
ref/
├── README.md                     # the source list — every source, and which of them a puller reaches
├── loom/<date>-<title>.md        # call recordings: summary, then the transcript under its chapters
├── fathom/<date>-<title>.md      # the same, for calls recorded on fathom
├── research/<id>.md              # chatgpt shares, one `## user` / `## assistant` section per turn
├── youtube/<id>.md               # channel videos, and `<id>/` the frames it cites
└── skool_gmbpp/                  # the classroom, via `social_networks`
    ├── README.md                 # what upstream holds, and the day it was last compared to this
    └── course/<NN-module>/       # a directory per module, a file per lesson, both in upstream's order
```

Two things a capture cannot state about itself are written beside it instead, since a
capture is never edited: **how far behind upstream it is**, in `skool_gmbpp/README.md`,
rewritten every run with the day of the run; and **what a source says that this tree
has no capture of**, printed by the puller for `ref/README.md` to be told about by hand.

### Call captures

```md
# <title>

- source: <https://www.loom.com/share/<id>>        # or https://fathom.video/share/<id>
- recorded: · duration: · transcribed by: · read by: · pulled by:

## summary

<the platform's, verbatim>

## transcript

### [02:39](https://www.loom.com/share/<id>?t=159) Location Spoofing Strategy

plain prose, in paragraphs, no stamps in it
```

- the transcript covers the whole recording. The platform's is taken unless it stops more than ten minutes short; then
  the puller transcribes the audio itself, and `transcribed by:` says which, and how far the platform got
  — and its summary and chapters go with its transcript, since they only read as far as it did
- the platform's summary and chapters are dropped too when its chapters stop more than ten minutes
  short of the end
- chapters are `###` headers inside `## transcript`, never a list of their own. The header is the
  only timestamp — a citation links the chapter a claim sits in
- nothing sits above the first header
- speaker turns, where the platform names speakers, are paragraphs opening `**<speaker>**:`
- a recording nobody chaptered gets an untitled header per paragraph instead — the times
  `/call-digest` needs to place chapters. It titles the ones that open a chapter and deletes the rest

`scripts/call-pull.rs` checks every capture on disk against this on each run, and `--check` runs only
that; a hook runs it after any agent edit under `ref/loom/` or `ref/fathom/`.

The one reading an agent may put in `ref/` is what the source's own platform would have:
a recording its platform never summarised gets a summary from `/call-digest`, which says so
in the capture's `read by:` line. Nothing else.

**`structured/suggested/`** — an agent's own reading of a source. Unreviewed.
**`structured/approved/`** — what the human has already vetted.

Promotion is the human moving a bullet from one to the other. An agent may place into
`approved/` only material the human already wrote — their own call notes, transcribed
and re-sourced — never its own inference.

**`secondary/`** — conclusions that follow from the notes rather than from a source:
plans, comparisons, decisions.

## Topics

One file per topic, same names on both sides of `structured/`:

`verif_and_suspension` · `reviews` · `seo_and_profile` · `site_conversion` ·
`profile_management` · `money_models` · `tooling`

The taxonomy follows the data. A topic that keeps collecting unrelated bullets splits.

## Note format

A bullet is one line. Elaboration goes underneath it: `reason:` for why, `Q:` / `A:`
for open questions, `>` for asides, `//` for parentheticals.

Every claim carries its source directly beneath it — the link text is the date, so
age reads at a glance:

```md
- not wise to compete with people with 200+ reviews
  [2026-09-18](https://www.loom.com/share/<id>?t=4210)
```

## Recency

Contradictions are resolved newest-wins: the new claim replaces the old one in place,
and the old one stays as a dated line beneath it. Nothing is deleted for being wrong —
only demoted, with its date still on it.

## Pullers

`scripts/*.rs` are cargo scripts (`#!/usr/bin/env -S cargo -Zscript`), run from
anywhere inside the checkout. Each one takes source URLs as arguments, or with no
arguments reads `ref/README.md` for links of its platform and fetches the ones missing
from disk. So adding a source is pasting its link into `ref/README.md` and re-running.

`ref/README.md` is the only registry. A capture quotes its own source URL and a
transcript quotes every link that was said out loud, so a puller that scanned the tree
would feed on its own output. A puller that *finds* a link — one lesson pointing at
another platform's recording — prints it rather than following it, and the registry
grows by hand.

```
scripts/
├── call-pull.rs       # loom / fathom share page → summary, chapters, and the platform's transcript, or whisper's where it stops short
├── chatgpt-pull.rs    # headless chromium → the DOM's turns
├── yt-pull.rs         # yt-dlp → captions in citable blocks, chapters, frames, description
└── skool-pull.rs      # recon classroom → the course tree, module by module and lesson by lesson
```

Anything needing a session lives in
[`social_networks`](https://github.com/valeratrades/social_networks), and a puller shells
out to it rather than linking it — that crate's tree only resolves against its own lock.
It knows how to read a platform; the puller here decides how a capture is filed.

Skool *conversations* are already `recon posts skool:<slug>`, which writes them into the
rolodex's `venues/` tree; nothing here duplicates that.

## Invariants

- Nothing enters `structured/` that did not enter `ref/` first.
- Every claim carries a link to its source and the date it was said.
- `ref/` is written by pullers, never by hand — bar what `/call-digest` fills in, which says so.
- An agent's own reading goes to `suggested/`, never straight to `approved/`.
- A puller writes a new capture, moves one, or does nothing; it never edits what one says.
