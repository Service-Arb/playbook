# Jeremy AI — product mechanics (what actually happens when you ask it something)

Captured 2026-09-18 by driving a headless Chromium over CDP (network capture + DOM dumps + a patched
`window.fetch` to tee the SSE stream), plus raw bundle archaeology and yt-dlp captions.
Site copy itself is already catalogued in `jeremyhaynesai-site.md` — this file is about the *engine*.

**The single biggest finding: Jeremy AI is not a custom build.** It is a white-labelled
**Delphi.ai** clone (channel `ec979a1a-55f2-4d3c-a49c-5c0a2f01e536`, backend `api.delphi.ai`), in
the middle of being migrated onto Haynes' own agentic platform **Utari** (`jeremy.utari.ai`). So
"rebuild an equivalent" means: rebuild Delphi's retrieval + citation + voice stack over our corpus.
Everything Delphi does is observable, and most of it is in the client bundle.

```
                    jeremyhaynesai.com  (Vite SPA on Vercel, marketing only)
                              │
              ┌───────────────┼────────────────────┐
              │               │                    │
        Whop checkout    Delphi embed         Arcade tour
        $1,000/mo        (iframe, chat)       of the NEW app
        $10,000/yr       EMAIL-GATED          jeremy.utari.ai/dashboard
              │               │                    │
              ▼               ▼                    ▼
        Kajabi portal   api.delphi.ai        Utari Eternal workspace
        (delivery)      RAG + citations      Workers / Triggers / Skills
                        voice via LiveKit    Slides/Docs/Video/Image
                        Telegram / SMS       = agentic actions
```

---

## the surface

| surface | URL | state |
|---|---|---|
| marketing + checkout | `https://www.jeremyhaynesai.com/` | public. Vite/React SPA, Vercel, 14 client routes, most unlinked |
| **public chat demo** | `https://www.jeremyhaynesai.com/checkoutmim` | renders, but **cannot answer** — see below |
| the Delphi iframe behind it | `https://www.delphi.ai/embed/ec979a1a-55f2-4d3c-a49c-5c0a2f01e536?landingPage=CHAT` | same |
| Delphi public profile | `https://www.delphi.ai/jeremyhaynes` | **404** — his clone is not in Delphi's public Discover directory |
| the actual paid product (old) | Kajabi portal `jeremy-haynes.mykajabi.com/ai-jeremy-haynes-IC` | paywalled |
| the actual paid product (new) | `https://jeremy.utari.ai/dashboard` | paywalled; a full UI tour is public via Arcade |
| public UI tour of the new app | `https://demo.arcade.software/uIOx59e9NEmJtPx7VFTA` | public, 12 real screenshots, empty state |

**Pricing / packaging** (from the live routes, 2026-09-18):

- `/plans`, `/unlockjeremyai`, homepage: **$1,000/mo**, or **$10,000/yr** ($833/mo). Whop checkout,
  `plan_0m3VHNFkEPWSu` / `plan_rJmFWyRAavknw`, biz `biz_HHTPQ5Kh9eZW5F`.
- `/plans3` is a **stale route still serving the launch price: $300/mo or $2,000/yr.** That matches
  his own launch video. So the product 3.3×'d its price between launch and now.
- Nothing is tier-gated inside the product — both plans list identical features. The gating is
  *upward*: Jeremy's Inner Circle at $10K/mo bundles Jeremy AI + Utari + an unadvertised
  **"Jeremy MCP"**; the agency is $20K/mo + revshare; cloning *yourself* is **Utari Eternal ~$25K/yr**,
  application-only. Team seats are "contact your CSM victor@jeremy.expert" — i.e. not self-serve.
- **No free tier, no trial, no card-free demo that actually answers.**

### how far I got inside — and the exact wall

