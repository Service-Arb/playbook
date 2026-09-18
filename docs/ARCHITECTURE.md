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

**`ref/`** — raw, verbatim, never hand-edited. One file per source, named by the
source's own id. Every file states where it came from and when it was recorded.
Re-running a puller over an existing file is a no-op; a stale capture is re-pulled
by deleting it.

```
ref/
├── README.md              # the source list — a link written here is a link the pullers will fetch
├── loom/<id>.md           # call recordings, one timestamped line per phrase
├── research/<id>.md       # chatgpt shares, one `## user` / `## assistant` section per turn
├── youtube/<id>.md        # channel videos, and `<id>/` the frames it cites
└── skool_gmbpp/course/    # the classroom, via `social_networks`
```

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
would feed on its own output.

```
scripts/
├── loom-pull.rs       # share page → signed transcript CDN → timestamped lines
├── chatgpt-pull.rs    # headless chromium → the DOM's turns
├── yt-pull.rs         # yt-dlp → captions in citable blocks, chapters, frames, description
└── skool-pull.rs      # recon classroom → the course, lesson by lesson
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
- `ref/` is written by pullers, never by hand.
- An agent's own reading goes to `suggested/`, never straight to `approved/`.
- A puller writes a new capture or does nothing; it never edits one.
