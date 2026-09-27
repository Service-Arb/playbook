---
name: service-arb
description: Answer a service-arbitrage question out of the playbook — Google Business Profiles (GBP/GMB), verification (video, live, instant), suspension, reviews, naming, categories, niches, VAs, contractors, splits, gmails, proxies, phones. Use for "do we have…", "what do they say about…", a message someone sent that needs a reply, or any question whose answer lives in structured/ or ref/.
---

# service-arb

A knowledge base on running Google Business Profiles into subcontracted trade work. Every claim in it
links the second someone said it, and the date. The boundary broken first: answering from general
knowledge where the corpus is silent.

## Sources

| path | what it is | read it |
|---|---|---|
| `structured/approved/` | notes the owner vetted, by topic | **first** |
| `structured/suggested/` | an agent's reading of the sources, unvetted | when approved has nothing |
| `ref/` | the raw captures: calls, course lessons, videos, chats | for what the notes never distilled, and to check the newest word |
| [sections/](sections/) | how to decide, per area | before advising, not just quoting |

The topics under `structured/` are `market` (niches, cities), `profile` (creation, verification,
suspension), `ranking` (naming, categories, reviews), `conversion`, `fulfilment` and `infra`.

## Answering

Search structured/approved first, then structured/suggested, then ref/. Every note already links its source, so the notes find it fast and the source is what you cite.

- cite under the claim it backs, one line, the bare URL at the second: `https://www.loom.com/share/<id>?t=3302`. Not `[r3 2026-09-18](…)`: link text is lost when the answer is copied out, and `r3` means nothing to the reader
- a claim with no source gets no citation. Say where it came from ("Valera, unrecorded")
- reliability: a citation's `r<base>` is 0–7, lower is better (0 tested ourselves, 1 skool course, 2 Eric, 3–4 a gmbpp member, 5–7 the cheap group). effective = base + age in days / 180. Cite the best first, the others under it as secondary
- sources disagree: the better effective number leads, the other stays beneath it, demoted, with its date. Between equal bases, newest wins
- take at most two hits per source, so one long call does not drown the rest
- before answering, run one more pass for the newest word on the topic: search ref/ for it and check nothing dated after your newest citation says otherwise
- nothing in the corpus answers it: say so, and name the nearest thing it does cover. Never fill the gap from general knowledge
- advising rather than quoting: read the matching section first
- the question is past the basics and someone here has run it: close with who to ask, per the experts section. otherwise the answer never mentions referrals

## Sections

Loaded on demand, one decision area each. Lines ending `// inferred` are drafted, not yet vetted.

| section | covers |
|---|---|
| [market](sections/market.md) | which trade, which city, whether to enter |
| [profile-creation](sections/profile-creation.md) | getting a listing to exist |
| [verification](sections/verification.md) | video, live, instant |
| [suspension](sections/suspension.md) | what links profiles, what to do once one goes down |
| [reviews](sections/reviews.md) | rate, sourcing, deletion |
| [ranking](sections/ranking.md) | name, categories, what to measure |
| [conversion](sections/conversion.md) | calls, VA, site |
| [fulfilment](sections/fulfilment.md) | contractors, splits, getting paid |
| [infra](sections/infra.md) | gmails, devices, proxies, numbers, cards |
| [experts](sections/experts.md) | who to refer someone to, and when not to |

## Tools

Only in a checkout; the remote server has none of these.

- [call-digest](../../.claude/skills/call-digest/SKILL.md): a capture whose `read by:` says `nothing yet`
- `../verif_tools`, only if it exists: `prepare-verif` builds the paper pack for an address, `submit-live` fills Google's live-verification form in your Chrome
- `../accounting`, only if it exists: `accountant`, for what was spent and is owed
- `scripts/photo-metadata.rs <dir> --city <city>`: the EXIF of a directory of photos, and synthetic per-city sidecars
