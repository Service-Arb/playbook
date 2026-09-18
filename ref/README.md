## Sources
Every source the playbook knows about. A link under a platform a puller reads is one `scripts/*.rs`
will fetch, so adding a source is pasting it below and re-running the puller — anything already on
disk is left alone. `skool-pull.rs` reads this file back to say which links the classroom names and
this list does not.

- [YT](https://www.youtube.com/@ericvelch)
- [course](https://www.skool.com/gmp-passive-profits-5347/classroom)
- [convos](https://www.skool.com/gmp-passive-profits-5347)
- [calls](https://www.skool.com/gmp-passive-profits-5347?c=bea7ab0d976f43dd953abd67d8987d0e&s=newest-cm&fl=)
- discord groups: Service Arb, DropHub, Lockedin GMB
- personal DMs: (all people marked as `ServiceArb` in rolodex)
- course recordings, as `skool-pull.rs` finds them in the classroom — the lesson's own video, and
  whatever the lesson wrote underneath it:
  - [2026-09-18](https://www.loom.com/share/1f32c7640a514b198adc997ca319238b)
  - [2026-09-18](https://www.loom.com/share/b497e683ff08477fba2f382082b9aca0)
  - [2026-09-18](https://www.loom.com/share/fc804298836c4ba282d8002dee239c58)
  - [2026-09-18](https://www.loom.com/share/9a2d3dc6b2c34d07a5fe4c667a01c0a3)
  - [2026-09-18](https://www.loom.com/share/622cc9f153ef4cf2b023ec6e8a21aa8d)
  - [2026-09-18](https://www.loom.com/share/044f7bdb93d14d329947ed50f9391dd4)
  - [2026-09-18](https://www.loom.com/share/088937bc44854962b7145d022e98184a)
  - [2026-09-18](https://www.loom.com/share/0be65ecbc0c94cb49a30bb7316a616e5)
  - [2026-09-18](https://www.loom.com/share/35e5f66432cc47e284303e545af0c0aa)
  - [2026-09-18](https://www.loom.com/share/64ca328cf63e498a972764993ec3892a)
  - [2026-09-18](https://www.loom.com/share/aa27220453994fcb8edb32b4d024817a)
- group calls, as they get shared:
  - [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5)
- research:
  - [2026-09-18](https://chatgpt.com/share/6aacb045-ac9c-83eb-a97d-8624c616a763)

### No puller reaches these
Said in the classroom, listed so `skool-pull.rs` stops asking — a drive folder and a google doc need a
session nothing here holds, and the rest are somebody else's player.

- [2026-09-18](https://drive.google.com/drive/u/0/folders/1jFd8aorlIyDqMB_OwfuFrzrFG0cPObkp) — Eric's example verification videos
- [2026-09-18](https://drive.google.com/drive/folders/1hsPix3WX9bpFGrFg4a4GWLnSti3fv55X) — recordings of calls being answered
- [2026-09-18](https://docs.google.com/document/d/1k5ZFhdIfE0zS-WIiUjTP3-6kZqQy3TOcEQIn1esvXBs/edit?tab=t.0) — the plumber onboarding script
- [2026-09-18](https://player.vimeo.com/video/1079014922) — address video shot in a different location
- [2026-09-18](https://vocaroo.com/118AEXIqXCDi) — the VA call whose audio the video missed
- tools named rather than sourced: [getghostme](http://getghostme.com), [textverified](http://textverified.com), `accsrush.com`

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
- `#` heading only when a file genuinely splits into phases (`# after`)
- every claim carries its source beneath it, link text = the date, so age reads at a glance:
  ```md
  - not wise to compete with people with 200+ reviews
    [2026-09-18](SOURCE-URL-AT-THE-SECOND-IT-WAS-SAID)
  ```
  > a real link, not a placeholder — an example URL under `## Sources` would get pulled
