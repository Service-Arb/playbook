---
name: kb-answer
description: Answer a question out of this playbook — "do we have…", "what do they ask for…", a message someone sent that needs a reply. Use for any question whose answer lives in structured/ or ref/.
---

# kb-answer

Find it in `structured/` first (`approved/` over `suggested/`), then in `ref/` for what the notes
never distilled. Every note already links its source, so `structured/` is how you find it quickly,
and the source is what you cite.

## Sources

The answer usually gets pasted into a chat with someone who has no access to this repo. So:

- cite the source under the note, not the note. Give the loom chapter `?t=`, the youtube `&t=`, the
  skool lesson or post URL, the chatgpt share. If a note links a moment, use that moment
- write the bare URL, `https://www.loom.com/share/<id>?t=3302`, not `[2026-09-18](…)`. Link text is
  lost when the answer is copied out of a terminal
- put the source on the line under the claim it backs, the way the notes do it
- cite a `structured/` or `ref/` path only for what has nothing public under it, like the human's own
  notes, and say that it is internal
- a claim with no source gets no citation. Say where it came from instead ("Eric, unrecorded")

## Source reliability

Rank sources by `docs/ARCHITECTURE.md`'s `## Reliability`. When a claim has more than one source,
cite the best one first. The others go under it as secondary, if at all.
