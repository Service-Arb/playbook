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

> very important to note that knowledge-base must be organized based on where and how it's likely to be used. [structured/approved/] drives the final agent's knowledge, so while [ref/] is organized based on where and when data is from, the actual knowledge we poses must follow the access patterns. Eg "expectations" from doing something are clearly subservient to the method/action itself, and data on how verify profiles, and on how to get reviews, are completely separate buckets.
> As such, the 

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
├── vimeo/ · vocaroo/             # the same, for hosts the registry line names and dates
├── <platform>/<capture>/         # beside every call capture: the recording, and what it shows
│   ├── recording.<ext>           # as the host serves it, 720p where there is a choice — LFS
│   ├── shown.md                  # what is on screen that the speech does not say, by `call-watch.rs`
│   └── frames/<secs>.jpg         # the frame each line of `shown.md` was read off — LFS
├── docs/<title>.md               # google docs, exported as markdown
├── research/<id>.md              # chatgpt shares, one `## user` / `## assistant` section per turn
├── youtube/                      # channel videos
│   ├── README.md                 # every video each channel lists, dated, ticked once captured — `yt-pull.rs sync`
│   └── <who>/<id>.md             # a video: summary, chapters, what is shown, captions — and `<id>/<secs>.jpg` the frames its `## shown` lines were read off
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
- a recording nobody speaks in is captured all the same: `transcribed by:` ends `— nobody speaks`,
  and only then is `## transcript` empty
- every capture has its recording beside it; one found without it gets it fetched, and the capture
  is left as it is

`scripts/call-pull.rs` checks every capture on disk against this on each run, and `--check` runs only
that; a hook runs it after any agent edit under a call capture's platform dir.

Machine transcriptions of a recording belong in `ref/` too, each naming what produced it: whisper's
of its audio, in `transcribed by:`, and `scripts/call-watch.rs`'s of its picture, in `shown.md`,
whose header names the model, the day and what it cost. A youtube capture reads its picture the same
way, into its own `## shown`, and states the models and the cost in its header. Every line of `shown.md` links the second it
describes and embeds the frame at that second, so the claim can be checked by eye.

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
| 1    | skool course material or in [structured/approved/]            |
| 2    | Eric said it                                                  |
| 3    | a gmbpp member tested it                                      |
| 4    | a gmbpp member claimed it or in [structured/suggested]        |
| 5    | a gmbpp member heard or thought of it; a cheap group member tested it |
| 6    | a cheap group member claimed it                               |
| 7    | a cheap group member heard or thought of it                   |

Then everything decays with age, up to 10 — a cheap group member having heard of it long ago, which
is as good as nothing. How the decay is counted lives in `scripts/cite-check.rs`, not here.

A claim backed by several sources cites its best one first; the rest sit under it as secondary.
Contradictions resolve the same way: the better effective reliability leads, and the other stays
beneath it, demoted, with its date still on it. Nothing is deleted for being wrong. Between two
sources of equal base, this is newest-wins.

## Pullers

`scripts/*.rs` are cargo scripts (`#!/usr/bin/env -S cargo -Zscript`), run from
anywhere inside the checkout. Each one takes source URLs as arguments, or with no
arguments reads `ref/README.md` for links of its platform and fetches the ones missing
from disk. So adding a source is pasting its link into `ref/README.md` and re-running.
`yt-pull.rs` alone goes through an index: `sync` lists every video of the channels
`ref/README.md` links into `ref/youtube/README.md`, and `transcribe` captures what that
list has unticked.

`ref/README.md` is the only registry. A capture quotes its own source URL and a
transcript quotes every link that was said out loud, so a puller that scanned the tree
would feed on its own output. A puller that *finds* a link — one lesson pointing at
another platform's recording — prints it rather than following it, and the registry
grows by hand. The one exception is the group's calls feed: what it posts is the list's own
material, so `skool-pull.rs` writes each recording into the registry itself.

```
scripts/
├── call-pull.rs       # loom / fathom share page, drive folder → summary, chapters, and the platform's transcript, or whisper's where there is none; the recording itself
├── call-watch.rs      # not a puller: a kept recording, through `ask_llm`'s `Client::watch`, frames only where the speech says something is shown (`--legacy`: wherever the picture changes) → `shown.md`
├── doc-pull.rs        # google doc → its markdown export
├── chatgpt-pull.rs    # headless chromium → the DOM's turns
├── yt-pull.rs         # sync: channels → the dated list of their videos; transcribe: captions in citable blocks, summary and chapters through `ask_llm`, the video through `Client::watch`, frames only where the captions say something is shown (`--legacy`: wherever the picture changes) → `## shown`, description
├── skool-pull.rs      # skool's classroom → the course tree, module by module and lesson by lesson; the calls feed → the registry's group calls
└── cite-check.rs      # not a puller: holds every citation in structured/ to its capture in ref/
```

What spends money goes through the `playbook` crate at the root of the cargo workspace, which
drives the scripts rather than duplicating them. `cargo r -- re-transcribe <audio|picture> <path>...`
lists every call capture under the paths with the estimated cost of redoing it; `picture` is
estimated at the `--legacy` rate either way, as the default's cost is known only after the pick. It runs only with
`--execute`, and asks first when the total is past $1. Audio is redone only for a whisper transcript
that has no digest yet, since a re-pull drops the digest.

A platform that needs a session, or one
[`social_networks`](https://github.com/valeratrades/social_networks) already knows, is read
by linking `social_networks_adapters`. It knows how to read a platform; the puller here
decides how a capture is filed.

Skool *conversations* are already `recon posts skool:<slug>`, which writes them into the
rolodex's `venues/` tree; nothing here duplicates that.

## Answering

```
skill/service-arb/         SKILL.md routes a question to the sources; sections/*.md say how to decide, per area
  ├─ symlinked to .claude/skills/service-arb — the owner's agent reads the tree directly
  └─ baked into playbook_web/ with structured/ and ref/**/*.md — members' agents get slices over MCP
playbook_web/              the member surface over MCP: search · read · guide, behind the Service-Arb panel (`sa:playbook:mcp:use`), every call logged
```

Members use the knowledge on their own tokens and never get the repo. What the server does, and how,
is in `playbook_web/README.md`.

## Invariants

- Nothing enters `structured/` that did not enter `ref/` first — bar `r0`, which enters as the
  comment that closed its test's issue.
- Every claim carries a link to its source and the date it was said.
- `ref/` is written by pullers, never by hand — bar what `/call-digest` fills in, which says so.
- An agent's own reading goes to `suggested/`, never straight to `approved/`.
- A puller writes a new capture, moves one, or does nothing; it never edits what one says.
- The member image carries `skill/`, `structured/` and `ref/**/*.md` only: no scripts, no docs, no
  history, no media.
