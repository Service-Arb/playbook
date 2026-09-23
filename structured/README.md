# Blocks

`scripts/cite-check.rs` holds every block here to this file. It runs in `nix run .#check`, and a hook
runs it after any agent edit under `structured/`.

## Sections
A block has up to three sections, in this order, and no other headers:

- `## info`: what is so, as told
- `## facts`: what someone ran, and the result they report
- `## plays`: an inefficiency and how to act on it

A claim scoped to one trade lives under `market/niches/<niche>/`, at the same path it would have
outside it. A general claim with a trade caveat keeps its `*niche*:` tag where it is.

## Citations
A citation is a line that holds only a link, `[r<base> <date>](url)`:

```md
- one review a day per profile, at most
  [r2 2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=3026)
  [r4 2026-08-07](https://www.loom.com/share/143a77b218e54f3eb570ba1e47f8285a?t=1200)
```

- `<base>` is the base reliability from `docs/ARCHITECTURE.md`'s `## Reliability`, 0 to 7. Only the
  base is written, because the effective number changes every day
- `<date>` is the capture's own: `recorded:`, `uploaded:`, a lesson's `updated:`, a chatgpt share's
  `pulled:`
- the url resolves to a capture in `ref/`, and a `?t=` links the second the claim was said
- skool course material is always `r1`
- `r0` is a test we ran ourselves. It links the comment that closed the test's issue,
  `https://github.com/Service-Arb/playbook/issues/<n>#issuecomment-<id>`
- consecutive citation lines are a run. A run backs the text above it, back to the previous citation
- every bullet is backed by a run, or by an `// unsourced: <who>` line, before its section ends

## Effective reliability
effective = min(10, base + age in days / 180)

The number is fractional, so two citations keep their order as both of them age. A run leads with its
best effective reliability, and a tie goes to the newer one. When claims contradict, the better
effective number leads. The other stays beneath it, demoted, with its date still on it.

## Style
Observed from how the notes get written by hand. Follow it when persisting anything.

- terse lowercase bullets, one claim each. no filler, no hedging, no "it's worth noting"
- `Q:` an open question. `A:` its answer, recorded even when we don't buy it
  > skepticism about an answer goes in the aside, the answer still gets written down
- `>` aside / caveat / the thing that qualifies the line above
- `//` parenthetical, thinking out loud, the second-order observation
- `reason:` why, under the claim it explains
- `TODO:` something to go test
- `(!)` marks the surprising bit — the part that actually changes how we'd act
- attribute by name, in full: `Ilya says…`, `Eric recommends…`, `Sam Hart says…`
  > who said it is half the information. an unattributed claim is weaker than a named one
- backticks for handles, services and tools: `accsmarket`, `Jordan Rodriguez`
- `*niche*:` italics to open a bullet that's scoped to one trade
- `[^1]` footnote for the side-reference that would break the line
