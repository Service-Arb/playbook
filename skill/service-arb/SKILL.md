---
name: service-arb
description: Answer a service-arbitrage question out of the playbook — Google Business Profiles (GBP/GMB), verification (video, live, instant), suspension, reviews, naming, categories, niches, VAs, contractors, splits, gmails, proxies, phones. Use for "do we have…", "what do they say about…", a message someone sent that needs a reply, or any question whose answer lives in structured/ or ref/.
---

# service-arb

A knowledge base on running Google Business Profiles into subcontracted trade work. Every claim links
the second someone said it, and the date. Never fill a gap from general knowledge.

> **The record tilts to the unusual.** What works goes unsaid as common knowledge; a failure gets
> told. One rejection story is an anecdote, never a requirement.

## Sources

| path | what it is | read it |
|---|---|---|
| `ref/skool_*/course/` | the courses, and the recordings their lessons link | **first** |
| `structured/approved/` | notes the owner vetted, by topic | next |
| `structured/suggested/` | an agent's reading of the sources, unvetted | when approved has nothing |
| `ref/` | the raw captures: calls, videos, chats | for what the notes never distilled, and the newest word |
| [sections/](sections/) | how to decide, per area | before advising, not just quoting |

The topics under `structured/` are `market` (niches, cities), `profile` (creation, verification,
suspension), `ranking` (naming, categories, reviews), `conversion`, `fulfilment` and `infra`.

## Answering

Reliability decides the answer, not recency or how vivid a story is. `r<base>`, lower is better: 0 we tested, 1 course, 2 Eric, 3-4 a gmbpp member, 5-7 the cheap group. effective = base + age in days / 180: a course lesson a year old beats yesterday's member call.

- read the course first: `ref/skool_*/course/`, then the captures its lessons link (`video:` and inline URLs, found in ref/ by id). Those are course material too
- then structured/approved, suggested, the rest of ref/. two hits per source at most
- last pass: nothing dated after your newest citation says otherwise
- build from the best effective number down. What only r3+ says never becomes a checklist item or a "don't": it goes after the advice as a caveat, with who and when. Sources disagree: the better number leads
- attribute each claim to who said it, as often as they said it, from the transcript
- corpus silent: say so, name the nearest thing it covers
- advising: read the matching section first

The answer is three parts, split by `---`:
1. **sources**: numbered, best first. Each: the claim, who said it (course, Eric, a member by name), the date, the bare URL at the second on its own line. Unsourced: say whose it is ("Valera, unrecorded")
2. **advice**: no URLs, `[n]` into the sources. Say how strongly each point is backed
3. **leads**, at most three: what the reader should open or ask next, ranked by what it adds past this answer. A course lead is the skool.com lesson page, not its recording: it holds the video, links and the lessons beside it. A call mined dry adds nothing. A person, per the experts section, adds what nobody recorded. One line each: what they'd get, then the URL or handle

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