The `/checkoutmim` page really does render a live Delphi chat ("Chat With Jeremy AI / Ask it how to
increase your show rate"). It is **not** usable anonymously. The first thing it says is:

> Before we continue, what is your email address? I'll use it to provide personalized advice and save
> your conversation history.

That gate is **server-enforced, not cosmetic**. I minted a fresh anonymous session token from
`POST api.delphi.ai/chat/bootstrap` and posted a question straight at the chat endpoint, bypassing
the UI entirely:

```
POST https://api.delphi.ai/chat?channelId=ec979a1a-55f2-4d3c-a49c-5c0a2f01e536
Authorization: Bearer <bootstrap token>
→ HTTP 401
{"type":"authentication","code":"embed_email_required","message":"Email is required to continue."}
```

Getting past it needs a real mailbox (`/embed/account/email/start` → emailed code →
`/embed/account/email/verify`). I did not do that: no user data, and it would plant a lead record in
his CRM. **So I have never seen Jeremy AI's own words.** Everything below about answer *shape* comes
from the same platform, same code path, different clone — see next section.

---

## a real answer, verbatim

**⚠️ Read this framing before quoting any of it.** The transcript below is **not Jeremy AI**. It is
another Delphi clone — Brian Halligan (HubSpot co-founder), `delphi.ai/bhalligan/chat` — which is
public and answers anonymously. Same Delphi backend, same `api.delphi.ai/chat` endpoint, same
renderer, same citation machinery. It is a faithful sample of the *mechanics and output shape* Jeremy
AI runs on, and nothing more. **There is no verbatim Jeremy AI output anywhere in public** — I
checked his whole YouTube channel, the Wistia VSLs, the Arcade tour and the testimonial screenshots;
every one of them is him talking *about* the clone or users praising it.

### Q1 — a substantive domain question

> **You said:** What is the single biggest mistake founders make with inbound marketing, and what
> should they do instead?
>
> The single biggest mistake founders make with inbound is treating it as a content factory instead of
> a founder-led conviction engine. They delegate it too early to a marketing department and expect it
> to sing.
>
> At HubSpot, Dharmesh and I did most of the inbound lifting ourselves in the early days. We were
> motivated, we were creative, and we got a high return on calories. The thing we got right was naming
> what was already in the ether: inbound marketing, pulling people in through search, social, and
> blogging, versus outbound interrupting them. We deliberately didn't trademark it; we wanted everyone
> to use the term so we'd be the company most known for it. **[6] [10]**
>
> So instead of delegating, founders should do the work themselves first. Write the posts, give the
> talks, build the audience, and inject your own standards into the content. Inbound works when it
> carries the founder's voice and obsession, not when it's processed through a committee.
>
> What kind of company are you building, and where are you in that journey?
>
> *Suggested:* What are the best practices for a startup founder to evolve into a scale-up CEO? /
> What are the biggest pitfalls on the CEOs journey from startup founder to scale-up CEO? / How can I
> create my company's second act?

The `[6] [10]` are rendered as small numbered citation pills (`span.citation-badge`) sitting inline
at the end of the sentence they support. Note the shape, it's consistent across every answer I got:

1. **thesis sentence**, contrarian and named ("content factory" vs "conviction engine");
2. **a personal war story with proper nouns** (Dharmesh, HubSpot) — this is the paragraph that carries
   the citations;
3. **prescription, imperative mood**;
4. **a question back at the user.** Every single answer ended with one. It is a sales motion baked
   into the persona.
5. **auto-generated follow-up chips**, regenerated per answer.

Asked the identical question twice, ~30s apart, in the same thread: same claims, different prose,
different citation count (`[6]` only the second time). Not deterministic, not cached.

### Q2 — "what sources are you drawing on, and how recent is your knowledge?"

> I draw from a corpus of my own content: LinkedIn articles, tweets, talks at INBOUND, MIT Sloan
> lectures, podcast episodes like Long Strange Trip, and other recordings and writing I've put out
> over the years. My knowledge has a cutoff. The newest material I have is from around April 2026,
> and a lot of my HubSpot-era specifics are anchored in the 2006-2021 period. For anything happening
> right now, I might have a useful lens but not fresh data. **This is drawn from the description of
> my personal content corpus used to build this digital mind.**

This is the closest thing to honest epistemics the platform produces, and it is worth copying: it
names *media types*, gives a **concrete cutoff month**, gives the **era its specifics are anchored
in**, and flags the limit ("a useful lens but not fresh data"). That last bolded sentence is the
model telling you it is reading a corpus *description*, not counting documents — i.e. the recency
claim is prose in the system prompt, not a computed `max(doc.date)`. Worth doing properly in ours.

### Q3 — the free-tier wall

Third message in the same anonymous thread, straight off the wire:

```
{"type":"rate_limit","code":"quota_exceeded","message":"Message limit reached. Please sign up to continue."}
```

Two free messages per anonymous visitor on a public Delphi clone. Jeremy's embed sets
`messageLimit: 0` but `requireEmail: true`, so his gate is the email, not a counter.

---

## how it cites and dates its claims

This is the part worth stealing wholesale. I opened the citations drawer on a real answer and dumped
it.

**In the answer text:** inline numeric markers. The bundle literally strips them for text-to-speech
with `text.replace(/\s*\[\d+\]/g, "")`, which confirms the model emits `[n]` into the prose and the
renderer swaps them for pills.

**On the wire:** a Vercel AI SDK data part, `data-citations`, streamed alongside the text. Two
schemas are in the bundle:

```js
// public/API shape
{ answer: string,
  citations: [{ url?, text, created_at?, type, title?,
                page_num?, timestamp?, tweet_id?, citation_url? }] }

// embed shape
{ text, title, id_uuid, summary?, meta_data: { url, start, end, handle, ... } }
```

`created_at` = the date of the *source*. `page_num` = PDF/DOCX page. `timestamp` / `meta_data.start`
and `.end` = **millisecond offsets into audio/video**.

**In the UI:** a right-hand drawer, grouped by source, headed **"This source"** then **"More
sources"**. Verbatim from the drawer, on a real answer:

> **Citations**
> **This source**
> HubSpot Co-Founder Brian Halligan: How Culture Scales Faster Than Code and Built a $20B Company
> — *2 clips*
> · Jump to **40:30 – 41:34**
> · Jump to **40:15 – 40:18**
>
> **More sources**
> The Philosopher CEO | Clay Co-Founder Kareem Amin — *1 clip*
> HubSpot ft. Brian Halligan & Dharmesh Shah - How an underdog helped invent modern marketing — *1 clip*
> Innovating and Iterating for Growth with Hubspot Co-Founder Brian Halligan | Revenue Builders Ep. 83 — *1 clip*
>
> x.com @bhalligan
> "No matter what kind of company you are...start making your internal company data legible to AI. Today. …"
>
> delphi-training-content-prod.s3.amazonaws.com — File — Untitled document _19_.docx
> "Page 1: Start with Problems, Not Solutions …"
>
> sequoiacap.com — File — Untitled document _1_.docx
> "we thought we could just re-run the same playbook for 'inbound sales.' Failed. …"

Clicking a timestamp mounts an **actual YouTube iframe seeked to the clip**:
`https://www.youtube.com/embed/RMOvO3tFntQ?enablejsapi=1&start=2430` (2430s = 40:30). Web sources get
`?utm_source=delphi.ai` appended. Podcasts get a parallel `parsePodcastSource` path with the same
timestamp jump. So: **yes — it names the call, quotes the excerpt, and links to the second.**

Source types the renderer knows: `YOUTUBE, PODCAST, VIDEO, TWEET/X, INSTAGRAM, TIKTOK, LINKEDIN,
ARTICLE, WEBSITE, PDF`. Source files live in an S3 bucket, `delphi-training-content-prod`.

There is also word-level `alignment` (`startTimes[]`/`endTimes[]` per word) for karaoke-style
read-aloud, with citation markers excluded from the alignment.

**...and Jeremy has all of it switched off.** His bootstrap response:

```json
"showCitations": false,
"suggestQuestions": true,
"singleBubble": false,
"canVoiceCall": true,
"embedAccess": {"requireEmail": true, "requirePhoneNumber": false},
"embedSession": {"messageCount": 0, "messageLimit": 0},
"introMessage": "What's up, I'm Jeremy Haynes. What's the biggest challenge you're facing in your business right now?"
```

`showCitations: false`. On the marketing surface at least, Jeremy AI **deliberately does not cite**.
Which tracks: his moat pitch is "this is private paid material you can't get anywhere else" — showing
the receipts would be showing the goods. No public artifact anywhere claims the paid product cites
either, and no user testimonial mentions a source link.

### what Delphi's own docs say about all of this

Pulled from `docs.delphi.ai` (30 pages, GitBook, `.md` mirror per page). This is the vendor's own
account, and it lines up with what I saw on the wire.

**Retrieval is hybrid, and chunked.** The one place they leak architecture is the Search API
(`POST /v3/search/query`, Immortal tier only): `query` is *"Semantic (meaning-based) search"*,
`keywords` is exact-match *"via BM25 boosting"*, and when both are supplied results are
*"merge[d] and deduplicate[d]"* and re-ranked. `limit` is **"1–50 chunks, default 10"**. The response
returns `chunks[]` (text, sources, **timestamps**) plus `content[]` (deduplicated source metadata, so
one round-trip gives you both the evidence and the citation cards). A `tag` param scopes retrieval by
access tier. Chunking is confirmed; sizes, overlap and embedding model are not published. The vector
store is never named — only *"Creator content is stored in its own private index and is not shared or
used to train any external models."*

**Citations are a toggle, with per-item override.** Response Settings: *"**Show citations** — Toggles
whether visitors can open the cited sources used to generate a response."* And every knowledge item
carries a **Citation URL** field: *"If citations are enabled, visitors can click them in your Delphi's
responses to see the source. You can update the citation URL here or hide it entirely."* That second
one is worth copying directly — the place a transcript *came from* is rarely the place you want to
send a reader.

The documented citation union (`GET /v3/conversation/{id}?include_citations=true`) is slightly
different from the embed's, and tighter:

```
type          WEB | PDF | TWITTER | CONTENT
text, title, url, citation_url
page_num      ← PDF page
timestamp     ← video/podcast offset
tweet_id      ← X
created_at    ← ISO8601, date of the source
```

A discriminated union over *medium*, not a generic `{title, url}`. That is the design decision that
turns a footnote into a deep link.

**The answer-shaping knobs**, in panel order: Purpose · Custom Instructions (*"a maximum of three"*
recommended) · Style · Initial Message · Message on No Answer · Response Length · Creativity · Show
citations · Disclaimer · Recency Bias. Two of them matter for a RAG rebuild:

- **Creativity** — *Strict* (*"it only says things it is trained on that directly answer the
  question"*) / *Adaptive* (*"it can infer how you might answer new situations based on your training
  data"*) / *Creative* (*"it can augment responses with outside knowledge"*). Note the wire enum is
  `STRICT|BALANCED|CREATIVE` — "Adaptive" is the UI label for `BALANCED`.
- **Response Length** — `INTELLIGENT|CONCISE|DETAILED` on the wire; the UI reads Intelligent /
  Concise / **Explanatory** / **Custom** (a midpoint slider the enum doesn't expose).

**The email gate is a quota, not a wall** — and this explains Jeremy's. Delphi has four fixed access
groups: **Just Me / Insiders / Public / Anonymous**, each with a monthly message and voice-minute
allowance. *"By default, Anonymous is set to 0 messages. Go to Usage Limits, edit the Anonymous group,
and increase the message allowance to allow anonymous visitors to chat."* **Public** means
*"Email-logged-in users"*. So "give us your email" is a capacity decision, not a modal — and content
itself is assigned per access group on the Knowledge page. Elegant; worth copying.

**Per-visitor memory is first-class and typed.** The Audience API stores facts under 13 enumerated
categories: `GOAL, PREFERENCES, INTERESTS, PERSONAL_INFO, EXPERTISE, SITUATION, BELIEF,
COMMUNICATION_STYLE, EMOTIONAL_STATE, RELATIONSHIP, WHY_DELPHI, HOW_DELPHI, JOURNAL`, plus tags and
custom properties, with a creator-side CRM ("Audience") showing message count, last active, past
conversations and *"saved conversation memory"*. Webhooks fire on Contact Created / Updated / Phone
Number Captured / Contact Became Inactive.

**The unanswered-question flow is undocumented.** The endpoints exist in the client bundle, but
Delphi publishes nothing about them — the docs' own generative search answers *"I can't find any docs
about a 'question queue', 'unanswered questions', or 'knowledge gaps' workflow"* and lists
`skip-unanswered` / `owner-reply` / creator-inbox as *"not found in accessible documentation."* What
*is* documented is the pull-side half: **Message on No Answer** (*"what your Delphi says when it does
not have enough information to answer. Use it to suggest a different question"*), **Edit This Answer**
(right-click an answer in Conversations, revise it, the revision is *"stored as a Q&A content item in
your Mind"*), and **Training Mode** for plain-language corrections. The only API way to inject creator
words into a live thread is `POST /v3/conversation/{id}/append-clone-message` — and the injected
message is attributed **`sender: "CLONE"`**, not to the human. Delphi itself hasn't decided whether an
owner reply should look like the clone or like the person. We should decide deliberately.

**Dating / staleness.** The platform has no "as of `<date>`" in answers, and its only automatic
mechanism is a single always-on flag:

> **Recency Bias (Always On)** — "Prioritizes ideas and information from your most recent content."

There is no supersede, no retire, no decay, no staleness prompt. Deleting a connected feed does *not*
remove already-ingested content — you delete items by hand. Contradictions are treated as a
*prompting* problem: the docs' guidance on conflicting answers is *"Pick one default and rewrite until
there's one clear default."* **This is the weakest surface on the product**, and with a corpus of
dated transcripts plus course lessons that supersede each other, it is exactly where we would feel the
pain first. Note the data is there — search results expose `createdTime` / `editedTime` per item and
citations carry `created_at` — it is simply never surfaced in the answer.

Beyond the platform default, Jeremy layers two things of his own at *ingest*: 

- Delphi's "three layers of the mind" — Haynes, official VSL: *"Layer one are things that we want to
  actively be talking about… there's a new algorithm update that just happened last month. That's top
  of mind versus an algorithm update that happened in twenty nineteen is bottom of mind."* A
  recency-weighted priority tier on the corpus, not a date stamp on the answer.
- Utari's daily auto-sync: *"Every day that I document stuff, we put it into a Google Drive folder…
  it's called auto-sync. **My clone is one day behind real Jeremy.**"* The site sells this as "Daily
  updates. It's trained on everything I do and is one day behind real Jeremy." (`/logininstructions`)

### and the new platform cites even less

Utari, where the product is moving, is worse on provenance, not better. Across 26 public help-centre
articles, both legal documents, the marketing site, the blog and the changelog there is **nothing**
describing an answer that names its source, quotes a call, gives a timestamp, or dates itself. The
word "sources" appears only as an *ingestion* concept. Three corroborating signals:

- The only source-related member permission is a switch to **suppress** them: Instance Config →
  Access has exactly three toggles — *"Allow creating workers / Allow creating triggers / **Hide
  knowledge base sources**"*. Something is exposed somewhere; no article says what it renders.
- The documented "I don't know" behaviour is a canned string, same as Delphi: a single **Message on
  No Answer** field, with guidance to *"Acknowledge that the AI does not have enough information;
  **Avoid guessing**; Tell the member what to do next."*
- Power users bolt provenance on **from outside**. The only occurrence of "Data-as-of timestamp and
  sources used" anywhere in Utari's corpus is inside a user-authored MCP prompt template that treats
  Jeremy AI as an unauditable black box: *"Keep source facts, calculations, **JeremyAI advice**, and
  your final synthesis **visibly separate**."* / *"Reconcile JeremyAI's reply against the source data…
  **Flag disagreements or unsupported claims instead of smoothing them over.**"* / *"**Do not infer
  JeremyAI-internal skill execution without direct evidence.**"*

Utari's official line on changed answers is explicitly *not* a recency story
(`support.utari.ai/…/why-did-jeremyai-change-its-answer`):

> "**The first reply only uses what you put in that first message.** If you later connect an ad
> account, paste real numbers, upload an offer or funnel, or add missing context, the next answer
> should change. That is expected. It is not a glitch, and it does not mean the first reply was a lie."
> … "**That first answer is a starting point, not a locked report.**" … "**JeremyAI is not Meta Ads
> Manager or Google Analytics.**"

The troubleshooting advice is *"ask it to **show the assumptions** behind the first answer"* —
assumption-surfacing, not source attribution. **Nobody in this category does provenance well. That is
the opening.**

**Hedging when sources disagree:** never observed, and nothing in the config supports it. The closest
levers are per-surface knobs in `experienceOverrides`: `creativity: STRICT|BALANCED|CREATIVE`,
`responseLength: INTELLIGENT|CONCISE|DETAILED`, `messageOnNoAnswer` (a fixed string shown when
retrieval comes up empty), `purpose`, `style`, `customInstructions[]`, `locationGuidelines`. It is a
persona-and-retrieval product, not a truth-maintenance one.

**The "I don't know" path is a human-in-the-loop, and it's the best idea here.** The API has
`/message/{id}/skip-unanswered`, `/thread-preview/{id}/unanswered` and `/chat/owner-reply`: the clone
admits it can't answer, the owner gets emailed the question, the owner can answer it, and the answer
goes back to the asker. Haynes on this, on camera: *"These represent what is known as, uh, inability
for my clone to have answered a question"* — 100 of them on Apr 1, 78 by Apr 5. He treated the pile as
a product backlog, and the causes were mostly **capability limits, not knowledge gaps**: an
undisclosed character-count cap (*"too long of a character count, buddy. Sorry."*), PDF size limits,
no spreadsheets or slides, and no agentic actions. That queue caused churn and a refund inside an
hour of purchase. **An unanswered-question queue is cheap, and it is the highest-signal telemetry the
whole product produces.**

---

## capabilities matrix

| capability | Delphi build (what was live) | Utari build (what it's moving to) | evidence |
|---|---|---|---|
| Q&A chat | yes | yes | observed / Arcade tour |
| streaming | yes, SSE via Vercel AI SDK | assumed | wire capture |
| citations | **supported, and Jeremy disabled them** | not claimed anywhere | `showCitations:false` |
| timestamp links into A/V | yes, inline seeked player | unknown | drawer capture |
| memory across sessions | yes — `/visitor/{id}/fact`, `/property`, `/tag` per-visitor stores | claimed ("Memory" pillar) | API surface + site copy |
| user-specific context | yes; email gate exists *to* key it — "I'll use it to provide personalized advice" | yes | bootstrap + copy |
| file upload | yes — `/chat-upload/presign`, `/index`, `/read-url`; **but** size/type caps that drove churn | yes, explicitly "don't have those limitations" | API + his own video |
| images / screenshots in | yes — he pasted an ad-account screenshot, got a multi-paragraph audit back | yes | launch video |
| voice, in-app | yes — `/voice-call/session/start`, `/token`, `/quota`, `/languages`, LiveKit | "New Voice Call" button | API + UI |
| voice, real phone number | yes, Twilio (toll-free + A2P 10DLC verification objects in the schema) | yes | bundle + site |
| video / Zoom avatar | **existed, then Delphi removed it** — *"We don't have this anymore… The platform we were using rolled it back."* | unknown | launch video |
| Telegram 1:1 + group | yes, preferred channel | yes | site + testimonials |
| Slack | possible, actively discouraged — *"Most of the time we deny those people."* | yes | launch video |
| SMS / WhatsApp / email broadcast | yes — broadcast objects with `channelType: sms|email|whatsapp` | — | bundle |
| MCP server | **no** — Delphi ships none | **yes, "Jeremy MCP"** — `persona-mcp.utari.ai/mcp/`, 3 tools, 90-day bearer token | Utari help centre; Telegram screenshots on his own site |
| agentic actions | **no.** Haynes asked Delphi's CEO if agents were coming: *"he said, 'No.'"* | **yes** — Slides, Data, Docs, Canvas, Video, Research, Image; Workers / Triggers / Skills; Google Drive picker | launch video / Arcade tour |
| multi-seat / team | via group chat, priced ad hoc by a CSM | "Team mode", unlimited seats | site |
| multilingual | yes, 13 call languages enumerated + `multipleLanguages` flag | — | bundle |
| per-audience "instances" | yes — scoped mini-clones with a slice of the corpus, time-boxed (72h webinar access, 3-day challenge), used to upsell inside the funnel | — | launch video |

Two corrections from Delphi's own docs, against Jeremy's on-camera complaints:

- **File limits.** Docs today say *"videos, audio files, documents, and more. Maximum 5GB per file"*,
  PDF/DOC/DOCX/TXT "and other common formats", and in-chat attachments via a `+` button (*"You can
  share documents or images and ask the Delphi questions about them"*). Jeremy's churn-causing
  character-count cap and "no spreadsheets or slides" are from spring 2026 and may since be fixed —
  don't treat them as current platform limits, treat them as **evidence of which limits users hit
  first**.
- **Video avatars weren't rolled back for him specifically.** Delphi's changelog, 2025-09-01:
  *"Removing Video Avatars… Video functionality was removed in September 2025, with plans to relaunch
  later."* The Usage API still meters `video seconds` — vestigial.

**Ingestion surface** (what a Delphi clone can eat, for comparison with ours): one-shot files ≤5GB,
YouTube links, podcast episodes, single URLs, Quick Notes, Q&A typed or bulk CSV, Loom and Vimeo;
auto-syncing accounts for Website / X / Instagram / TikTok / Substack / YouTube channel / Podcast
series; cloud sync for **Google Drive, Notion, Granola, Obsidian**. Newsletter exports from HubSpot /
Salesforce / GoHighLevel / ConvertKit / Beehiiv, and voice memos. **No LinkedIn, no Slack/Telegram
export, no email-inbox ingestion** — which means Jeremy's "85,000+ DMs and emails" and Telegram/Slack
history were bulk-loaded as files, not via a connector.

The **instances** idea is the most transferable product move in the whole teardown: same corpus,
deliberately *narrowed* knowledge + a different system prompt, handed out free as a lead magnet with
a TTL, IP/VPN abuse blocking, and a sales objective inside the persona.

---

## network + model fingerprints

**jeremyhaynesai.com** — Vercel, Vite/React SPA, single bundle `assets/index-CDUS1gKF.js` (356 KB),
`og:url` still points at `jeremy-ai.netlify.app`. Third parties: Whop checkout + `t.whop.tw` pixel,
Wistia (`fast.wistia.com`, VSLs incl. `7gmr4yvgcy` = "jeremyaimainvsl"), Arcade, a Typeform
application (`ahbigalex10.typeform.com/to/VZFsfO4b`), a Zapier catch hook, `embed.delphi.ai/loader.js`.

**Delphi** — Next.js on Vercel, Sentry, PostHog (`phc_ym2L4Ep23SRmhpndM8zH9zPjxdHjLCKU2y8nsJ2ATdAw`)
with session recording. Embed loader is versioned `embed-script@0.0.0+ddd3c95+2026-09-17`.

Flow:

```
embed.delphi.ai/loader.js
  → GET  /api/embed/runtime?channelId=…            → {"data":"legacy"}   (runtime selector)
  → iframe www.delphi.ai/embed/<channelId>?theme=light&landingPage=CHAT&parentOrigin=…
  → POST api.delphi.ai/chat/bootstrap?channelId=…&origin=…  → token, threadId, theme, access flags
  → GET  api.delphi.ai/voice-clone/availability?owner_username=jeremyhaynes  → {"available":true}
  → POST api.delphi.ai/chat            Authorization: Bearer <token>
```

Send body (captured live):

```json
{"id":"<threadId>",
 "message":{"id":"<uuid>","role":"user","parts":[{"type":"text","text":"…"}]},
 "messages":[ …full history, each with metadata{threadSessionId, createdAt, isUnanswered} … ],
 "channel":"embed"}
```

Note `isUnanswered` is carried **per message in the history** — the unanswered state is first-class.

Custom headers the API advertises in CORS: `X-Delphi-Chat-Surface`, `X-Delphi-Call-Surface`,
`X-Delphi-Experience-Channel`, `X-Delphi-Client-Trace-Started-At`, `X-User-Timezone`, plus exposed
`X-Workflow-Run-Id`, `X-Workflow-Run-Binding`, `X-Workflow-Stream-Tail-Index` (there's a durable
workflow engine behind the chat, with resumable streams — `/chat` supports `reconnectToStream`).

**Streaming format:** Vercel **AI SDK v5** UI-message stream (`DefaultChatTransport`,
`data-*` parts, rendered by `streamdown`). Citations arrive as a `data-citations` part. Errors arrive
as JSON on the same channel (`rate_limit/quota_exceeded`, `authentication/embed_email_required`).

**Model names: not exposed.** No `gpt-*`/`claude-*`/`gemini-*` string anywhere in 60 client chunks.
The bundle does carry Vercel **AI Gateway** client code (`https://ai-gateway.vercel.sh/v4/ai`,
`ai-sdk/gateway/4.0.40`), which is a multi-provider router — consistent with Haynes' VSL line
*"running on the most powerful models available"* and with swapping models without touching clients.
Provider names present in the bundle: **LiveKit** (realtime voice transport), **ElevenLabs** and
**Cartesia** (TTS / voice clone), **OpenAI** (string present; role unconfirmed). Twilio implied by
the toll-free / A2P verification schema.

**Vector DB: not visible.** Zero mentions of Pinecone/Weaviate/Qdrant/Chroma/Turbopuffer client-side —
expected, retrieval is server-side. The only storage fingerprint is the S3 bucket
`delphi-training-content-prod`. Marketing says calls are "transcribed, indexed, **embedded**"; the
citation objects carry `id_uuid` + `page_num` + `start`/`end` ms, so chunks are clearly
**span-level within a source document**, not whole-doc.

**What Jeremy is paying for underneath.** Delphi's public pricing: **Free $0** (real free tier, live
since 2026-03-03, SMS-verified, US/UK/Canada only — you can build a mind and chat with it), **Builder
$79/mo** (voice + chat, 1M training words, Slack, 2 embed locations, Products, Custom Alerts),
**Scaler $299/mo** (Pro Voice, 12M training words, Telegram, contact import, custom branding, 10 embed
locations), **Immortal, custom** (white-glove setup, unlimited training words, **SMS + WhatsApp**,
**API access**, SSO, custom applications). Jeremy is plainly on Immortal — he has SMS, Telegram,
a phone number and a white-labelled portal — and he resells at $1,000/mo. Add-ons: Pro Voice $150/mo.
API is `https://api.delphi.ai`, `x-api-key`, v3, **120 req / 60s per key**. **Delphi ships no MCP
server** (*"direct MCP-style integration is not currently documented"*) — so "Jeremy MCP" is a Utari
artefact, not a Delphi one.

Delphi's own "agentic" story is worth being precise about, because it's the gap Jeremy left them
over: **the clone talks, the platform acts.** No in-turn tool-calling is documented. "Actions" means
webhooks (9 events) plus **Hosted Actions** — *"React to an event, Run on a schedule, Wait, then
act"*, durable pauses up to a year, running *"in an isolated sandbox that can only reach Delphi and
the specific outside services you have allowed"*, able to tag contacts, generate text in the mind's
voice, *"notify you when a human should step in"*, and hit your CRM. The only in-conversation slot is
**Products** — an affiliate/offer recommendation with frequency control (every mention / once per
user / once per conversation) and an `Affiliate Product Mentioned` webhook. A recommendation slot, not
a tool call. Haynes asked Delphi's CEO directly for agent function and got *"No"*, which is why the
whole product is moving to Utari.

**Utari** — the new app is `jeremy.utari.ai/dashboard`. Left rail: **Chats / Workers / Triggers /
Skills**, "New Chat ⌘J", "New Voice Call". Composer: paperclip attach + Google Drive picker + a
**persona dropdown reading "Jeremy AI"** (so multiple clones/personas per workspace) + mic. Quick
actions: **Slides, Data, Docs, Canvas, Video, Research, Image**. Empty-state prompt: *"What do you
want to get done?"* Workspace footer: "Utari Eternal". Sidebar thread titles in the demo account —
"VSL Workshop Stru…", "Coaching Program …", "Creative Writer Wo…", "Ad Creative Request", "Funnel
Strategy Dis…", "Ad Hooks Request", "Course Launch P…", "Facebook Ad Ho…". Training data is compiled
into **`skill.md` files** per his own description — *"broken down into different skills that my agent
is now equipped to go and do when prompted."*

### Utari, specified

**It is a rebranded fork of Suna**, the Kortix open-source generalist agent. `utari.ai/suna` is a live
page titled *"Suna is now Utari"*: *"**Our name changed from Suna to Utari.** … Utari is the evolution
of Suna — the same open source AI assistant and generalist AI worker."* The blog footer still ships
`kortix-symbol.svg`. (Caveat: the GitHub link on that page 404s while `github.com/kortix-ai/suna`
exists — read it as lineage, not a clean ownership claim.) It was a $20–$200/mo self-serve SaaS with a
free tier until ~mid-2026; Haynes shut public signups down and repositioned it: *"The old $20, $50,
and $200 monthly plans no longer exist, and there is no free tier."* **Utari Eternal, $25,000/yr, by
application** — priced explicitly against Delphi: *"Delphi charges $25K annually for essentially a
chat interface with limited functionality. **Utari Eternal is priced at the same level** and delivers
agentic execution, voice, integrations, and none of those file or character limits."*

The primitives, verbatim:

> "**Workers** are agents with a defined role and intention, like auditing sales calls, monitoring CRM
> compliance, or analyzing ad accounts." · "The **knowledge base** is a master library where each
> worker only gets access to the material relevant to its job." · "**Triggers** are events that put
> agents to work automatically, like a new call transcript landing in a Google Drive folder." ·
> "**Integrations** cover CRMs, Slack, Gmail, Stripe, and ad platforms. If it has an API, it can plug
> in." · "**Credit-based processing** means the system reads everything you give it, every word,
> instead of summarizing to save costs."

A Worker has five configurable surfaces — **Tools, Integrations, Knowledge, Skills, Triggers** — and
the docs advise *"3-5 skills that directly support the job. If you add too many skills, JeremyAI has
more possible directions to choose from."*

**A Skill is literally a `SKILL.md` file.** *"Rather than simply storing knowledge in a database and
letting users query it, the system recalls information from the database and **references skill.md
files we create from your training data**. All training data provided to Utari Eternal is analyzed and
broken down into capabilities the agent can execute when prompted."* Skills are minted from **single
knowledge files** ("Convert to skill"; *"Skills are created from individual files, not entire
folders"*). Haynes publishes 38 of his own at `jeremyhaynes.com/skills/`, version-tagged, each with
`README.md` + `SKILL.md` + `sources.md` and the instruction *"Copy SKILL.md into Claude Code, ChatGPT,
Cursor, or any LLM conversation."* The `SKILL.md` bodies are email-gated; the harvested lead is the
point.

**The knowledge base has one structural rule and one genuinely good idea.** The rule:
*"Keep upload structure simple: **Folder → File**. … **Nested folder structures will not be read or
parsed correctly.**"* The good idea — every folder carries a **Content Priority**, one of exactly
three:

> **Knowledge** · **Voice / Style Source** · **Opinions & Beliefs** — *"use this when the folder
> contains strong takes, values, positioning, or judgment calls the AI should understand."*

Three retrieval roles for the same corpus: *what is true*, *how he talks*, *what he believes*. That is
a cheap, high-leverage distinction and we should copy it — our transcripts, course lessons and
research notes fall into exactly those three buckets. Files also carry an **editable Summary** that
the retriever uses to decide relevance.

**Ingestion**: three flows — *Add Knowledge* (one file / pasted text / Google Doc / YouTube video),
*Bulk Import* (**≤50 URLs per batch**), and *Sources* (ongoing Google Drive folder or YouTube channel,
**≤100 videos**, *"this is also where auto-sync comes from"*). Google Docs must be link-shared.
Historic limits: 100MB per file, 150,000 characters per document. The freshness claim resolves to a
**T-1 Google Drive sync** — *"we autosync a Google Drive folder each day… **The AI system reflects
content from one day prior**"* — driven source-side by his team exporting into Drive, with a
**read-only** Drive connector (*"Utari's Google Drive connector is intended to be read-only"*). So
Drive is an input, never an output.

**"Jeremy MCP" is real and precisely specified**: `https://persona-mcp.utari.ai/mcp/`, bearer token
minted in General Settings → MCP, shown once, **90-day lifetime**, revocable. Three tools:
`send_message` (start or continue a conversation via `conversation_id`), `get_response` (poll by
`run_id`), `persona_info`. Every MCP conversation *"is a real thread in your Utari dashboard"*, and
*"all usage is billed to the instance owner"*. Utari also consumes **external** MCP servers (HTTP or
SSE) per worker, with per-member private credentials. This is the shape of "let a customer's own agent
tool-call our expert" — one small server, three tools, tokens that expire.

**Bridges**: dedicated email address `yourname@mail.utari.ai` (approved senders only, +1 extra),
Telegram via BYO `@BotFather` bot (handles files, images, voice notes, group @mentions, `/new` to
reset), Slack via OAuth with a default worker and per-person self-linking by signup email. Every
channel's thread lands in the dashboard list with a **source icon**. **SMS and an inbound phone number
are marketing claims with no documentation behind them** — the help centre only ever describes
browser-based voice calls.

**Voice**: 30s–5min WAV/MP3/M4A sample → cloned voice → pick the **Voice Agent** (which supplies the
worker behaviour, personality and knowledge for calls) → **Greeting Message** and **Thinking Message**
(*"the filler message played while the AI is processing"* — latency papered over, not eliminated).
Metered in voice minutes. Vendors named in the Terms: *"**ElevenLabs, Vapi, Deepgram, Cerebras**, or
other vendors."* Consent machinery is real, and voice is flagged biometric under BIPA.

**Agentic scope**: *"Your clone doesn't just answer. It **runs ad accounts, edits CRMs, builds
reports**, and executes the work you'd otherwise sit at a computer to do."* `llms.txt` claims
*"**35+ Built-in Tools**"* and *"**Sandboxed Execution**: Secure Docker-based code execution"*. The
dashboard chips map to real models — **Veo 3.1** for video, **Nano Banana Pro** for images. Integration
layer is **Composio**; named connectors include Meta Ads (system-user token + `act_` id), Google Ads,
**GoHighLevel** (via HighLevel's own MCP, 36 tools), Fathom, Fireflies, Stripe, Whop, Shopify, Webflow,
Zendesk, Notion, GitHub, Google Calendar, Zapier. Notably, the most sophisticated documented build
clamps itself: *"**Operates read-only**: it never touches budgets, campaigns, or CRM records without
your explicit approval."*

**Models — Utari publishes an actual subprocessor list, which Delphi does not:**

> "AWS, Vercel, Supabase, Stripe, RevenueCat, Google, **OpenAI, Anthropic, OpenRouter, AWS Bedrock,
> Cerebras**, Vapi, ElevenLabs, **Deepgram, AssemblyAI**, **Llama Cloud / LlamaParse**, PostHog,
> Sentry, **Langfuse, Braintrust**, Umami, Google Analytics, Meta, and Google Ads."

Routing is **LiteLLM** (*"Multi-Model Support: Works with Anthropic, OpenAI, and other LLM providers
via LiteLLM"*). Haynes' own stated primary: *"We always run and operate on the **latest and greatest
available API from Claude, specifically from Anthropic**. So we are built on [Claude] as our
foundation."* Parsing is **LlamaParse**; transcription Deepgram/AssemblyAI; eval/observability
**Langfuse + Braintrust**. Embeddings and vector search appear only as a subprocessor *category* and a
retention row — *"**Embeddings / vector data | Deleted when the source content is deleted**"* — the
store is still never named.

**One thing a $1,000/mo buyer should know, and a thing we must decide on for ours.** Utari's privacy
policy, §11: *"the **Instance Owner and authorized administrators may be able to view Community Member
conversations, queries, memories, transcripts, usage**, and related interaction data"*, and Utari and
the owner are *"joint controllers"*. The owner's Analytics has a **Recent Queries** feed the docs
recommend mining for *"**High-intent members who may need follow-up**"*. Your questions to the clone
are the seller's lead telemetry. Also: *"Utari **does not currently fine-tune**… However… Utari **may
in the future** use… Community Member data to train, fine-tune, or improve models."*

---

## what we could not observe

- **Jeremy AI's own words. Not one sentence.** Blocked by a server-enforced email gate
  (`embed_email_required`, HTTP 401) which needs a real inbox to clear. Every verbatim answer in this
  file is Brian Halligan's clone on the same platform, and is labelled as such. **Do not let a
  Halligan quote get laundered into a Jeremy AI quote downstream.**
- Whether the *paid* product has `showCitations` on. The flag is `false` on the public marketing
  embed only; the paid Kajabi/Utari surface could differ. Nothing public suggests it does, and no
  user testimonial mentions a source link.
- Whether Jeremy AI ever says "as of `<date>`". Never observed; the platform gives it no mechanism,
  and recency is handled at ingest instead.
- Behaviour on out-of-domain questions, on questions where his advice has changed over a decade, and
  on contradictory sources. All three needed a live session with *his* clone.
- **Chunk size, overlap, embedding model, reranker, vector store.** Confirmed *that* it chunks
  (`limit` is "1–50 chunks") and *that* it's hybrid semantic + BM25, but Delphi's docs explicitly
  decline the rest, and their security page names only infra vendors (Cloudflare, AWS Shield,
  CloudTrail, KMS, Sentry, Axiom, Logfire, Snyk, GuardDuty, 1Password) — **no AI vendor, no vector
  DB, no subprocessor list.** They deliberately don't say. The bundle evidence
  (LiveKit/ElevenLabs/Cartesia/AI Gateway) is better than anything they publish.
- The actual model(s). Vercel AI Gateway routes them; no model id crosses the wire to the client, and
  no Delphi page names a provider.
- Third-party corroboration of anything. G2 403s, and the session's web-search budget was exhausted
  mid-investigation; the Delphi half rests on Delphi's own docs plus my own wire captures.
- Utari's **credit rate card** — billing is in "credits and Tokens" with a "token budget" in
  Analytics, but no rates are published. Its `docs.utari.ai` is **dead** (DNS wildcards to the
  marketing app) while still being linked from the footer, `llms.txt` and two blog posts; the four
  articles that would fully specify Worker mechanics ("Configure Worker Tools", "Add Integrations to
  Workers", "Activate and Assign Knowledge", "Create and Manage Triggers") are cross-referenced but
  not publicly reachable. `api.utari.ai` is FastAPI with every probed endpoint 404 — **no public REST
  API**; the only public HTTP surface is `POST /v1/triggers/{id}/webhook`.
- Whether Utari/Jeremy AI actually has **SMS or an inbound phone number**. Marketed on both sites,
  documented nowhere.
- Wistia `7uvgiv12gr` (`utarieternal_testimonial_jh`, 9:11, the Utari case study) publishes no
  captions; would need local ASR.
- Voice: never heard it. Needs the paid product or a phone call.

### the seven things worth copying

1. **A citation type that is a discriminated union over medium** — `timestamp` for A/V, `page_num` for
   PDF, `tweet_id` for X, `created_at` for the source's date — rendered as span-level source cards
   grouped by source, with the excerpt quoted and a player seeked to the second. We have hundreds of
   transcripts; this is the whole differentiator, and it is the one feature Jeremy turned *off*.
2. **A per-item `citation_url` override (and a hide flag).** Where a transcript was ingested from is
   almost never where you want to send a reader. Cheap field, large effect.
3. **The unanswered-question queue.** Clone says it doesn't know → question lands in an owner inbox →
   owner answers → answer ships back to the asker and into the corpus. Free roadmap, free eval set.
   Delphi has the endpoints and publishes nothing about it; we get to design it properly. Decide up
   front whether an owner reply is attributed to the human or laundered as the clone — Delphi's
   `append-clone-message` picks the latter and it's the wrong call.
4. **Instances**: scoped, time-boxed, narrower clones as lead magnets, each with its own system prompt
   and its own slice of the corpus. Generalises to Delphi's access groups, where **content is assigned
   per audience tier** and the anonymous tier's message quota *is* the email gate.
5. **Daily ingest ("one day behind")** as a *marketed* property, with recency-tiered priority
   ("top of mind" vs "bottom of mind") rather than flat similarity — but go further than
   `Recency Bias (Always On)`. Supersede and retire are the obvious gap in the entire category.
6. **Utari's three Content Priorities on a knowledge folder** — *Knowledge* / *Voice & Style Source* /
   *Opinions & Beliefs*. Three retrieval roles over the same corpus: what is true, how he talks, what
   he believes. Our transcripts / lessons / research notes map onto it almost exactly, and it costs one
   enum column.
7. **A stated corpus description** the model can honestly recite — media types, cutoff month, the era
   its specifics are anchored in. Ours should compute that from the corpus instead of hardcoding prose,
   which is what Delphi's clones are visibly doing.
