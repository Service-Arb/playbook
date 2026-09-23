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
├── drive/<date>-<title>-<id>.md  # the same, for recordings in a shared drive folder
├── docs/<title>.md               # google docs, exported as markdown
├── research/<id>.md              # chatgpt shares, one `## user` / `## assistant` section per turn
├── youtube/<id>.md               # channel videos, and `<id>/` the frames it cites
└── skool_<group>/                # a classroom, via `social_networks` — `gmbpp`, and the `cheap` group
    ├── README.md                 # what upstream holds, and the day it was last compared to this
    └── course/<NN-module>/       # a directory per module, a file per lesson, both in upstream's order
```

Two things a capture cannot state about itself are written beside it instead, since a
capture is never edited: **how far behind upstream it is**, in `skool_<group>/README.md`,
rewritten every run with the day of the run; and **what a source says that this tree
has no capture of**, printed by the puller for `ref/README.md` to be told about by hand.

### Call captures

```md
# <title>

- source: <https://www.loom.com/share/<id>>        # or fathom.video/share/<id>, drive.google.com/file/d/<id>
- recorded: · duration: · transcribed by: · read by: · pulled by:

## summary

<the platform's, verbatim>

## transcript

### [02:39](https://www.loom.com/share/<id>?t=159) Location Spoofing Strategy

plain prose, in paragraphs, no stamps in it
```

- the transcript is the platform's; where it has none, the puller transcribes the audio itself, and
  `transcribed by:` says so
- the platform's summary and chapters are dropped when its chapters stop more than ten minutes short
  of the last words said
- chapters are `###` headers inside `## transcript`, never a list of their own. The header is the
  only timestamp in the capture. A citation links the second a claim was said
- nothing sits above the first header
- paragraphs and headers are a blank line apart
- a digest's prose is the prose of the committed pull it ran on, so a pull is committed before it is
  digested
- a digested summary has a `### <topic> <stamp>` per chapter, same names and times; a platform's own
  summary is verbatim and held to nothing
- speaker turns, where the platform names speakers, are paragraphs opening `**<speaker>**:`
- a recording nobody chaptered gets an untitled header per paragraph instead — the times
  `/call-digest` needs to place chapters. It titles the ones that open a chapter and deletes the rest

`scripts/call-pull.rs` checks every capture on disk against this on each run, and `--check` runs only
that; a hook runs it after any agent edit under `ref/loom/`, `ref/fathom/` or `ref/drive/`.

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

## Blocks

`structured/` is a tree of topical blocks, the same shape on both sides. As with rust modules, `x.md`
is the block and `x/` holds its children, and either one can exist alone:

```
structured/{approved,suggested}/
├── market.md       market/niches/<niche>/…     niche and city choice; per-trade economics
├── profile/        creation · verification/{video,live} · suspension/registration
├── ranking/        naming · categories · reviews/{stand,rate_limits,deletion,sourcing,local_guide}
├── conversion.md   site, intake, VA call handling
├── fulfilment.md   contractors, splits, pricing
└── infra.md        gmails, devices, proxies, phones, payments
```

A block splits into `## info` (what is so), `## facts` (what someone ran, and its result) and
`## plays` (an inefficiency and how to act on it). The taxonomy follows the data. A block that keeps
collecting unrelated bullets splits into children.

## Note format

A bullet is one line. Elaboration goes underneath it. Every claim carries its source directly
beneath it, and the link text is the base reliability and the date:

```md
- one review a day per profile, at most
  [r2 2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=3026)
```

The rest of the format and the style are in `structured/README.md`. `scripts/cite-check.rs` enforces
them, in `nix run .#check`, and a hook runs it after any agent edit under `structured/`.

## Reliability

Every citation carries a reliability: 0 to 10, lower is better. Who said it and how they came to know
it sets the base:

| base | source                                                        |
|------|---------------------------------------------------------------|
| 0    | tested myself                                                 |
| 1    | skool course material                                         |
| 2    | Eric said it                                                  |
| 3    | a gmbpp member tested it                                      |
| 4    | a gmbpp member claimed it                                     |
| 5    | a gmbpp member heard or thought of it; a cheap group member tested it |
| 6    | a cheap group member claimed it                               |
| 7    | a cheap group member heard or thought of it                   |

Then everything decays with age, up to 10 — a cheap group member having heard of it long ago, which
is as good as nothing. How the decay is counted is defined below this level, not here.

A claim backed by several sources cites its best one first; the rest sit under it as secondary.
Contradictions resolve the same way: the better effective reliability leads, and the other stays
beneath it, demoted, with its date still on it. Nothing is deleted for being wrong. Between two
sources of equal base, this is newest-wins.

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
├── call-pull.rs       # loom / fathom share page, drive folder → summary, chapters, and the platform's transcript, or whisper's where there is none
├── doc-pull.rs        # google doc → its markdown export
├── chatgpt-pull.rs    # headless chromium → the DOM's turns
├── yt-pull.rs         # yt-dlp → captions in citable blocks, chapters, frames, description
├── skool-pull.rs      # recon classroom → the course tree, module by module and lesson by lesson
└── cite-check.rs      # not a puller: holds every citation in structured/ to its capture in ref/
```

Anything needing a session lives in
[`social_networks`](https://github.com/valeratrades/social_networks), and a puller shells
out to it rather than linking it — that crate's tree only resolves against its own lock.
It knows how to read a platform; the puller here decides how a capture is filed.

Skool *conversations* are already `recon posts skool:<slug>`, which writes them into the
rolodex's `venues/` tree; nothing here duplicates that.

## Invariants

- Nothing enters `structured/` that did not enter `ref/` first — bar `r0`, which enters as the
  comment that closed its test's issue.
- Every claim carries a link to its source and the date it was said.
- `ref/` is written by pullers, never by hand — bar what `/call-digest` fills in, which says so.
- An agent's own reading goes to `suggested/`, never straight to `approved/`.
- A puller writes a new capture, moves one, or does nothing; it never edits what one says.
