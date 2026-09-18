# Retrieval for LLM systems — state of the art, September 2026

Compiled 2026-09-18. Scope: what to build for a small-corpus expert assistant over a few
hundred markdown files (loom/meeting transcripts with per-second timestamps, course
lessons, research notes). Target corpus 1–3M tokens; **this repo measures 80 md files /
~163k tokens today** (`find . -name '*.md'`, 2026-09-18), which is itself the single most
decision-relevant number in this document.

Every non-obvious claim carries a URL and a date. Claims resting only on pre-2026 evidence
are marked **[pre-2026]**. Vendor self-reported numbers are marked **[vendor]**.

---

## the 2026 verdict in one page

```
            corpus size  ──────────────────────────────────────────────────────▶
            0         200k        1M          3M            50M          ∞
            │          │           │           │             │
 stuff it   ███████████                                                   trivially right
 all in     ░░░░░░░░░░░░░░░░░░░░░░░                                       right but slow+$$
                                   ╳ physically impossible (1M window cap)

 retrieve → whole docs → long ctx   ████████████████████████              ◀── YOU ARE HERE
 chunk RAG + rerank                        ░░░░░░░░░░████████████████████████
 graph / global summarisation                             ░░░░░░░░████████████
```

1. **RAG did not die, but "chunk-and-embed" as the *whole* architecture did.** The 2026
   consensus shape is a *funnel*: cheap wide retrieval → rerank → hand the generator
   **large, whole, contiguous units of text** (whole documents or whole chapters, not 400-token
   fragments) → let a long-context model do the reading. Retrieval is now a *filter that
   picks documents*, not a *compressor that picks sentences*.
2. **At 1–3M tokens you are past the stuffing threshold but well short of needing a
   vector database as the primary mechanism.** 1M is the largest production window
   (Claude Opus 4.6, beta, [anthropic.com, 2026-02-05](https://anthropic.com/news/claude-opus-4-6)),
   so 3M cannot be stuffed. But the corpus fits in ~3 windows. Any retrieval step that
   narrows to <200k tokens is sufficient; precision beyond that buys latency, not accuracy.
3. **Context rot is real but is being engineered away, unevenly.** Opus 4.6 scores 76% on
   MRCR v2 8-needle @1M vs Sonnet 4.5's 18.5% (same source), and ~93% @256k. Gemini 3 Pro
   was 26.3% @1M at its Nov-2025 launch. So: **a 200k-token stuffed context is now safe on
   a frontier model; a 1M one is not, and is 5–10× the price per token.**
4. **Hybrid (BM25 + dense) + reranking is still the strongest cost-adjusted baseline**, and
   2026 gave it a *proof*: [LIMIT / ICLR '26](https://arxiv.org/abs/2508.21038) (v2
   2026-03-12) shows single-vector embeddings **provably cannot represent all top-k subsets**
   above a dimension-dependent bound, and BM25 crushes every dense model on their dataset.
   Keeping a lexical leg is no longer a hedge, it is a ceiling you cannot reach otherwise.
   Empirically hybrid beats either leg by 15–30% recall
   ([cruxdigits, 2026-07-24](https://cruxdigits.nl/blog/rag-vs-graphrag-2026/)).
5. **Agentic retrieval wins on *hard* queries and loses on *cost*.** Controlled measurement:
   3.3× input tokens, 1.9× output tokens, 1.5× latency vs a well-tuned fixed pipeline, and
   it *underperforms* that pipeline at document reranking
   ([arXiv 2601.07711v2, 2026-04-20](https://arxiv.org/html/2601.07711v2)).
   Production reports stretch to 3–10× cost, 2–5× latency with much worse p95.
6. **GraphRAG's cost objection collapsed; its *value* case did not broaden.** Indexing went
   from ~$33k/dataset (2024) to parity with plain embedding (LazyGraphRAG/LightRAG), but the
   2026 practitioner verdict is that graphs earn their keep only on genuinely multi-hop,
   cross-document *synthesis* queries over entity-dense corpora. Not our shape — except for
   one narrow case (see §5).
7. **Two defaults people still reach for are now measurably stale.** OpenAI's
   `text-embedding-3-large` has had **no successor since January 2024** and sits ~14th on
   RTEB, ~14% behind the frontier. And `chunk → embed → top-k fragments` has been superseded
   by **contextualized chunk embeddings** (voyage-context-4, +7.11% on LongEmbed over single
   vectors) — chunk normally, embed each chunk conditioned on its whole document.
8. **Evaluation is the actual bottleneck, not architecture.** Every credible 2026 source
   converges on: hand-label ~100 traces, build a 50–100 question golden set, validate your
   LLM judge against human labels before trusting it (RAGAS metrics correlate with humans at
   a harmonic mean of only ~0.55 — [arXiv 2506.20128](https://arxiv.org/pdf/2506.20128)),
   and spend 60–80% of build time on error analysis
   ([hamel.dev, updated 2026-09-17](https://hamel.dev/blog/posts/evals-faq/)).

**The one-line call for this corpus: at 163k tokens today, stuff the whole thing into one
prompt-cached context and ship — that is a working assistant this week and your permanent
quality ceiling. As it grows toward 1–3M, add a hybrid keyword+dense search *tool* over
whole-document and chapter-level units, give it to an agent with a hard step budget, and have
it return whole units into a 200k context. Never index a derived summary in place of the
verbatim text. Do not build a chunk-level vector pipeline, do not build a graph, and build
the eval set before any of it.**

Two findings do more work than the rest, and both point the same way:
**chunking strategy dominates embedding-model choice** as a performance driver
([2603.06976](https://arxiv.org/html/2603.06976), 2026-03-07), and **verbatim text beats
LLM-extracted artifacts by 15.9–22.0 points** on conversation memory
([2601.00821](https://arxiv.org/abs/2601.00821), 2026-07-22). Where you cut the text and
whether you keep it intact matters more than any model on any leaderboard.

---

## 1. does RAG still matter at this corpus size?

### The hard constraint

The largest production context window in September 2026 is **1M tokens** (Claude Opus 4.6,
beta, developer-platform only, requires a header;
[anthropic.com, 2026-02-05](https://anthropic.com/news/claude-opus-4-6)). Gemini 3 Pro and
GPT-5.x families sit at 200k–1M depending on tier
([benchlm.ai pricing, Sept 2026](https://benchlm.ai/llm-pricing)).

So at 3M tokens **stuffing is not an option**. At 163k tokens — today's actual repo —
stuffing *is* an option and is the correct default. The interesting regime is the crossover.

### Context rot, 2026 status

The canonical study remains Chroma's *Context Rot* (July 2025,
[trychroma.com/research/context-rot](https://www.trychroma.com/research/context-rot)):
18 models, 8 input lengths × 11 needle positions, 194,480 LLM calls; performance degrades
non-uniformly with input length even when task complexity is held constant. Note the
conflict of interest — Chroma sells a vector DB.

2026 work refines rather than overturns it:

| finding | number | source |
|---|---|---|
| positional ("lost in the middle") vs length degradation are distinct failure modes | middle-position penalty 20–30 pts; length-only penalty persists with evidence favourably placed | [TMLS, 2026-06](https://www.tmls.nyc/research/context-rot-mechanistic) |
| effective context ≈ 50–65% of advertised (RULER cross-check) | — | practitioner replications, 2026 |
| length-only floor with distractors masked | −7.9% | ibid. |
| coherent distractors hurt *more* than shuffled ones | — | ibid. |
| context rot in long-horizon agentic search is a named, separate problem | — | [arXiv 2606.29718](https://arxiv.org/pdf/2606.29718) |
| safety/monitor classifiers also rot with length | — | [arXiv 2605.12366](https://arxiv.org/pdf/2605.12366) |

The important 2026 update is that **the frontier is pulling away from the pack**. MRCR v2
8-needle:

| model | @256k | @1M |
|---|---|---|
| Claude Opus 4.6 | 93% | **76%** |
| Claude Sonnet 4.5 | — | 18.5% |
| GPT-5.2 Thinking | 70% (8-needle) / 98% (4-needle) | — |
| Gemini 3 Pro | 77% | 26.3% (Google model card, Nov 2025) |

(Opus figures from Anthropic's own announcement and system card, 2026-02-05 — **[vendor]**,
independent replication not found. One third-party reviewer measured >90% to 500k and 78%
at 1M on a non-MRCR harness, i.e. broadly consistent.)

Anthropic also shipped **context compaction** — automatic summarise-and-replace of older
context at a configurable threshold (same announcement). That is retrieval-shaped machinery
built into the model layer, and it exists precisely because rot is real.

### The economics

Practitioner figures, all **[vendor/practitioner, not peer-reviewed]**, from
[usewire.io, 2026-04-08 upd. 2026-07-20](https://usewire.io/blog/long-context-vs-rag-what-the-data-shows/)
and [tianpan.co, 2026-04-09](https://tianpan.co/blog/2026-04-09-long-context-vs-rag-production-decision-framework):

- RAG ≈ **$0.00008/query**; long-context ≈ **$0.10/query** → claimed ~1,250×.
- Latency: RAG ≈ 1s; 160k-token request ≈ 20s; 890k ≈ >60s; production long-context avg 45s.
- NIAH recall of 99.7% collapses to ~60% on realistic multi-fact retrieval.
- Small-to-big retrieval: **48k well-chosen tokens beat full-context 117k by 13 F1**.

Treat the 1,250× as marketing arithmetic (it compares a cached embedding lookup against an
uncached million-token prompt). The **honest** number for our case:

```
Opus 4.6 pricing (2026):  $5/M in, $25/M out; cache read $0.50/M (0.1×); 5-min cache write 1.25×
                          >200k input → premium tier $10/M in, $37.50/M out
```
([benchlm.ai/anthropic/api-pricing, Sept 2026](https://benchlm.ai/anthropic/api-pricing);
the 0.1× cache-read ratio is now near-universal across OpenAI/Anthropic/Google —
[tokencost.app, 2026](https://tokencost.app/blog/prompt-caching-pricing-2026))

So stuffing today's 163k corpus:
- cold: 163k × $5/M = **$0.82/query**
- warm (cache hit): 163k × $0.50/M = **$0.08/query**, plus ~20–40s TTFT.

Stuffing a 1M-token corpus at the premium tier: **$10/query cold, $1/query warm**, ~60s.
At 3M: impossible.

Retrieving 8 whole documents (~60k tokens) into a 200k window: **~$0.30 cold / $0.03 warm**,
~5–10s, and lands in the regime where Opus scores 93%+ rather than 76%.

**Caveat that matters:** Claude 4.7+ uses a tokenizer that produces ~30% more tokens for the
same text ([flexera, 2026](https://www.flexera.com/blog/ai/prompt-caching-breakdown/)) —
budget accordingly.

### The 2026 architectural consensus

Not "long context *or* RAG" but **route by query shape, then retrieve coarsely and read
richly**. Supporting evidence:

- [arXiv 2501.01880](https://arxiv.org/pdf/2501.01880) (LaRA): long-context wins on
  Wikipedia-style QA, RAG wins on dialogue-based queries; neither dominates. **[pre-2026]**
- [arXiv 2605.10235](https://arxiv.org/pdf/2605.10235) — *Route Before Retrieve*: activating
  latent routing ability in the LLM to choose RAG vs long-context per query.
- [arXiv 2606.20898](https://arxiv.org/pdf/2606.20898) (2026-06-23) — *The Token Tax of
  Epistemic Accuracy*, an explicit RAG-vs-long-context cost/accuracy study on
  document-grounded apps. (PDF text extraction failed; abstract-level only. **Flagged as
  unverified detail.**)
- [arXiv 2601.06551](https://arxiv.org/pdf/2601.06551) — L-RAG: entropy-based lazy loading,
  i.e. load more context only when the model's uncertainty says to.

### Verdict at 1–3M tokens

| corpus | call |
|---|---|
| **<200k (today's repo)** | **Don't build retrieval yet.** Stuff the whole corpus with prompt caching. A 200k context on Opus 4.6 is in the 93%-accuracy regime. Build the eval harness with this as your ceiling baseline — you now have a number to beat. |
| **200k–1M** | Retrieve *documents*, not chunks. A crude filter (BM25 + filename/date/speaker metadata) that keeps ≤200k tokens is enough. Rerank if the eval says the filter is leaking. |
| **1–3M (the stated target)** | Same, but the filter now has to actually work. Hybrid BM25+dense over document- and chapter-level units, top-k reranked, whole units returned. Still no chunk-level pipeline. |
| **>10M** | Now you need real chunk-level IR, and this document's advice starts to look naive. |

**The stuffed-corpus baseline is not a strawman — it is your control condition.** Any
retrieval system you build must be measured against "just paste everything in", and at
163k tokens it will be hard to beat on quality. It will be easy to beat on cost and latency.

---

## 2. chunking

```
         what actually moved the needle, 2024 → 2026
  chunk SIZE + BOUNDARY TYPE  ████████████████████   9–15 pp end-to-end swings
  hybrid dense + sparse       ████████████           large, orthogonal to chunking
  reranking                   ███████████            large, orthogonal to chunking
  contextual retrieval        ██████                 real, 5–15% — not 49% — on repro
  late chunking               ███                    +1.8–3.6% rel, and can go negative
  semantic chunking           ▌                      ~0, often negative, 14× the cost
  chunk OVERLAP               ▌                      ~0 on QA; +20% MRR on tables/finance
```

**The default that survives 2026 scrutiny: recursive / structure-aware splitting at
256–512 tokens, 0–15% overlap, plus hybrid retrieval and a reranker.** Everything fancier
is corpus-specific and has to be measured on your own data.

### Fixed / recursive is still the default, and the 2026 evidence made it *stronger*

| study | date | result |
|---|---|---|
| [Chroma, *Evaluating Chunking Strategies*](https://research.trychroma.com/evaluating-chunking) | 2024-07-03 **[pre-2026]** | RecursiveCharacter 400/0 → recall 89.5, precision 3.6; 200/0 → recall 88.1, precision 7.0. Best-vs-worst spread **9% recall** |
| [Vecta/FloTorch, 7 strategies](https://www.runvecta.com/blog/we-benchmarked-7-chunking-strategies-most-advice-was-wrong) | 2026-02 | 50 papers / 905,746 tokens. **Recursive-512 = 69% e2e accuracy** (page F1 0.92); fixed-512 = 67%; fixed-1024 = 61%. **15 pp best-to-worst spread** |
| [arXiv 2602.16974](https://arxiv.org/abs/2602.16974) *Beyond Chunk-Then-Embed*, SIGIR '26 | 2026-02-19 | nDCG@10 across 4 embedders: paragraph 0.4303–0.4997 > fixed-size 0.4136–0.4863 > LumberChunker 0.4270–0.4830 >> **proposition 0.3484–0.3937 (15–27% worse)** |
| [arXiv 2603.06976](https://arxiv.org/html/2603.06976) | 2026-03-07 | 36 strategies × 5 embedders × 6 domains: Paragraph-Group nDCG@5 **0.459** vs naive fixed-character **0.244**. *"Chunking strategy selection dominates model architecture as a performance driver"* (\|r\|<0.2 between embedding dim and score) |

That last finding is the headline of 2026 chunking research: **your chunker matters more
than your embedding model.**

**Overlap became contested in 2026** — this is the real change since 2024.
[arXiv 2601.14123](https://arxiv.org/abs/2601.14123) (2026-01-20, NQ + SPLADE +
Ministral-8B) found (a) **overlap provides no measurable benefit** and only raises indexing
cost, (b) **sentence chunking is the most cost-effective**, matching semantic chunking up to
~5k tokens, (c) a **"context cliff" at ~2,500 tokens**, and (d) optimal context is
metric-dependent — semantic-similarity scores peak at *small* contexts, exact-match at
*large* ones. Directly countered on domain grounds by
[arXiv 2604.12047](https://arxiv.org/pdf/2604.12047) (2026-04, financial QA), which found
**25% overlap best**, lifting MRR **0.529 → 0.658 on FinanceBench** and **0.735 → 0.833 on
TableQuest**. Reconciliation: overlap pays exactly where boundaries are semantically
load-bearing (tables, clauses, numeric reference material) and nowhere else.

> ⚠️ **The 2,500-token "context cliff" is in direct tension with §1's advice to return whole
> documents into a 200k window.** Note what produced it: Ministral-8B on Natural Questions.
> That is a small model on short factoid QA. Opus 4.6's 93% at 256k says frontier models do
> not have that cliff. **Read 2601.14123 as "small generators choke on long context", not
> "long context is bad".** If you ever swap the generator for a cheap small model, the cliff
> comes back and your architecture has to change with it.

Provider defaults have **not** converged — a 3.4× spread. Normalise before comparing anything:

| provider | chunk | overlap | note |
|---|---|---|---|
| OpenAI `file_search` (`auto`) | 800 tok | **400 (50%)** | Chroma called this config *"a particularly poor recall-efficiency tradeoff"* — recall 85.4, **precision 1.5** |
| Amazon Bedrock KB | ~300 tok | 20% | FIXED_SIZE / HIERARCHICAL / SEMANTIC / NONE. **Immutable after data-source connect.** Managed KB GA 2026-06-17 |
| Vertex AI RAG Engine | 1024 tok | 256 (25%) | set per *import*, so A/B-able by re-importing |

**Gotcha that still bites:** LangChain's `chunk_size` counts **characters**, not tokens.
`chunk_size=512` ≈ 128 tokens. Use `RecursiveCharacterTextSplitter.from_tiktoken_encoder()`.

### Semantic chunking: the negative result held, and hardened

- [arXiv 2410.13070](https://arxiv.org/abs/2410.13070), *Is Semantic Chunking Worth the
  Computational Cost?* (Findings of NAACL 2025) — fixed-size ≥ semantic across document
  retrieval, evidence retrieval and answer generation. **[pre-2026]**
- Vecta, 2026-02: semantic **54% e2e vs 69% recursive**. The diagnosis is the useful part —
  semantic chunks averaged **43 tokens**. They retrieved cleanly and then *starved the generator*.
- SIGIR '26 (2602.16974): semantic 0.4104–0.4810 nDCG@10, **below paragraph and fixed-size
  on all four embedders**.
- Cost: ~**0.33 MB/s vs 4.82 MB/s** for token chunking (~14×); one measurement of
  topic-based chunking at **133.79s vs 6.01s** paragraph-based on the same corpus.

**The widely-quoted "semantic chunking gets 91.9% recall" is a misattribution.** In Chroma's
table that row is `LLMSemanticChunker` (GPT-4o doing boundary detection), recall 91.9 ± 26.5.
The off-the-shelf `KamradtSemanticChunker` — what people actually mean by "semantic
chunking" — scored **83.6, the worst row in the table**.

Nuance kept: [SemRAG](https://arxiv.org/pdf/2507.21110) found semantic chunking higher on
*answer relevancy* but fixed-size higher on *answer correctness*; 2603.06976 found semantic
methods had **higher variance and occasional zero-hit retrievals** despite decent means —
structure-preserving methods had "fewer catastrophic failures on difficult queries".

### Late chunking (Jina, 2024): small, real, conditional, sometimes negative

Vendor ([arXiv 2409.04701](https://arxiv.org/abs/2409.04701), v3 2025-07-07) **[vendor]**:
**+3.63% relative / +1.9% absolute** nDCG@10 over 3 models × 4 datasets; NFCorpus
23.46 → 29.98; SciFact 64.20 → 66.10; Jina's blog claims TRECCOVID 64.7 → 77.2 with
jina-embeddings-v3 (2024-10-03). Gains scale with document length, and late chunking on
*fixed-token* boundaries beats naive chunking on *semantic* boundaries — i.e. it makes
boundary choice matter less.

Independent, much weaker and mixed:
- [arXiv 2504.19754](https://arxiv.org/abs/2504.19754) (Merola & Singh, 2025-04-28; published
  KEIR@ECIR, Springer LNCS 16086, 2026): NFCorpus/Jina-v3 **late 0.309 vs contextual 0.317 vs
  traditional baseline 0.312** nDCG@5. On **MSMarco with Stella-V5, early 0.630 vs late 0.503
  — late chunking 20% worse.** Verdict: *"higher efficiency but tends to sacrifice relevance
  and completeness."*
- SIGIR '26 (2602.16974) splits it cleanly by task: **in-corpus retrieval, contextualized
  (late) chunking helps** (structure-based 0% to +7.20%; proposition +22.87% to +27.11%);
  **in-document retrieval, it degrades on every method, −3.83% to −53.17%.**

Support: late chunking is an **embedding-pipeline concern, not a database feature**.
Weaviate has the substantive writeup ([Late Chunking](https://weaviate.io/blog/late-chunking),
2024); Qdrant 1.18, Weaviate 1.38 and Milvus 2.6 ship nothing late-chunking-specific.
Chonkie has a `LateChunker`. **Hard blocker: OpenAI's embedding API does not expose
token-level vectors, so late chunking is impossible on it.**
⚠️ Every late-chunking number traces to 2024/2025; **no 2026 Jina re-benchmark found**.

### Anthropic Contextual Retrieval: the 49/67% never reproduced

Original ([anthropic.com, 2024-09-19](https://www.anthropic.com/news/contextual-retrieval))
**[pre-2026] [vendor]**, top-20 retrieval failure rate:

| config | failure rate | reduction |
|---|---|---|
| baseline (embeddings + BM25) | 5.7% | — |
| + contextual embeddings | 3.7% | −35% |
| + contextual BM25 | 2.9% | **−49%** |
| + reranking (150 → 20) | 1.9% | **−67%** |

800-token chunks, 8k-token docs, 50–100 token generated context, **$1.02 per million
document tokens** with prompt caching. Anthropic's own guidance in that same post: **under
200k tokens (~500 pages), skip RAG and put the corpus in the prompt.**

What reproduction found:
- Merola & Singh reproduced it **from Anthropic's own cookbook notebook**: NFCorpus/Jina-v3,
  contextualized + rank fusion + rerank **0.317 vs 0.312 baseline — ~1.6% relative**. Their
  decomposition matters more than the number: **most of Anthropic's headline gain is
  attributable to rank fusion + reranking, not to contextualization.**
- Published AWS and other engineering reproductions land at **5–15% retrieval-precision
  gain**, not 49%.
- A 2026 chunking paper: contextual beats semantic by **3–4% for single-document queries at
  2–3× indexing cost**, ~4–6s per document.
- The InSeNT critique: Anthropic-style contextualization indexes at **1890.94 ms/doc vs
  15.26 ms/doc** for ModernBERT+InSeNT (~120×).

**The cost argument for contextualization is now won; the latency argument is not.**
Sept-2026 list prices for the contextualizer: Gemini 2.5 Flash-Lite $0.10/M in, Gemini 3.1
Flash-Lite $0.25/M, Claude Haiku 4.5 $1.00/M. Anthropic cache reads $0.30/M vs $3.00/M
fresh. **Correction worth internalising:** the "90% savings" applies to the *cached portion*
only — with 70–80% of input cacheable, real bills drop **60–70%**
([theculprit.ai](https://theculprit.ai/blog/anthropic-prompt-caching-90-percent)).

### Document-level retrieval with no chunking

Still not viable as a general default — a single vector over thousands of words
over-compresses (confirmed in 2602.16974 and 2606.18781). Where 2026 actually moved is
**long-context encoding with post-hoc aggregation**, not "no chunking":
[arXiv 2606.18781](https://arxiv.org/pdf/2606.18781) *Lost in a Single Vector: chunk evidence
aggregation*; [arXiv 2606.23642](https://arxiv.org/pdf/2606.23642) *Multi-Prefix Embedding* —
insert EOS tokens between chunks, extract prefix embeddings in one causal forward pass,
MaxSim match; matches or beats both single-vector and independent-chunk baselines on
MLDR-en, BrowseComp-Plus and LongEmbed (authors caveat the comparison vs Late Chunking is
not fully controlled).

**The one legitimate no-chunking case is still Anthropic's own: corpus < ~200k tokens → put
it in the prompt.** That is our case today.

### Hierarchical / parent-document / RAPTOR — and why benchmarks hide it

The most important 2026 result here is **HiChunk**
([arXiv 2509.11552](https://arxiv.org/abs/2509.11552), ACL 2026 Long pp. 29738–29753, Tencent,
[code](https://github.com/TencentCloudADP/hichunk)). Its core claim:

> **Existing RAG benchmarks cannot differentiate chunking quality, because their evidence is
> sparse.** On GutenQA and OHRBench, *all* chunking methods produced near-identical evidence
> recall and response quality.

They built **HiCBench** with evidence-*dense* QA (T₀ sparse / T₁ single-chunk dense / T₂
multi-chunk dense). Results: chunking F1_Lall out-of-domain **HiChunk 0.5450 vs LLM-chunking
0.4858**; end-to-end on HiCBench T₁ with Qwen3-32B, **HiChunk-200 + Auto-Merge → evidence
recall 81.03 / fact-coverage 68.12** vs fixed-256 **74.06 / 63.20**. Depth L3 optimal.
*Auto-Merge* is adaptive parent-document retrieval: merge fine child nodes into coarse
parents at query time based on what was retrieved.

**Practical read: hierarchical / parent-document retrieval pays off specifically when
evidence is dense and spans multiple chunks.** If your eval set is single-fact
needle-in-a-haystack, every strategy ties and you will wrongly conclude flat chunking is fine.

⚠️ **No 2026 head-to-head of RAPTOR vs HiChunk or vs flat baselines exists.** RAPTOR's
numbers remain ICLR 2024. **[pre-2026]**

### What's genuinely new in 2026

| work | date | contribution |
|---|---|---|
| [2601.14123](https://arxiv.org/abs/2601.14123) | 2026-01-20 | overlap ≈ useless; context cliff @2.5k; sentence chunking best cost/quality |
| [2601.05265](https://arxiv.org/pdf/2601.05265) | 2026-01 | cross-document topic-aligned chunking |
| [2602.16974](https://arxiv.org/abs/2602.16974) SIGIR '26 | 2026-02-19 | unified taxonomy (segmentation × embedding-timing); task-dependence; throughput paragraph 1854 docs/s vs LumberChunker 1.11 docs/s (**167×**) |
| [2603.06976](https://arxiv.org/html/2603.06976) | 2026-03-07 | chunking > embedding-model choice |
| [2603.25333](https://arxiv.org/abs/2603.25333) LREC 2026 | 2026-03-26 | **adaptive per-document chunker selection**; answer correctness 62–64% → **72%**; questions answered 49 → **65 (+30%)** |
| [2604.10167](https://arxiv.org/pdf/2604.10167) | 2026-04 | visual late chunking |
| [2604.12047](https://arxiv.org/pdf/2604.12047) | 2026-04 | 25% overlap wins in tabular/financial (counters 2601.14123) |
| [2604.24334](https://arxiv.org/pdf/2604.24334) | 2026-04 | post-retrieval chunk filtering / redundancy removal |
| [2606.00881](https://arxiv.org/abs/2606.00881) | 2026-05-30 | broadest effectiveness-vs-cost sweep; conclusion **no method dominates** (⚠️ result tables did not extract) |
| [2606.16661](https://arxiv.org/pdf/2606.16661) SCAR | 2026-06 | semantic-continuity-aware context expansion at *query* time |
| [ACL 2026 short 64](https://aclanthology.org/2026.acl-short.64/) | 2026 | late code chunking for repo-level completion |

Library defaults have **not** changed: LangChain `RecursiveCharacterTextSplitter` and
LlamaIndex `SentenceSplitter` are still the defaults, no 2026 announcement from either.
**Chonkie** (YC X25) is the throughput play — token chunking 4.74 MB/s vs LlamaIndex 1.71
(4.05×) and LangChain 1.7 (2.09×); semantic 13m59s vs LangChain ~1h01m; ~15MB install vs
80–170MB. But on *quality*, a third-party test found **LlamaIndex SentenceSplitter
recall@5 0.86 > LangChain recursive 0.79**, and concluded chunk size mattered more than the
library.

### Transcripts specifically — the section that actually decides our design

This is thin in the literature but what exists is unusually decisive.

**Tao An, *Fidelity Before Structure: Verbatim Chunks Beat Lossy Artifact Extraction in
Long-Conversation LLM Memory*** — [arXiv 2601.00821](https://arxiv.org/abs/2601.00821)
(submitted 2025-12-23, latest version **2026-07-22**). One fixed retrieval → rerank →
reasoning pipeline, ablating *only* the stored representation:

| stored representation | LoCoMo | LongMemEval-S |
|---|---:|---:|
| **verbatim conversation chunks** | **43.9%** | **67.4%** |
| LLM-extracted typed artifacts (DECISION / TODO / KEYFACT) | 28.0% | 45.4% |
| Δ | **+15.9 pp** | **+22.0 pp** |

Their verbatim config: **512-character sliding window, 100-character overlap, with explicit
turn markers and session timestamps left in the chunk text**, no LLM at write time.

> **Adding artifacts *alongside* chunks preserves accuracy. *Substituting* artifacts for
> chunks destroys it.**

That is the single most directly applicable finding in this document, and it is a **direct
constraint on §5's summary-index proposal**: the per-document summary index is a *routing
layer that sits beside* the verbatim transcripts. The moment it becomes the thing you
retrieve *instead of* the transcript, you lose 16–22 points.

**Turn-boundary chunking** is engineering consensus with no published retrieval benchmark:
- The transcript-specific failure mode with no prose analogue: **a boundary inside a turn
  orphans text from its speaker.** Misattributing a commitment is worse than not retrieving it.
- Keep the **speaker label inside the chunk text**, not only in metadata — embeddings cannot
  see metadata.
- Keep **original media timestamps** in metadata, for citation and deep-linking.
- Long turns: split at sentence boundaries but **re-prepend the speaker label** to each fragment.
- Overlap should be **whole turns**, not characters.

**Tooling gap is real.** Chonkie ships 11 chunkers, LangChain 11 splitters, LlamaIndex 7 node
parsers — **none of the 29 models a conversation.**
[Chonkie issue #658](https://github.com/feyninc/chonkie/issues/658) (opened 2026-08-28, still
open) proposes one and cites 90 public repos hand-rolling `def chunk_by_speaker`. The one
packaged option is **[`turnchunk` 0.4.1](https://pypi.org/project/turnchunk/0.4.1/)** (PyPI,
**2026-09-02**, MIT, zero deps, Python + TypeScript): parses 11 formats with content-based
auto-detection (WebVTT from Teams/Zoom/YouTube, SubRip, Whisper/WhisperX, Deepgram,
AssemblyAI, Rev.ai, Speechmatics, AWS Transcribe, Google STT, Azure Speech). Defaults
`target=2000` chars of rendered text *including labels*, `overlap=200`, rounded to whole
turns. 1.7M-char transcript → 450ms parse, 16ms chunk. Ships a Chonkie pipeline adapter.

⚠️ **Weakest-evidenced section in this document. There is no peer-reviewed study that varies
chunk size/overlap on speaker-diarized transcripts and reports retrieval quality.**
`turnchunk` publishes *speed* benchmarks, not recall. The multimodal chunking survey
[arXiv 2512.00185](https://arxiv.org/abs/2512.00185) covers audio/video segmentation but
gives no sizes or numbers. Oracle's transcript-chunking post is qualitative and was
Cloudflare-blocked to automated fetch (cited second-hand).

---

## 3. retrieval: dense, sparse, hybrid, late interaction, embeddings

### Dense has a proven ceiling that BM25 does not

The most consequential retrieval result of the cycle:
**[arXiv 2508.21038](https://arxiv.org/abs/2508.21038), *On the Theoretical Limitations of
Embedding-Based Retrieval* — accepted ICLR '26, v2 revised 2026-03-12.** Single-vector
embeddings **provably cannot represent all top-k document subsets** above a
dimension-dependent bound. On their LIMIT dataset BM25 crushes every dense model. This is now
the standard citation for "dense retrieval has a ceiling keyword search doesn't", and it is
the strongest available formal argument for keeping a lexical leg **permanently**, not as a
transitional hack.

Empirically corroborated: [arXiv 2604.01733](https://arxiv.org/abs/2604.01733) (2026-04-02,
23,088 queries / 7,318 docs) found **BM25 beats dense retrieval on financial documents**, and
that query expansion and adaptive retrieval give limited benefit for precise numerical queries.

### Fusion: RRF is still the default, but the defaults diverged — check your vendor

| system | default fusion | default k |
|---|---|---|
| Elasticsearch | RRF (GA) | `rank_constant = 60` |
| Milvus | RRFRanker / WeightedRanker | **60** |
| **Qdrant** | RRF (unweighted) | **2** ⚠️ configurable since v1.16.0; per-retriever weights since v1.17.0. Also ships **DBSF** (distribution-based score fusion, ±3σ normalisation) since v1.11.0 |
| **Weaviate** | **relativeScoreFusion — abandoned RRF as default in v1.24** | n/a |
| Turbopuffer | weighted RRF *plus* operator fusion (`Sum`, `Max`) over BM25 + dense + sparse in one ranking expression | — |
| Azure AI Search | RRF | 60 |

> **"RRF with k=60" is no longer a safe cross-vendor assumption.** Qdrant defaults to k=2.

k=60 was never theory — Cormack/Clarke/Büttcher (SIGIR 2009) fixed it "during a pilot
investigation and not altered during subsequent validation"; MAP is flat across k ∈ [10,100].
2026 practitioner guidance: k ∈ [40,80] precision-first, k ∈ [60,100] when feeding a reranker.

**Does anything beat RRF? Yes, if you have ~50 labelled queries.** Convex combination (CC)
of normalised scores beats RRF in-domain *and* out-of-domain and is sample-efficient —
[arXiv 2210.11934](https://arxiv.org/abs/2210.11934) (Bruch et al., ACM TOIS) **[pre-2026, and
still the canonical citation]**. 2026 corroboration from 2604.01733's fusion ablation:

| fusion | Recall@5 |
|---|---:|
| **CC, α = 0.5** | **0.726** |
| RRF k=10 | 0.716 |
| RRF k=60 | 0.695 |

Their full pipeline (hybrid RRF + cross-encoder) reached **Recall@5 0.816 / MRR@3 0.605** —
note the reranker is worth far more than the fusion choice.

Learned/orchestrated fusion is the 2026 research direction, not a shipped default:
*ProRetrieval* ([2608.27017](https://arxiv.org/abs/2608.27017), 2026-08-27), *ORDER:
task-conditioned routing for RAG* ([2609.17012](https://arxiv.org/abs/2609.17012), 2026-09-15).

**Hybrid uplift magnitudes — the honest number is single-digit percent, corpus-dependent.**
WANDS e-commerce: BM25 0.6983, pure kNN 0.6953, plain RRF 0.7068 (**+1.2% only**), *tuned*
hybrid 0.7497 nDCG (**+7.4%**) — the lift came from tuning, not from fusing **[pre-2026,
2025-03]**. Domain spread cited in 2026 reference material: **+5% on BEIR SciFact to +24% on
BRIGHT Biology** — the big numbers come from high vocabulary-mismatch domains. *Spoken
transcripts are a high vocabulary-mismatch domain.*

### Late interaction (ColBERT / ColPali)

**Text side, 2026 models:**

| model | params | date | headline |
|---|---|---|---|
| **lightonai/ColBERT-Zero** | <150M (ModernBERT-base) | 2026-02-19 | **55.43 BEIR nDCG@10**, SOTA <150M. [arXiv 2602.16609](https://arxiv.org/abs/2602.16609), Apache-2.0 |
| LiquidAI/LFM2.5-ColBERT-350M | 353M, 128d/token | 2026-05-20 | NanoBEIR-multilingual nDCG@10 **0.605** over 11 langs |
| answerai-colbert-small-v1 | 33M | 2024-08 **[pre-2026]** | still 767k downloads/mo |
| jina-colbert-v2 | — | 2024-08 **[pre-2026]** | 59k/mo |
| colbert-ir/colbertv2.0 | 110M | 2023 **[pre-2026]** | **still #1 by downloads (2.45M/mo)** — the actual production default |

ColBERT-Zero's controlled ablation is the number that matters — same data, same base, same
pipeline, only dense-vs-multi-vector varies:

```
ModernBERT-embed-supervised (dense)               52.89
gte-modernbert-base (dense, better data)          55.33   ← 2.4-pt data-quality gap
ModernColBERT-embed-base-kd-only (industry std)   54.09
ModernColBERT-embed-base (sup+KD)                 55.12   ← ~40 GH200-h
ColBERT-Zero (full 3-phase)                       55.43   ← ~408 GH200-h
```

**Late interaction buys ~+2.5 nDCG@10 over dense on identical data (52.89 → 55.43) — which
is less than the gap between good and bad *training data* on the dense side (2.4 pts).**
That is the whole cost-benefit case in one table.

**Visual documents are where late interaction is now unambiguously SOTA.** ViDoRe:

| model | date | v1 | v2 | v3 (PUB AVG) |
|---|---|---:|---:|---:|
| **OpenSearch-AI/Ops-Colqwen3-4B** (2560d) | 2026-01-23 | **91.4** | **68.7** | 61.27 |
| Ops-Colqwen3-4B **@128 dims** | — | 90.9 | 66.9 | 60.23 |
| TomoroAI/tomoro-colqwen3-embed-8b | 2025-11 | 90.76 | 67.72 | 61.13 |
| nvidia/llama-nemoretriever-colembed-3b-v1 | 2025 | 91.0 | 63.3 | 57.07 |
| jina-embeddings-v4 (128d) | 2025 | 90.4 | 58.2 | 57.54 |

The field got its own venue: **LIR 2026, first workshop on Late Interaction and Multi-Vector
Retrieval @ ECIR 2026** ([arXiv 2511.00444](https://arxiv.org/abs/2511.00444)) — framing:
strong generalisation **particularly out-of-domain**, but "significant challenges of
efficiency, usability, and integration."

**The cost is storage, not latency.** LFM2.5 query-encode p50: ColBERT 8.1ms vs dense 7.3ms
on M4 Max CPU; 1.3ms vs 1.5ms on H100 ([liquid.ai, 2026-06-18](https://www.liquid.ai/blog/lfm2-5-retrievers)).
But 128 dims/token vs one 1024-dim vector is **~64× more floats per 512-token doc** before
compression. Mitigations: MUVERA turns MaxSim into single-vector MIPS (**+10% recall, −90%
latency vs PLAID, 32× memory reduction under PQ** —
[Google Research, 2025-06-25](https://research.google/blog/muvera-making-multi-vector-retrieval-as-fast-as-single-vector-search/)
**[pre-2026]**); Ops-Colqwen3 at **128 dims loses only 0.83 nDCG@5** (84.87 → 84.04) for a
20× storage cut.

**Native multi-vector support today:**

| DB | status |
|---|---|
| **Qdrant** | ✅ `max_sim` since v1.10.0; Turbo4 4-bit quantization in v1.19.0 (~⅛ storage); documented `prefetch` → ColBERT rescore pattern |
| **LanceDB** | ✅ native MaxSim, **cosine only** |
| **Vespa** | ✅ via mixed tensors — you write MaxSim yourself, not turnkey |
| **Elasticsearch** | ⚠️ `rank_vectors` **still Preview since 9.0**; **no ANN index — second-pass rescoring only** |
| **Turbopuffer** | ⚠️ private beta |
| **Milvus** | ❌ not in v3.0.x reranking docs |

### Learned sparse (SPLADE and successors): alive, demoted

Current best inference-free sparse encoder is still
`opensearch-neural-sparse-encoding-doc-v3-gte`, **2025-06-18 [pre-2026]** — no 2026 successor.
13-dataset BEIR subset:

| model | inference-free | params | nDCG@10 | FLOPS |
|---|---|---|---:|---:|
| neural-sparse-v2-distill | ✗ | 67M | 0.528 | 8.3 |
| doc-v3-distill | ✔ | 67M | 0.517 | 1.8 |
| **doc-v3-gte** | ✔ | 133M | **0.546** | **1.7** |

The doc-only trick (query side = tokenizer + weight lookup, zero neural inference) gets
0.546 nDCG@10 at 1.7 FLOPS on a plain Lucene inverted index. **SPLADE-v3 remains the last
SPLADE release (2024)** — no v4. Research is active but diagnostic rather than advancing:
*Rescaling MLM-Head for Neural Sparse Retrieval* ([2606.18811](https://arxiv.org/abs/2606.18811))
and *Why Advanced Encoders Lag on Sparse Retrieval?* ([2607.00004](https://arxiv.org/abs/2607.00004))
both explain **why stronger pretrained encoders fail to improve SPLADE** (MLM-head scale
mismatch, vocabulary gaps).

**Verdict: learned sparse is a *systems* choice, not a quality choice** — reuse your inverted
index, no ANN, near-zero query cost, interpretable term weights. 0.546 is well below a 2026
dense model. BGE-M3 (the "one model does dense+sparse+ColBERT" convenience pick) is now weak:
**RTEB mean 0.5893**, near the bottom of the current table.

### Current best embedding models — and the leaderboard is compromised

**The honest framing: MTEB overall is no longer where retrieval people look; RTEB is — and
RTEB had a governance incident in January 2026.**

RTEB (nDCG@10 over 29 retrieval datasets, legal/finance/medical/code, 20+ languages) exists
*because of* MTEB overfitting: "many models train on the test set, some intentionally, leading
to inflated scores." Hybrid design — open datasets for reproducibility, private datasets
scored by maintainers. **MTEB maintainers removed the private RTEB column on 2026-01-14**
because RTEB was co-developed with Voyage, giving Voyage access to the private evaluation
data — *"an undeniable structural advantage… the uneven playing field fundamentally
undermines trust in MTEB leaderboards."* Issue still open
([embeddings-benchmark/mteb#3934](https://github.com/embeddings-benchmark/mteb/issues/3934)).

RTEB overall average, aggregator snapshot
([benchmarklist.com/benchmarks/rteb](https://benchmarklist.com/benchmarks/rteb/), **updated
2026-05-06**):

| rank | model | RTEB avg |
|---:|---|---:|
| 1 | voyage-code-3 | 81.02 |
| 2 | **voyage-4-large** | 79.70 |
| 3 | **Octen-Embedding-8B** (open, Apache-2.0) | 79.38 |
| 4 | voyage-3-large | 78.26 |
| 6 | Octen-Embedding-4B | 76.38 |
| 7 | gemini-embedding-001 | 75.85 |
| 10 | llama-embed-nemotron-8b | 73.37 |
| 12 | Qwen3-Embedding-8B | 72.43 |
| 13 | gemini-embedding-2-preview | 68.4 |
| 14 | **text-embedding-3-large** | **65.45** |
| 15 | KaLM-Gemma3-12B | 64.39 |
| 19 | bge-m3 | 56.39 |
| 21 | all-MiniLM-L6-v2 | 39.86 |

**The MTEB-vs-real-retrieval gap is measurable and large.** KaLM-Gemma3-12B is **#1 on MMTEB
(72.32) and #15 on RTEB (64.39)**. One reported legal-contract case had the MTEB top-3 ranking
5th/7th/2nd in-domain, while the in-domain winner (bge-large-en-v1.5) sat 11th on MTEB.
Diagnostic tell for a leaderboard-tuned model: **a 15+ nDCG drop moving to a private domain**
([zeroentropy.dev](https://zeroentropy.dev/concepts/mteb/)). MTEB v1 and v2 scores are **not
comparable**, and v2 retrieval excludes MS MARCO and NQ.

New harder 2026 benchmarks: **HTEB** ([2605.28190](https://arxiv.org/pdf/2605.28190));
**HAKARI-Bench** ([2606.22778](https://arxiv.org/abs/2606.22778), 2026-06-22) — 35 benchmarks,
43 languages, five retrieval families (BM25 / dense / sparse / late interaction / rerankers),
55 models, **Spearman >0.97 with MTEB/BEIR at a fraction of the cost**, MIT-licensed.

**Model-by-model, September 2026:**

| model | date | dims | max ctx | price | note |
|---|---|---:|---:|---|---|
| **OpenAI text-embedding-3-large** | **2024-01** | 3072 | 8191 | $0.13/M | **No successor. The single biggest stale default in the space** — ~14% behind voyage-4-large on RTEB. Verified against the live pricing page 2026-09-18 |
| OpenAI text-embedding-3-small | 2024-01 | 1536 | 8191 | $0.02/M | cheap, RTEB 0.5874 |
| **Voyage voyage-4-large / -4 / -4-lite / -4-nano** | **2026-01-15** | 2048/1024/512/256 MRL | 32K | — | **First production embedding model with MoE.** `-nano` is **open weights, Apache-2.0**. All four share **one embedding space** — embed docs with `-large`, queries with `-nano`, no re-index. float32/int8/uint8/binary. First 200M tokens free. Vendor deltas **[vendor]**: +3.87% vs Cohere v4, +8.20% vs gemini-embedding-001, +14.05% vs OpenAI v3-large |
| **Voyage voyage-context-4** | **2026-06-29** | — | 32K (auto-splits beyond) | **$0.12/M** | **contextualized chunk embeddings**; **+7.11% on LongEmbed vs single vectors**; +3.02% single-embedding mode |
| Voyage voyage-code-4 | 2026-08-13 | — | — | $0.12/M | +28.25% vs Cohere v4 on agentic code retrieval **[vendor]** |
| **Google gemini-embedding-2** | preview 2026-03-10, **GA ~2026-04** | 3072 default, MRL | 8192 | $0.20/M | first fully multimodal Gemini embedding (text/image/video/audio/PDF), 100+ langs. **Embedding space incompatible with `-001` — full re-index required.** Note it is *still behind* `-001` on the May-2026 RTEB snapshot (68.4 vs 75.85) |
| Google gemini-embedding-001 | 2025 | 3072 | 2–20K (sources disagree) | $0.15/M | RTEB 75.85, MMTEB 68.37 |
| **Cohere embed-v4.0** | **2025-04** **[pre-2026]** | 256/512/1024/**1536** MRL | **128K** (longest of any major embedding model) | $0.12/M text, $0.47/M image | no v5; live model list confirms no 2026 embed release. Reranking *did* advance (`rerank-v4.0-pro`/`-fast`, 32K) |
| **Jina jina-embeddings-v5-text-small / -nano** | **2026-02** | MRL | **32K** | open | small 677M: **MTEB(Eng,v2) 71.7, MMTEB 67.7 — best multilingual under 1B**; RTEB 66.84. Their Generalized Orthogonal Regularization makes **binary quantization nearly lossless** — the practically interesting bit |
| **Microsoft harrier-oss-v1** (270m / 0.6b / 27b) | **2026-03** | up to 5376 | **32,768 all sizes** | **MIT** | Bing team. **MTEB v2: 66.5 / 69.0 / 74.3** **[vendor, ❓ unconfirmed by any third-party board]**. Decoder-only, last-token pooling, ~94–100 langs. **Instruction-tuned — queries need a one-sentence task instruction, documents don't** |
| Qwen3-Embedding-0.6B/4B/8B | 2025-06 **[pre-2026]** | 4096 (8B) | 32K | Apache-2.0 | 8B = MMTEB 70.58, RTEB 72.43. **No Qwen3.5-Embedding.** Successor went multimodal: `Qwen3-VL-Embedding` 2B/8B (2026-01, MMEB-V2 **77.8**) but *worse* on text than the text-only model of the same size |
| Octen-Embedding-8B / 4B | 2025-12 – 2026-01 | 4096 | 32K | Apache-2.0 | LoRA on Qwen3-Embedding-8B; RTEB-topping open model, 874k downloads |
| LiquidAI LFM2.5-Embedding-350M | 2026-06 | 1024 | — | open | 11 langs, 7.3ms CPU p50 |

**Declining / legacy, now 15–40 RTEB points behind the frontier:** BGE (bge-m3 0.5893,
bge-large-en-v1.5 0.4836), E5 (e5-mistral-7b 0.6270), NV-Embed-v2 (0.6203, **CC-BY-NC**),
GTE (gte-multilingual-base 0.5921), all-MiniLM-L6-v2 (0.3986).

### Long-context embeddings: they work, and you still shouldn't use them that way

The 2026 consensus is **not** "long-context embeddings degrade" — it's **"a single vector
over a long document is the wrong shape, regardless of the window."**

- Theory: LIMIT/ICLR'26 — more content per vector makes the representational bound *worse*.
- Direct measurement: `voyage-context-4` reports **+7.11% on LongEmbed for contextualized
  chunks over single vectors**, i.e. chunking with document context beats one big embedding
  by ~7% *even though the model can take the whole document*.
- Same conclusion from research: [2606.23642](https://arxiv.org/abs/2606.23642) (Multi-Prefix
  Embedding) and [2608.29899](https://arxiv.org/abs/2608.29899) (REIGN).
- AWS's own docs on Cohere's 128K window explicitly warn: *"for RAG, smaller chunks often
  improve retrieval and cost."*

**The winning pattern is contextualized chunk embeddings** (`voyage-context-4`, Jina's late
chunking lineage): chunk normally, but embed each chunk conditioned on the whole document.
Chunk-level precision, document-level context, no single-vector bottleneck. This is
Anthropic's contextual retrieval idea moved from a prompt-engineering trick into the
embedding model — and it costs $0.12/M instead of a contextualization pass per chunk.

### Provider-hosted retrieval: convenient, unmeasured

| provider | product | what it is | price | verdict |
|---|---|---|---|---|
| **Google** | [Gemini File Search](https://ai.google.dev/gemini-api/docs/file-search) | managed import + chunk + index + retrieve. **Semantic only, no BM25.** Configurable `max_tokens_per_chunk` / `max_overlap_tokens` | **storage free, query embeddings free**; indexing at standard embedding rates; retrieved tokens billed as context | best-value hosted option. Pure-dense → inherits the LIMIT ceiling. Can't combine with Search grounding |
| **OpenAI** | [file search / vector stores](https://developers.openai.com/api/docs/guides/tools-file-search) | managed stores, "semantic and keyword search", metadata filters | **$0.10/GB/day + $2.50/1k tool calls** | **no stated embedding model, no reranker disclosure, no published benchmarks.** You are paying for opacity |
| **Anthropic** | [search result content blocks](https://platform.claude.com/docs/en/build-with-claude/search-results) | **not hosted retrieval** — a `search_result` content-block type so *your* retriever's output gets first-class citations with source + title | n/a | **Anthropic ships no managed vector store.** Bring your own retrieval; they solve attribution. This is exactly the primitive our timestamped-citation requirement needs |
| MongoDB/Voyage | Atlas Embedding & Reranking API | voyage-4 family hosted in Atlas | first 200M tokens free | tightest DB+model integration |

**None of the three publishes a retrieval-quality benchmark.** There is no RTEB or BEIR number
for OpenAI file search or Gemini File Search. Treat them as convenience layers, not quality
choices.

### The decision shape

```
                    ┌─ visual / PDF ──► ColPali line (Ops-Colqwen3-4B @128–320d)
                    │                   ViDoRe v3 ~60–61; 13–20× storage cut vs full-dim
  what are you      ├─ code ──────────► voyage-code-4 ($0.12/M)
  retrieving?  ─────┤
                    ├─ long docs ─────► contextualized chunks (voyage-context-4, +7.1% LongEmbed)
                    │                   NOT one 128K-token vector
                    └─ general text ──► API:   voyage-4-large > gemini-embedding-001 > Cohere v4
                                              >> text-embedding-3-large (Jan 2024, ~14% behind)
                                        open:  Octen-8B / Qwen3-Emb-8B / harrier-oss-v1 (MIT, 32K)
                                        small: jina-v5-small 677M / LFM2.5-350M

  then: BM25 alongside it, ALWAYS  ──► fuse ─┬─ no labels ► RRF (CHECK YOUR VENDOR'S k — 60 vs Qdrant's 2)
        (LIMIT/ICLR'26 says dense              └─ ~50 labels ► convex combination α≈0.5
         cannot reach where BM25 goes)                        (0.726 vs 0.695 Recall@5)

  then: rerank top ~32 ──► Cohere rerank-v4.0-pro / Voyage rerank-3 (both 32K ctx)
        ↑ worth more than any amount of fusion tuning (0.726 → 0.816 Recall@5)
```

### Flagged: unverified in this pass

- ❓ **"Wholembed v3"** (claimed 2026-03-12, "first embedding model to exceed BM25 on LIMIT",
  BrowseComp-Plus 64.82% vs Voyage 61.6%) appears in exactly one source
  ([trilogyai.substack.com](https://trilogyai.substack.com/p/late-interaction-colbert-to-wholembed)),
  with **zero hits on HuggingFace model search and zero on arXiv full-text**. Do not cite.
- **The live MTEB leaderboard could not be read** (Gradio SPA). All MTEB/RTEB rankings above
  come from dated aggregators (2026-05-06, 2026-05-17) or vendor model cards, and the
  aggregators are demonstrably incomplete — neither lists Harrier, released March 2026.
  Verify at [leaderboard.mteb.org](https://leaderboard.mteb.org/) before acting on a ranking.
- **Harrier's 74.3 MTEB v2** is vendor-reported and unconfirmed third-party.
- **All Voyage-vs-competitor deltas are vendor-stated**, and the private RTEB split they were
  partly measured against was pulled from the leaderboard on 2026-01-14 for conflict of interest.
- Not retrieved: a 2026-sourced BM25 BEIR average; HAKARI-Bench per-architecture numbers;
  Weaviate's current multi-vector version; Stella family 2026 status.

---

## 4. reranking

### The current field (Sept 2026)

The most useful head-to-head is Agentset's public leaderboard
([agentset.ai/rerankers](https://agentset.ai/rerankers), last updated **2026-02-15**).
Methodology: BGE-small-en-v1.5 embeddings → FAISS top-50 → reranker; 3 datasets (financial
queries, scientific claims, long-form); nDCG@5/10 and Recall@5/10, plus an **ELO** from
GPT-5 blind-comparing two ranked lists.

| model | ELO | latency (ms) | $/1M | license |
|---|---:|---:|---:|---|
| **Zerank 2** | 1638 | 265 | 0.025 | cc-by-nc-4.0 |
| **Cohere Rerank 4 Pro** | 1629 | 614 | 0.050 | proprietary |
| Zerank 1 | 1573 | 266 | 0.025 | cc-by-nc-4.0 |
| **Voyage Rerank 2.5** | 1544 | 613 | 0.050 | proprietary |
| **Zerank 1 Small** | 1539 | 248 | 0.025 | **Apache 2.0** |
| Voyage Rerank 2.5 Lite | 1520 | 616 | 0.020 | proprietary |
| Cohere Rerank 4 Fast | 1510 | 447 | 0.050 | proprietary |
| Qwen3 Reranker 8B | 1473 | **4687** | 0.050 | Apache 2.0 |
| Contextual AI Rerank v2 Instruct | 1469 | 3333 | 0.050 | cc-by-nc-4.0 |
| Cohere Rerank 3.5 | 1451 | 392 | 0.050 | proprietary |

Note the **ELO and nDCG disagree** in that table (Voyage 2.5 has the best nDCG@10 but only
4th ELO) — which is itself the lesson: leaderboard rank depends on whether you trust a
graded-relevance metric or an LLM's holistic preference. Neither is your corpus.

Licence trap: Jina Reranker v2 base multilingual weights are CC-BY-NC-4.0; commercial use
needs the hosted API. Same for Zerank 1/2 (Small is Apache 2.0). **bge-reranker-v2-m3**
remains the safe Apache-2.0 self-host default.

Vendor claims to discount **[vendor]**: Pinecone claims best avg nDCG@10 on BEIR (6 of 12
datasets, +60% on Fever vs Google Semantic Ranker); ZeroEntropy claims +18% nDCG@10 over
Cohere rerank-3 on financial docs. Every vendor benchmarks favourably; several embedding
models are trained on BEIR tasks (FEVER especially), so BEIR averages are contaminated as a
cross-vendor comparison.

### How much does it actually buy

Published range, top-50 → rerank → top-5/10:

- **+5 to +15 nDCG@10** typical in production, 20+ on lexically hard datasets, <200ms added
  latency ([redis.io, 2026](https://redis.io/blog/top-reranking-models-rag-accuracy/)).
- Practitioner, 50-query labelled set: recall@5 **0.61 → 0.79** with bge-reranker-v2-m3 over
  the same top-50 ([dev.to, 2026](https://dev.to/gabrielanhaia/the-reranker-setup-that-actually-changes-recall5-1ole)).
- Academic: relative recall@5 gains of **+27.6%** on one benchmark, **+4.7%** on another.
- Anthropic's contextual-retrieval ablation: reranking took failure rate 2.9% → **1.9%**
  (a 34% relative cut on top of contextual embeddings + contextual BM25)
  ([anthropic.com, 2024-09-19](https://www.anthropic.com/news/contextual-retrieval)) **[pre-2026]**.
- Inside an agent loop on SEC filings: **+59% MRR@5** from adding a cross-encoder.

### It can go negative

Documented regressions: recall@5 0.83 → 0.71 after a query-distribution shift to short,
typo-heavy queries (removing the reranker restored 0.82); on patent retrieval
bge-reranker-v2-m3 *hurt* BM25 (0.153 → 0.140)
([arXiv 2605.24297](https://arxiv.org/pdf/2605.24297)). Reranking k=100 performed worse than
no reranking at all in one benchmark because the wider pool injected noise.

### The decision rule (this is the part to actually use)

> **Measure recall@50 and recall@5 on a frozen golden set. The gap between them is the
> reranker's entire headroom.** If recall@50 ≈ recall@5, a reranker has nothing to lift. If
> recall@50 is itself poor, the leak is upstream and the reranker will polish a candidate
> set that doesn't contain the answer.

At a few hundred documents you can retrieve top-50 cheaply — but you may also be able to
retrieve top-50 *out of 300*, which means your candidate pool is 1/6 of the corpus and
recall@50 is near 1.0 by construction. **In that regime the reranker is doing the entire
job, and you should think of it as "the retriever" rather than as a refinement.** Which in
turn means an LLM listwise reranker over ~50 documents is the more honest architecture:
one Haiku/Flash-class call that reads 50 titles+summaries and picks 8.

Cost sanity: Cohere rerank pricing ≈ **$2.00 per 1,000 search units** (1 query + up to 100
docs). At 300 documents and a few hundred queries a day this is rounding error. A dedicated
reranker is cheap; the question is only whether it helps.

---

## 5. GraphRAG and its successors

### The two-year arc

| date | event | number |
|---|---|---|
| 2024-04 | Microsoft GraphRAG released | indexing a 5GB legal-case dataset: **~$33,000** |
| 2024-11 | LazyGraphRAG (Microsoft's own follow-up) | claimed **0.1%** of indexing cost |
| 2026 | LazyGraphRAG / LightRAG in practice | indexing cost ≈ **parity with plain vector RAG**; full-GraphRAG answer quality at **>700× lower query cost** ([cruxdigits, 2026-07-24](https://cruxdigits.nl/blog/rag-vs-graphrag-2026/)) |
| 2026-09-16 | **EffiRAG** ([arXiv 2609.18099](https://arxiv.org/abs/2609.18099)) | vs LightRAG-hybrid on UltraDomain (120 open-ended Qs, 4 domains): preferred on **93/120**, total cost **$0.408 vs $0.952** (−57%); at 10–20 docs/domain, **4.2–4.5× cheaper** |

**The cost objection is dead.** That is the single biggest change since 2024 and it is why
"GraphRAG is too expensive" is no longer a valid reason to skip it.

### But the value case did not broaden

The 2026 practitioner verdict, consistently:

- Hybrid BM25+dense with RRF beats either leg by **15–30% recall**; enterprise pipelines
  report recall **0.72 (BM25 alone) → >0.9 (tuned hybrid)** (cruxdigits, 2026-07-24).
  Graphs earn their keep only on genuinely multi-hop, cross-document questions.
- **GraphRAG is worse on time-sensitive queries**: one study measured a **−16.6% accuracy**
  drop vs traditional RAG, because incremental graph update is hard and re-indexing is
  expensive (ibid.).
- The standing advice is **keep the decision reversible**: expose retrieval as a tool
  (MCP-shaped), so swapping a graph backend in later is a retrieval-layer change, not an
  agent redesign.

EffiRAG's thesis is the sharpest formulation available: **graph structure is worth paying
for only when it is used to *locate original source evidence*, not when it is used to
*generate summaries or reasoning chains*.** A graph that produces community summaries you
then answer from is paying twice — once to build, once in fidelity loss. (Caveat: a machine
review of that paper notes its central quality claim rests on one LLM judge and one dataset.)

Related lineage worth knowing: HippoRAG-style Personalized-PageRank indexing reports ~12pp
precision gains on long-horizon QA; RAPTOR (hierarchical recursive summary trees) is the
non-graph alternative for the same "global/thematic query" problem and is far cheaper to build.

### At our scale

A few hundred markdown files, 1–3M tokens, with the corpus being **transcripts of a small
recurring set of people discussing a small recurring set of tactics**.

- **Against a graph:** the entity set is tiny and mostly *already named in the filenames and
  the loom-generated chapter headings*. The corpus changes weekly (new calls), which is
  exactly GraphRAG's documented weak spot (−16.6% on time-sensitive queries). Local factoid
  queries ("what did Eric say about signage in the verification video?") are pure lookup.
- **For a graph — the one honest case:** global/thematic queries. *"What are the recurring
  objections across all coaching calls?"* / *"Which tactics changed between December and
  August?"* Flat vector similarity genuinely cannot answer these, because no single chunk
  contains the answer. This is the query class GraphRAG was built for.
- **But at 1–3M tokens there is a cheaper answer to that query class**: a **map-reduce over
  a corpus that fits in ~3 context windows**, or a precomputed per-document summary index
  (~300 summaries × 200 tokens = 60k tokens — one context window holding a summary of
  *everything*). That is RAPTOR's bottom layer without the tree, and it costs one cheap-model
  pass over the corpus.

**Verdict: no graph. Build the flat per-document summary index instead** — it answers the
global-query class at ~1% of the conceptual complexity, and it doubles as the routing layer
for the retrieval tool (§6). Revisit only if the eval shows synthesis queries failing *and*
the summary index is the thing that's failing.

> **Hard constraint on that summary index, from §2:** it is a *routing layer beside* the
> verbatim transcripts, never a *replacement* for them.
> [arXiv 2601.00821](https://arxiv.org/abs/2601.00821) (2026-07-22) measured exactly this
> substitution on conversation memory: verbatim chunks 43.9% / 67.4% (LoCoMo /
> LongMemEval-S) vs LLM-extracted typed artifacts 28.0% / 45.4% — **−15.9 and −22.0 points**.
> Artifacts *added alongside* chunks preserved accuracy. This is the same failure GraphRAG's
> community summaries commit, which is also why EffiRAG's "use the graph to locate source
> evidence, not to generate summaries" framing and this result are the same finding arriving
> from two directions.

---

## 6. agentic retrieval

### The measured tradeoff

The cleanest controlled study is **"Is Agentic RAG worth it?"**
([arXiv 2601.07711v2](https://arxiv.org/html/2601.07711v2), Ferrazzi, Cvjeticanin,
Piraccini, Giannuzzi; v2 dated **2026-04-20**). Three configurations — Naïve RAG, *Enhanced*
RAG (fixed pipeline: routing + rewriting + reranking), Agentic RAG (LLM orchestrates) — over
NQ (3,452 q), FIQA (648), FEVER (6,666), CQADupStack-English (1,570), with Qwen3
0.6B/4B/8B/32B and GPT-4.1-nano.

| capability | Enhanced (fixed) | Agentic | winner |
|---|---|---|---|
| user-intent routing (F1) | 87.9–96.6 | 64.6–**99.8** (high variance) | agentic *when it works* |
| query rewriting (nDCG@10) | 52.8 | **55.6** (+2.8) | agentic |
| document refinement / rerank (nDCG@10) | **49.5** | 43.9 | **fixed pipeline** |
| input tokens | 1× | **3.3×** | fixed |
| output tokens | 1× | **1.9×** | fixed |
| latency | 1× | **1.5×** | fixed |

Their conclusion, verbatim in spirit: *neither is universally superior; a well-optimized
Enhanced RAG can match or exceed Agentic performance while remaining more efficient.* Use
agentic for **routing and query rewriting** (no manual examples needed); use the fixed
pipeline for **reranking** and cost-sensitive paths.

Also useful from that paper: in the Enhanced pipeline, **45–50% of wall-clock is answer
generation**, a similar share is query rewriting, retrieval is 0–5% and reranking 0–2%.
**LLM calls dominate latency. Optimising your vector index is optimising 5% of the problem.**

Production figures (practitioner, **[not peer-reviewed]**): token cost **3–10×**, latency
from 1–2s to **4–15s p95**, i.e. 2–5× with disproportionately worse tails. A
poisoning-robustness study logged **$0.0004/query vanilla vs $0.0011/query agentic**
([arXiv 2605.05632](https://arxiv.org/pdf/2605.05632)). One practitioner reported
$200/month classic → $1,800 in two weeks agentic, because each query spawned 3–7 LLM calls.

**The dominant production failure mode is over-retrieval**: the agent loops and calls the
retriever 8–12 times for a question two retrieves would answer, burning tokens *and*
producing a worse answer. Documented fixes: a hard step budget (e.g. max 4 retrieve calls
per turn), a faithfulness judge that exits early, and tracking **retrieves-per-correct-answer**.

### The "just let the agent grep" school

This is the live argument and both sides have published.

**Grep side.** Anthropic removed vector search from Claude Code in May 2025 — embedding
pipeline, local vector DB, chunking heuristics — replacing it with grep. Boris Cherny
(Claude Code creator) on Latent Space: early versions used RAG + a local vector DB, but
agentic search "generally works better" and dodges security, privacy, staleness and
reliability problems. Windsurf, Cline, Devin and Sourcegraph Amp are reported to have
followed. The formal argument is **staleness**: an embedding index is a derived copy that
goes stale on commit.

Anthropic's own framing is *just-in-time retrieval*: agents hold lightweight identifiers
(file paths, stored queries, links) and load data at runtime via tools — Claude Code writes
targeted queries and uses `head`/`tail` to inspect large volumes without loading them
([Effective context engineering for AI agents](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)).

**Embeddings side.** Cursor published the counter-data
([cursor.com/blog/semsearch](https://cursor.com/blog/semsearch), **2025-11-06**) **[pre-2026]**:

- offline eval: **+12.5% accuracy** on average (range **+6.5% to +23.5%** by model)
- A/B: code retention **+0.3%** overall, **+2.6%** on codebases with 1,000+ files
- dissatisfied follow-up requests **+2.2%** when semantic search was removed
- they trained a custom embedding model on agent session traces (an LLM ranks what content
  would have helped at each step; the embedder learns to match those rankings)
- **their conclusion is hybrid**: *"our agent makes heavy use of grep as well as semantic
  search, and the combination of these two leads to the best outcomes."*

**Does this transfer from code to transcripts?** Partially, and the direction is *against*
grep. LlamaIndex's May 2026 treatment
([is-grep-all-you-need](https://www.llamaindex.ai/blog/is-grep-all-you-need-lexical-vs-sematic-search-for-agents),
**2026-05-26**) notes the debate is framed around text-based corpora (markdown, source code)
and argues the vocabulary-mismatch problem is the decider. Code has **near-perfect lexical
anchors** — identifiers are exact, unique, and machine-generated, so grep hits them exactly.
**Spoken transcripts have the worst possible lexical properties**: the speaker says "that
thing with the sign on the car", not "vehicle signage"; the same concept appears under a
dozen paraphrases; ASR introduces errors; there are no unique identifiers.

> **The grep-school argument is a property of code, not a property of agents. Do not import
> it wholesale into a transcript corpus.** Keep grep for names, dates, URLs and quoted
> phrases — where it is unbeatable — and put a semantic leg next to it.

One genuinely 2026 point in grep's favour that *does* transfer: once retrieval is a tool
call it becomes a learnable policy you can RL-tune, which you cannot do to a frozen
embedding model ([arXiv 2605.27123](https://arxiv.org/pdf/2605.27123),
*Rethinking Agentic RAG: Toward LLM-Driven Logical Retrieval Beyond Embeddings*).

### Self-RAG / CRAG in 2026: absorbed, not superseded

Neither is a standalone architecture any more; both are **components inside a generic agent
loop**. Measured benefit: Self-RAG-style critique loops cut unsupported-claim rates
**30–45%** on multi-hop questions vs single-shot RAG on RAGTruth and FaithBench — *but only
when the loop has a step cap*. The 2026 shift is **Adaptive RAG**: a complexity classifier
up front, since ~60–70% of production queries are simple and should skip the loop entirely.
One team measured **−60% cost** from a simple vector-similarity check before escalating,
with only ~15% of queries actually needing multi-step reasoning. A small local critic cut
routing overhead **785ms → 42ms** and cost per 10k queries **$3.00 → $0.06**.

A genuinely 2026 wrinkle nobody talks about — **memory-format portability across model
upgrades** ([arXiv 2609.05339](https://arxiv.org/abs/2609.05339), 2026-09-04). Four formats
(LC-RAW verbatim history / RAG chunks / NOTES compressed to natural language / KG-fixed
schema) over 48 synthetic histories, two open-weight <10B models, measured before and after
a model swap:

| format | accuracy change after model replacement |
|---|---|
| **KG-fixed** | **+0.0004 ± 0.0020** (stable) |
| RAG (chunked) | mixed-embedding migration recovered only 4.96 of 11.90 available points |
| NOTES (compressed) | **+9.91 or −13.28 pp** depending on migration direction |

Store-only NOTES repair never reached 90% recovery; **retaining raw histories enabled
recovery in 34 of 48 cases**. Caveat: sub-10B models on synthetic histories, so the absolute
numbers don't transfer — but the *ordering* is a third independent arrival at the same
conclusion as §2's verbatim result and §5's EffiRAG framing: **keep the raw text; derived
representations are the fragile part.**

Research frontier has moved to *post-hoc repair* of failed trajectories
([Doctor-RAG, arXiv 2604.00865](https://arxiv.org/pdf/2604.00865)) and step-level online
verification — both out of scope for us.

**Practical translation: don't implement "Self-RAG" or "CRAG" by name. Implement a relevance
grader, a routing edge, and a step-capped reflection loop behind a complexity gate.**

### Multi-hop and deep research

FRAMES (824 multi-hop questions, 2–15 Wikipedia articles each) is still the reference
multi-hop RAG benchmark, but **there is no comparable 2026 SOTA number** — figures vary by
harness (single-shot TSV vs agentic ReAct), retrieval corpus and judging protocol.
Tracked dated entries include GoLongRL-30B-A3B (2026-05-19) and "Orchestrator" (2025-11-26)
but accuracies were not exposed. **Flagged: I could not establish a trustworthy FRAMES SOTA.**
The robust cross-benchmark finding: accuracy falls monotonically with hop depth under a
fixed tool interface, while tool-call count rises.

Anthropic's multi-agent research system (June 2025) claimed **+90.2%** over a single agent
on their internal research eval at roughly **15× the tokens** — **[pre-2026]**, and I found
no 2026 follow-up with updated numbers. **Flagged.**

Counter-datapoint that bounded agents can beat heavy machinery: BLAgent completed all 300
benchmark instances at **$0.09/bug** with Claude-4.6 vs an effective **$1.65/instance** for
graph-traversal LocAgent ([arXiv 2605.17965](https://arxiv.org/pdf/2605.17965)).

### Memory systems (adjacent, and relevant to a personal corpus)

mem0's *State of AI Agent Memory 2026* (**2026-09-18** — published today)
([mem0.ai](https://mem0.ai/blog/state-of-ai-agent-memory-2026)) **[vendor]**:

| system | LoCoMo | LongMemEval | tokens/query |
|---|---:|---:|---:|
| mem0 2026 algo | **92.5** | **94.4** | 6,956 |
| Zep | 80.32 | 71.2 | — |
| Letta (filesystem-based) | 74.0 | — | — |
| OpenAI Memory | 52.9 | — | — |

Their headline argument is the one that matters here: **selective memory ~6,900 tokens/query
vs full-context ~26,000 tokens/conversation** — *"a system that scores well on accuracy but
requires 26,000 tokens per query is not production viable."* Note this is a vendor arguing
for its own product, and note that 26k tokens is trivially affordable at our query volume.
Open problems they name: temporal abstraction, cross-session identity resolution, memory
staleness — all three are live issues for a transcript corpus with recurring speakers.

---

## 7. evaluation

### What to use

The 2026 harness landscape is "intensely competitive with rapidly changing feature sets"
(hamel.dev, updated **2026-09-17**), and no harness is the differentiator. The named tools
still in play: LangSmith, Arize Phoenix, Braintrust, LangFuse, DeepEval, promptfoo, RAGAS,
Inspect (UK AISI).

**On RAGAS specifically — the criticism is now concrete:**

- Correlation between RAGAS metrics and human evaluation: **harmonic mean of only 0.55**
  ([arXiv 2506.20128, CCRS](https://arxiv.org/pdf/2506.20128)). That is not enough to steer
  development decisions.
- RAGAS `faithfulness` is a *pipeline*, not a call: an LLM extracts discrete claims, then an
  LLM runs NLI per claim against context, score = proportion verified. Brittle if any
  intermediate step fails, and computationally heavy.
- RAGAS' own docs now lead with **judge alignment**: *"a misaligned judge is like a compass
  pointing the wrong way — every improvement based on its guidance moves you further from
  your goal."* They recommend a `judge_alignment` metric (F1 of judge decision vs human
  expert verdict) and stopping when alignment plateaus over 2–3 iterations
  ([docs.ragas.io](https://docs.ragas.io/en/stable/howtos/applications/align-llm-as-judge/)).

**Use RAGAS-shaped metrics as a smoke test, not as the objective function.**

### LLM-as-judge pitfalls

Documented biases: position, verbosity, self-enhancement (judge prefers its own model's
output), authority — the CALM framework catalogues **12** distinct ones. MT-Bench found
GPT-4-class judges matched human evaluation at **>80%** agreement, with answer length
correlating strongly with win rate for **both** LLM judges and human annotators
**[pre-2026]**. Case-aware judging for enterprise RAG is an active 2026 direction
([arXiv 2602.20379](https://arxiv.org/pdf/2602.20379), 2026-02-25 — numbers not extractable
from the PDF; **flagged**).

Best practice that survives into 2026, from
[hamel.dev/blog/posts/evals-faq](https://hamel.dev/blog/posts/evals-faq/) (updated
**2026-09-17**):

- **Binary pass/fail over Likert.** "The difference between adjacent points (3 vs 4) is
  subjective and inconsistent across annotators."
- **Don't report raw agreement** — misleading under class imbalance. Report **TPR and TNR**
  separately on a held-out test set.
- Using the same model as judge and as the pipeline is fine *if* alignment with human labels
  is strong; the judge performs a different, scoped task.
- A worked example reached **>90% agreement** with the human expert in **three iterations**.
- **Beware high pass rates**: 100% means the eval isn't challenging the system; ~70%
  indicates a meaningful, stress-testing eval.

### Golden set sizing — the numbers

| purpose | size | source |
|---|---:|---|
| error discovery (hand-read traces) | **≥100 diverse traces**, ≥30 annotated by you personally before any automation | hamel.dev, 2026-09-17 |
| starting golden dataset | **30–50** real/realistic queries | [Braintrust](https://www.braintrust.dev/articles/what-is-rag-evaluation) |
| confidence over a defined corpus (~150 pages) | **50–100** | Microsoft, Data Science at Microsoft |
| labelled examples **per failure mode** to validate an LLM judge | **100–200**, split 10–20% train / 40–45% dev / 40–45% test, with **30–50 Pass and 30–50 Fail** in each of dev and test | hamel.dev, 2026-09-17 |
| deeper insight | a few hundred | Microsoft |
| pre-production reliability | 200+ | practitioner |

Composition beats size: **200 questions including reasoning and risk-analysis cases beat
10,000 scraped junk questions.** Coverage should span (a) factual questions with clear
answers, (b) multi-document synthesis, (c) ambiguous queries with >1 valid reading, and
(d) **cases where the correct answer is "that isn't in the corpus"** — this last class is
the one small-corpus assistants fail most embarrassingly.

**Difficulty mining beats a headline count.** The SIGIR 2025 LiveRAG team generated sets at
2/5/50/100/200/500/1000 questions, used small ones for debugging, then mined outputs for
"tricky" (179) and "challenging" (15) questions and built a **100-question main test set
from 15 tricky + 85 others** ([arXiv 2506.14516](https://arxiv.org/pdf/2506.14516))
**[pre-2026]**.

Synthetic question generation: usable as a *starter*, using a structured-dimensions approach
— **define ~20 manual tuples first**, then two-step generation (tuples → natural language).
Explicitly unreliable for complex domain-specific content, low-resource languages,
high-stakes domains, and underrepresented user groups (hamel.dev, 2026-09-17). The classic
failure mode is questions whose wording lexically overlaps the source chunk, which makes any
retriever look good.

**Version the golden set like code.** If success drops 92% (v1.0) → 75% (v1.1), you know the
problem is how new material was indexed, not the model.

### Metrics: the minimal set

Retrieval-side (cheap, deterministic, no judge needed — do these first):

- **recall@k at two values of k** (e.g. 5 and 50). The gap is your reranker headroom (§4).
- **MRR** or nDCG@10 if you have graded relevance; recall@k is enough if you don't.

End-to-end (needs a judge — validate it first):

- **binary correctness** (pass/fail, with a written reason)
- **claim-level faithfulness / attribution** — did every claim come from a retrieved source
- **judge_alignment** — the judge's own F1 against your human labels. **Track this as a
  first-class metric; it is the only thing telling you whether the other metrics mean anything.**

Agentic runs add trajectory metrics: **retrieves-per-correct-answer**, step count, p95
latency, and **cost-per-*correct*-answer** (the only rollup that actually adjudicates the
agentic-vs-fixed tradeoff, since cost-per-query always favours the fixed pipeline).

### Error analysis beats metrics

The Husain/Shankar school is unchanged and is still the highest-leverage advice available:

- **Look at raw data by hand before building any automated metric.** Everything else is
  scaffolding on top of that.
- Expect **60–80% of development time** on error analysis and evaluation.
- Continue reading traces until **theoretical saturation** — new traces stop revealing new
  failure modes.
- Ongoing: **100+ fresh traces every 2–4 weeks**, plus **10–20 traces weekly** focused on
  outliers.
- **One "benevolent dictator" annotator.** A single domain expert is the definitive quality
  voice; multiple annotators only for large multi-domain orgs.
- **Build a custom annotation tool.** Called the single highest-impact investment; teams with
  one iterate ~10× faster. Feasible in hours with AI-assisted dev.
- Not every failure mode deserves an eval — some are one-offs you just fix.
- You cannot write a good judge prompt until you have seen the data (Shankar et al.,
  *Who Validates the Validators?*).

---

## what this implies for a 1–3M token markdown corpus

### The shape of the thing

```
  ┌──────────────────────────────────────────────────────────────┐
  │ INDEX (cheap, rebuilt on every pull, no vector DB service)   │
  │                                                              │
  │  per-file: front-matter (date, source, speakers, duration)   │
  │            + loom/yt-generated summary  ← ALREADY EXISTS     │
  │            + chapter headings with timestamps ← ALREADY EXISTS│
  │                                                              │
  │  ≈300 files × ~200 tok = ~60k tokens                         │
  │  = a single context window that summarises EVERYTHING        │
  └──────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   grep / BM25          dense over           the whole index,
   (names, dates,       chapter units        pasted in, for
   URLs, quotes)        (paraphrase)         global/thematic Qs
        └─────────────────────┼─────────────────────┘
                              ▼
                   rerank / LLM-pick top 5-10
                              ▼
        ┌──────────────────────────────────────────┐
        │ return WHOLE chapters (or whole files)   │
        │ with timestamps intact → ≤200k context   │
        └──────────────────────────────────────────┘
```

### The architecture already in `docs/ARCHITECTURE.md` is the 2026-correct shape

This is worth stating plainly, because it means most of the work is already done and the
temptation to rebuild should be resisted:

| the repo's existing rule | the 2026 finding it satisfies |
|---|---|
| `ref/` is raw, verbatim, never hand-edited (Invariant) | verbatim chunks beat extracted artifacts by **+15.9 / +22.0 pp** ([2601.00821](https://arxiv.org/abs/2601.00821)); raw histories enabled recovery in 34/48 model-migration cases ([2609.05339](https://arxiv.org/abs/2609.05339)) |
| `structured/` holds distilled bullets, each linking back to `ref/` | *"adding artifacts alongside chunks preserves accuracy; substituting destroys it"* — the repo's promotion flow **is** the alongside-not-instead pattern, enforced as an Invariant ("nothing enters `structured/` that did not enter `ref/` first") |
| every claim carries a dated source link, link text **is** the date | claim-level attribution is the current recommended faithfulness metric (§7); the timestamped deep-link `?t=4210` is a one-click human verification |
| newest-wins recency, old claim demoted not deleted | directly mitigates GraphRAG's documented **−16.6%** on time-sensitive queries — the corpus carries its own temporal resolution instead of needing an index to |
| 7-topic taxonomy, "the taxonomy follows the data, a topic that keeps collecting unrelated bullets splits" | a hand-curated routing layer. This is what GraphRAG's community detection tries to learn, built by a human who knows the domain, for free |
| loom captures ship a summary + timestamped chapters | the chunk boundary and the summary index of §2/§5 **already exist on disk** |

**Nothing in the retrieval literature asks this repo to change how it stores anything.** The
gap is entirely on the read side: there is no search layer and no eval.

### Decisions this forces

1. **Do not stand up a vector database as infrastructure.** At 300 files an in-process index
   (SQLite FTS5 for BM25 + a flat numpy matrix of a few thousand embeddings, or LanceDB as a
   file) is sufficient and has no operational surface. Retrieval is 0–5% of latency
   ([arXiv 2601.07711v2](https://arxiv.org/html/2601.07711v2)) — there is nothing to optimise.

2. **Your chunk boundary is the loom chapter, and it already exists.** The transcripts carry
   `### <title> <mm:ss>` headings generated at capture time. That is a semantic chunk with a
   free title, a free timestamp, and a free parent document. Do not re-derive it with a
   semantic chunker. Index at two granularities — **file** and **chapter** — and always
   *return* the larger unit.

3. **Timestamps are a first-class retrieval output, not decoration.** The per-second stamps
   mean every answer can cite `loom.com/share/<id>?t=<seconds>`. Build the citation format
   into the tool's return value from day one; it is the cheapest possible faithfulness
   mechanism and it lets a human verify in one click.

4. **Two lexical properties of this corpus dominate everything.**
   - Spoken paraphrase kills pure grep (§6). A semantic leg is mandatory — this is *not* the
     code case where Anthropic dropped vectors.
   - Proper nouns (people, cities, tool names, GMB/SEO jargon) are exact and high-signal.
     BM25 is excellent at these and costs nothing. **Hybrid is not a hedge here; each leg
     covers a failure mode the other has.**

5. **Speaker attribution and recency are metadata filters, and they are worth more than
   any embedding upgrade.** "What did Eric say in August" is a `WHERE` clause, not a
   similarity problem. Extract `speaker` and `recorded` into the index and expose them as
   tool parameters. Note `tmp/research/our-corpus.md` already flags that 359k tokens of
   *speaker-attributed* skool discussion live outside this repo — closing that gap is worth
   more than any retrieval technique in this document.

6. **Build the "stuff everything in" baseline first and keep it forever.** At 163k tokens it
   is one prompt-cached call (~$0.08 warm). It is your quality ceiling and your regression
   detector: any retrieval system that scores below it is losing information, and you will
   only know that if you measure both. This also means **you have a working assistant today,
   before writing any retrieval code.**

7. **The agent gets a search tool with a hard step budget of ~4 calls.** Agentic loops buy
   routing and query rewriting (+2.8 nDCG@10) and lose at reranking (−5.6) at 3.3× the input
   tokens. Cap the loop, log retrieves-per-correct-answer, and gate escalation on a
   complexity check — ~60–70% of queries are simple and shouldn't pay for the loop.

8. **When you do add a dense leg, do not default to OpenAI.** `text-embedding-3-large` has
   had no successor since **January 2024** and sits ~14th on RTEB. For this corpus the
   shape-matched choice is **`voyage-context-4`** ($0.12/M, 32K, auto-splits beyond) — it
   embeds each chunk conditioned on the whole document, which is *exactly* the
   chapter-inside-a-transcript relationship we have, and it is +7.11% on LongEmbed over
   single vectors. Local alternatives with no API dependency: `jina-embeddings-v5-text-small`
   (677M, 32K, best multilingual under 1B, near-lossless binary quantization) or
   `Qwen3-Embedding-8B` / `harrier-oss-v1` (MIT, 32K) if you have the GPU. A 300-document
   corpus re-embeds in seconds, so **the switching cost is ~zero — never lock this in.**

9. **Fusion: RRF to start, convex combination once you have labels.** And check your library's
   `k` — it is 60 in Elasticsearch/Milvus/Azure and **2 in Qdrant**. Once the golden set
   exists, CC at α≈0.5 measured **0.726 vs 0.695 recall@5** against RRF k=60. But note the
   same paper's full pipeline hit **0.816** once a cross-encoder was added: **the reranker is
   worth more than all fusion tuning combined.** Tune in that order.

10. **Use Anthropic's `search_result` content blocks for the tool's return type.** It is not
    hosted retrieval — Anthropic ships no vector store — it is a content-block type that
    gives *your* retriever's output first-class citations with source and title. That is
    precisely the primitive the timestamped-deep-link requirement (item 3) needs, and it
    removes any temptation to hand-roll citation formatting in the prompt.

11. **Skip: GraphRAG, semantic chunkers, ColBERT/multi-vector, learned sparse, a reranker
    service, Self-RAG as a named system.** Each is defensible at 10× this corpus size. The
    late-interaction case is the clearest: ColBERT-Zero's own controlled ablation puts
    multi-vector at **+2.5 nDCG@10 over dense on identical data — less than the 2.4-point gap
    between good and bad *training data* on the dense side** — for ~64× the storage per
    document. Pay that when the corpus is large and out-of-domain, not now.

12. **Spend the saved effort on evaluation.** 50 hand-written questions across the four
    coverage classes (factual / multi-doc synthesis / ambiguous / **not-in-corpus**), binary
    pass-fail, a judge validated against your own labels with TPR+TNR reported, and 100 traces
    read by hand. That is a weekend, and it is the only thing that will tell you whether any
    of the above was right.

    **Build the eval set evidence-*dense*.** HiChunk's central finding is that
    sparse-evidence benchmarks cannot distinguish *any* chunking strategy — on GutenQA and
    OHRBench every method scored the same. If your 50 questions are single-fact lookups, every
    architecture in this document will tie and you will learn nothing. Write questions whose
    answer requires two or three separate moments across different calls.

13. **Recheck this document when the corpus crosses ~1M tokens.** Everything above is sized to
    "the corpus fits in a handful of context windows". Past ~10M the advice inverts.

### What would change the call

| if | then |
|---|---|
| corpus grows past ~10M tokens | real chunk-level hybrid index; late interaction becomes worth pricing |
| queries turn out to be mostly cross-document synthesis | RAPTOR-style summary tree (**not** a graph) |
| eval shows recall@50 ≫ recall@5 | add a reranker — that gap *is* the payoff, and nothing else is |
| eval shows recall@50 itself poor | the leak is in indexing/chunk boundaries, not ranking. Do not buy a reranker |
| every architecture ties on the eval | your eval is evidence-sparse (HiChunk). Rewrite the questions, not the retriever |
| latency becomes the complaint | drop the agent loop for the fixed pipeline; 45–50% of wall clock is generation, 0–5% is retrieval |
| you swap the generator for a small/cheap model | the 2,500-token context cliff comes back ([2601.14123](https://arxiv.org/abs/2601.14123)) and "return whole documents" stops being safe |
| PDFs or screenshots enter the corpus | that is the one place late interaction is unambiguously SOTA — ColPali line, ViDoRe v3 ~61 |

---

## sources

Primary / dated, grouped by how much weight they carry.

**Model + platform facts**
- Anthropic, *Introducing Claude Opus 4.6*, 2026-02-05 — https://anthropic.com/news/claude-opus-4-6 (1M beta window, MRCR v2 8-needle 76% @1M vs Sonnet 4.5 18.5%, context compaction, $5/$25 base, $10/$37.50 >200k)
- Anthropic, *Effective context engineering for AI agents* — https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents (just-in-time retrieval)
- Anthropic, *Building Effective AI Agents* — https://www.anthropic.com/engineering/building-effective-agents
- Anthropic, *Introducing Contextual Retrieval*, 2024-09-19 — https://www.anthropic.com/news/contextual-retrieval (**[pre-2026]** 35% / 49% / 67% failure-rate reductions; 800-tok chunks; $1.02/M doc tokens; top-20 > top-10 > top-5)
- Anthropic, *2026 Agentic Coding Trends Report* — https://resources.anthropic.com/2026-agentic-coding-trends-report
- BenchLM, *Claude API Pricing (September 2026)* — https://benchlm.ai/anthropic/api-pricing
- TokenCost, *Prompt Caching Pricing in 2026* — https://tokencost.app/blog/prompt-caching-pricing-2026 (the 0.1× cache-read convergence)
- Flexera, *Prompt Caching breakdown*, 2026 — https://www.flexera.com/blog/ai/prompt-caching-breakdown/ (Claude 4.7+ tokenizer ~30% more tokens)

**Long context vs RAG**
- Chroma, *Context Rot*, July 2025 — https://www.trychroma.com/research/context-rot (**[pre-2026]**, 18 models, 194,480 calls; vendor COI noted)
- TMLS, *Context Rot: mechanistic*, 2026-06 — https://www.tmls.nyc/research/context-rot-mechanistic (positional vs length degradation separated)
- arXiv 2606.29718 — *Diagnosing and Mitigating Context Rot in Long-horizon Search*
- arXiv 2605.12366 — *Classifier Context Rot*
- arXiv 2602.07962 — *LOCA-bench* (agentic context growth)
- arXiv 2501.01880 — *Long Context vs. RAG for LLMs* / LaRA (**[pre-2026]**)
- arXiv 2605.10235 — *Route Before Retrieve*
- arXiv 2606.20898, 2026-06-23 — *The Token Tax of Epistemic Accuracy* (**PDF text not extractable; abstract-level only**)
- arXiv 2601.06551 — *L-RAG: entropy-based lazy loading*
- Wire, *RAG vs long context: what the 2026 data shows*, 2026-04-08 upd. 2026-07-20 — https://usewire.io/blog/long-context-vs-rag-what-the-data-shows/ (**[practitioner]** 1,250×, 45s vs 1s, 200k threshold)
- TianPan, *Long-Context Models vs. RAG*, 2026-04-09 — https://tianpan.co/blog/2026-04-09-long-context-vs-rag-production-decision-framework (**[practitioner]** 48k beats 117k by 13 F1)

**Chunking**
- arXiv 2602.16974, 2026-02-19 — *Beyond Chunk-Then-Embed* (SIGIR '26) — https://arxiv.org/abs/2602.16974 (**the best-controlled 2026 chunking study**: taxonomy, in-corpus vs in-document split, proposition chunking 15–27% worse, 167× throughput spread)
- arXiv 2603.06976, 2026-03-07 — *Chunking Strategies & Embedding Sensitivity* — https://arxiv.org/html/2603.06976 (**chunking dominates embedding-model choice**; 0.459 vs 0.244 nDCG@5)
- arXiv 2601.14123, 2026-01-20 — *A Systematic Analysis of Chunking Strategies for Reliable QA* — https://arxiv.org/abs/2601.14123 (overlap useless; 2.5k context cliff — **small-generator caveat, see §2**)
- arXiv 2604.12047, 2026-04 — PDF parsing + chunking for financial QA — https://arxiv.org/pdf/2604.12047 (25% overlap; MRR 0.529→0.658 FinanceBench, 0.735→0.833 TableQuest)
- arXiv 2603.25333, 2026-03-26 (LREC 2026) — *Adaptive Chunking* — https://arxiv.org/abs/2603.25333 (per-doc chunker selection; correctness 62–64%→72%)
- arXiv 2509.11552 — *HiChunk / HiCBench*, ACL 2026 Long — https://arxiv.org/abs/2509.11552 (**benchmarks with sparse evidence cannot distinguish chunkers**; Auto-Merge; 81.03/68.12 vs 74.06/63.20)
- **arXiv 2601.00821, latest 2026-07-22 — *Fidelity Before Structure: Verbatim Chunks Beat Lossy Artifact Extraction*** — https://arxiv.org/abs/2601.00821 (**the single most applicable result here**: verbatim 43.9/67.4 vs artifacts 28.0/45.4 on LoCoMo/LongMemEval-S)
- arXiv 2410.13070 — *Is Semantic Chunking Worth the Computational Cost?* (Findings of NAACL 2025) — https://arxiv.org/abs/2410.13070 **[pre-2026]**
- arXiv 2409.04701 (v3 2025-07-07) — *Late Chunking* (Jina) — https://arxiv.org/abs/2409.04701 **[pre-2026] [vendor]**
- arXiv 2504.19754 — Merola & Singh, independent reproduction of late chunking **and** Anthropic contextual retrieval — https://arxiv.org/abs/2504.19754 (**contextual 0.317 vs baseline 0.312**; late chunking −20% on MSMarco/Stella-V5)
- arXiv 2606.18781 / 2606.23642, 2026-06 — chunk evidence aggregation / multi-prefix embedding
- arXiv 2606.00881, 2026-05-30 — chunking methods vs computational cost (**tables did not extract**)
- Chroma, *Evaluating Chunking Strategies*, 2024-07-03 — https://research.trychroma.com/evaluating-chunking **[pre-2026]** (note the `LLMSemanticChunker` vs `KamradtSemanticChunker` misattribution trap)
- Vecta/FloTorch, 7-strategy benchmark, 2026-02 — https://www.runvecta.com/blog/we-benchmarked-7-chunking-strategies-most-advice-was-wrong (**Cloudflare-blocked to automated fetch; numbers second-hand via premai.io 2026-03-17**)
- `turnchunk` 0.4.1, PyPI, 2026-09-02 — https://pypi.org/project/turnchunk/0.4.1/ (only packaged turn-aware chunker; 11 transcript formats)
- Chonkie issue #658, opened 2026-08-28 — https://github.com/feyninc/chonkie/issues/658 (no mainstream library models a conversation)

**Retrieval, fusion, embeddings**
- **arXiv 2508.21038 — *On the Theoretical Limitations of Embedding-Based Retrieval*, ICLR '26, v2 2026-03-12** — https://arxiv.org/abs/2508.21038 (**LIMIT: single-vector embeddings provably cannot express all top-k subsets; BM25 wins outright**)
- arXiv 2604.01733, 2026-04-02 — *From BM25 to Corrective RAG* — https://arxiv.org/abs/2604.01733 (CC α=0.5 **0.726** vs RRF k=60 **0.695** recall@5; full pipeline **0.816** / MRR@3 0.605; BM25 > dense on financial docs)
- arXiv 2210.11934 — Bruch et al., *An Analysis of Fusion Functions for Hybrid Retrieval* (ACM TOIS) — https://arxiv.org/abs/2210.11934 **[pre-2026, still canonical]**
- arXiv 2608.27017, 2026-08-27 — *ProRetrieval: orchestrating hybrid search via program synthesis*
- arXiv 2609.17012, 2026-09-15 — *ORDER: task-conditioned routing for RAG*
- arXiv 2602.16609, 2026-02-19 — **ColBERT-Zero** (55.43 BEIR nDCG@10 <150M; the dense-vs-multivector controlled ablation) — https://arxiv.org/abs/2602.16609
- arXiv 2511.00444 — **LIR 2026**, first workshop on Late Interaction and Multi-Vector Retrieval @ ECIR 2026
- Google Research, *MUVERA*, 2025-06-25 — https://research.google/blog/muvera-making-multi-vector-retrieval-as-fast-as-single-vector-search/ **[pre-2026]** (+10% recall, −90% latency vs PLAID, 32× memory under PQ)
- Liquid AI, *LFM2.5 retrievers*, 2026-06-18 — https://www.liquid.ai/blog/lfm2-5-retrievers (ColBERT 8.1ms vs dense 7.3ms CPU p50 — latency is not the cost, storage is)
- OpenSearch-AI/Ops-Colqwen3-4B model card — https://huggingface.co/OpenSearch-AI/Ops-Colqwen3-4B (ViDoRe v1 91.4 / v2 68.7; **128 dims loses only 0.83 nDCG@5**)
- TomoroAI/tomoro-colqwen3-embed-4b model card — https://huggingface.co/TomoroAI/tomoro-colqwen3-embed-4b (10.3 TB vs 0.82 TB per 1M images)
- opensearch-neural-sparse-encoding-doc-v3-gte model card, 2025-06-18 **[pre-2026]** (inference-free sparse, 0.546 nDCG@10 @ 1.7 FLOPS)
- arXiv 2606.18811, 2026-06-17 — *Rescaling MLM-Head for Neural Sparse Retrieval* (why stronger encoders fail to improve SPLADE)
- arXiv 2607.00004, 2026-04-20 — *Why Advanced Encoders Lag on Sparse Retrieval?*
- **HF blog, RTEB launch, Oct 2025** — https://github.com/huggingface/blog/blob/main/rteb.md ("many models train on the test set, some intentionally")
- **embeddings-benchmark/mteb issue #3934, 2026-01-14 — private RTEB column removed for Voyage conflict of interest** — https://github.com/embeddings-benchmark/mteb/issues/3934 (**still open**)
- benchmarklist.com RTEB snapshot, updated 2026-05-06 — https://benchmarklist.com/benchmarks/rteb/
- codesota.com MMTEB snapshot, updated 2026-05-17 — https://www.codesota.com/benchmarks/mteb (**incomplete — omits Harrier**)
- zeroentropy.dev, *MTEB overfitting diagnostics* — https://zeroentropy.dev/concepts/mteb/ (15+ nDCG drop on private domain = the tell)
- arXiv 2606.22778, 2026-06-22 — **HAKARI-Bench** (35 benchmarks, 5 retrieval families, 55 models, Spearman >0.97 with MTEB/BEIR, MIT)
- arXiv 2605.28190 — **HTEB**, Harder Text Embedding Benchmark
- Voyage, *voyage-4*, 2026-01-15 — https://blog.voyageai.com/2026/01/15/voyage-4/ (MoE; shared embedding space across the family; `-nano` Apache-2.0)
- Voyage, *voyage-context-4*, 2026-06-29 — $0.12/M, **+7.11% LongEmbed for contextualized chunks over single vectors**
- Google, *Gemini Embedding 2 GA*, ~2026-04 — https://blog.google/innovation-and-ai/models-and-research/gemini-models/gemini-embedding-2-generally-available/ (**embedding space incompatible with -001, full re-index**)
- Jina, *jina-embeddings-v5-text*, 2026-02 — https://jina.ai/news/jina-embeddings-v5-text-distilling-4b-quality-into-sub-1b-multilingual-embeddings/ + https://arxiv.org/html/2602.15547v2
- Microsoft Bing, *harrier-oss-v1*, 2026-03 — https://www.marktechpost.com/2026/03/30/microsoft-ai-releases-harrier-oss-v1-a-new-family-of-multilingual-embedding-models-hitting-sota-on-multilingual-mteb-v2/ (**[vendor], unconfirmed third-party**)
- arXiv 2601.04720, 2026-01 — *Qwen3-VL-Embedding / Reranker* (MMEB-V2 77.8)
- OpenAI pricing page, fetched 2026-09-18 — https://developers.openai.com/api/docs/pricing (**text-embedding-3-large is still the newest; no successor since 2024-01**)
- **Anthropic, search result content blocks** — https://platform.claude.com/docs/en/build-with-claude/search-results (a `search_result` content-block type for citing *your own* retriever; Anthropic ships no managed vector store)
- Google, Gemini File Search — https://ai.google.dev/gemini-api/docs/file-search (storage + query embeddings free; **semantic only, no BM25**)
- OpenAI file search — https://developers.openai.com/api/docs/guides/tools-file-search ($0.10/GB/day + $2.50/1k calls; **no disclosed model, no published benchmark**)
- Qdrant hybrid queries — https://qdrant.tech/documentation/concepts/hybrid-queries/ (**RRF k defaults to 2**; DBSF since v1.11.0; `max_sim` since v1.10.0)
- Elasticsearch RRF reference — https://www.elastic.co/docs/reference/elasticsearch/rest-apis/reciprocal-rank-fusion (k=60, GA)
- Elasticsearch `rank_vectors` mapping — https://www.elastic.co/docs/reference/elasticsearch/mapping-reference/rank-vectors (**still Preview; no ANN index, rescoring only**)
- Weaviate hybrid search docs — https://docs.weaviate.io/weaviate/search/hybrid (**relativeScoreFusion is the default, not RRF**)
- Turbopuffer query docs — https://turbopuffer.com/docs/query (operator fusion; multi-vector in private beta)
- LanceDB multivector search — https://docs.lancedb.com/search/multivector-search (**cosine only**)
- Voyage reranker docs, updated 2026-09-01 — https://docs.voyageai.com/docs/reranker (`rerank-3` preview, 32K)
- Cohere models list, fetched 2026-09-18 — https://docs.cohere.com/docs/models (`rerank-v4.0-pro`/`-fast` at 32K; **embed-v4.0 still current, no v5**)

**Reranking**
- Agentset reranker leaderboard, updated 2026-02-15 — https://agentset.ai/rerankers
- Agentset, *Best Reranker for RAG: we tested the top models* — https://agentset.ai/blog/best-reranker
- Redis, *Top Reranking Models to Boost RAG Accuracy in 2026* — https://redis.io/blog/top-reranking-models-rag-accuracy/ (+5–15 nDCG@10, <200ms)
- dev.to, *The Reranker Setup That Actually Changes Recall@5*, 2026 — https://dev.to/gabrielanhaia/the-reranker-setup-that-actually-changes-recall5-1ole (0.61→0.79)
- dev.to, *When the Reranker Hurts* — https://dev.to/gabrielanhaia/when-the-reranker-hurts-recall5-cases-where-two-stage-retrieval-loses-to-one-3kff (0.83→0.71 regression)
- arXiv 2605.24297 — *Benchmarking Patent Embeddings* (bge-reranker-v2-m3 hurt BM25: 0.153→0.140)
- arXiv 2510.22733 — *E2Rank* (embedding as listwise reranker)

**GraphRAG**
- arXiv 2609.18099, 2026-09-16 — *When Is Graph Structure Worth Its Cost? / EffiRAG* — https://arxiv.org/abs/2609.18099 (93/120 preferred; $0.408 vs $0.952; 4.2–4.5× cheaper at 10–20 docs/domain)
- CruxDigits, *RAG vs GraphRAG: A 2026 Decision Framework*, 2026-07-24 — https://cruxdigits.nl/blog/rag-vs-graphrag-2026/ ($33k → parity; >700× lower query cost; hybrid +15–30% recall; 0.72→0.9; −16.6% on time-sensitive queries)
- Graph Praxis, *The GraphRAG Cost Cliff: How $33,000 Became $33 in Eighteen Months* — https://medium.com/graph-praxis/the-graphrag-cost-cliff-how-33-000-became-33-in-eighteen-months-be1b0fbe37e4 (403 to automated fetch; cited via search summary)
- arXiv 2601.21162 — *A2RAG: Adaptive Agentic Graph Retrieval for Cost-Aware Reasoning*

**Agentic retrieval**
- arXiv 2601.07711v2, 2026-04-20 — *Is Agentic RAG worth it?* — https://arxiv.org/html/2601.07711v2 (**the key controlled study**: 3.3×/1.9× tokens, 1.5× latency, +2.8 nDCG rewriting, −5.6 nDCG reranking, latency breakdown)
- arXiv 2605.27123 — *Rethinking Agentic RAG: Toward LLM-Driven Logical Retrieval Beyond Embeddings*
- arXiv 2605.05632 — *Architecture Matters: RAG under Knowledge Base Poisoning* ($0.0004 vs $0.0011/query)
- arXiv 2605.17965 — *BLAgent* ($0.09/bug vs $1.65 LocAgent)
- arXiv 2604.00865 — *Doctor-RAG* (post-hoc trajectory repair)
- arXiv 2601.11888 — *Agentic-R: Learning to Retrieve for Agentic Search*
- Cursor, *Improving agent with semantic search*, 2025-11-06 — https://cursor.com/blog/semsearch (**[pre-2026]** +12.5% avg accuracy, 6.5–23.5% range, +2.6% retention on 1000+ file repos, +2.2% dissatisfied without it; conclusion = hybrid)
- LlamaIndex, *Is grep all you need? Lexical vs semantic search for agents*, 2026-05-26 — https://www.llamaindex.ai/blog/is-grep-all-you-need-lexical-vs-sematic-search-for-agents
- Sourcegraph, *Context Engineering: A Practical Guide for AI Agents (2026)* — https://sourcegraph.com/blog/context-engineering
- mem0, *State of AI Agent Memory 2026*, 2026-09-18 — https://mem0.ai/blog/state-of-ai-agent-memory-2026 (**[vendor]** LoCoMo 92.5 / LongMemEval 94.4 @6,956 tok; Zep 80.32; Letta 74.0; OpenAI Memory 52.9)
- arXiv 2602.22769 — *AMA-Bench: long-horizon memory for agentic applications*

**Evaluation**
- Hamel Husain & Shreya Shankar, *AI Evals: Everything You Need to Know* (FAQ), updated 2026-09-17 — https://hamel.dev/blog/posts/evals-faq/ (100 traces / 100–200 labels per failure mode / binary over Likert / TPR+TNR / 60–80% of time / benevolent dictator / custom annotation tool)
- Hamel Husain, *Using LLM-as-a-Judge For Evaluation* — https://hamel.dev/blog/posts/llm-judge/
- arXiv 2506.20128 — *CCRS: A Zero-Shot LLM-as-a-Judge Framework for Comprehensive RAG Evaluation* (**RAGAS–human correlation harmonic mean 0.55**)
- arXiv 2602.20379, 2026-02-25 — *Case-Aware LLM-as-a-Judge Evaluation for Enterprise-Scale RAG* (**numbers not extractable from PDF**)
- Ragas docs, *Align an LLM as a Judge* — https://docs.ragas.io/en/stable/howtos/applications/align-llm-as-judge/
- Braintrust, *What is RAG evaluation?* — https://www.braintrust.dev/articles/what-is-rag-evaluation (30–50 starting golden queries)
- Microsoft, *The path to a golden dataset* — https://medium.com/data-science-at-microsoft/the-path-to-a-golden-dataset-or-how-to-evaluate-your-rag-045e23d1f13f (50–100 for ~150 pages)
- arXiv 2506.14516 — *RMIT-ADM+S at SIGIR 2025 LiveRAG* (**[pre-2026]** difficulty mining, 100-question main set from 15 tricky + 85)
- arXiv 2404.17347 — *InspectorRAGet* (**[pre-2026]**)

**Local ground truth**
- `tmp/research/our-corpus.md` (2026-09-18, measured at `51774b7`) — repo total ~148k tokens / 75 files; 359k tokens of speaker-attributed skool discussion outside the repo
- `find . -name '*.md'` at 2026-09-18 — 80 files, 652,797 bytes, ~163k tokens; largest single file `ref/loom/2026-09-18-group-call.md` at 216KB (~54k tokens)

### Gaps and caveats

- **No independent replication** of Anthropic's MRCR v2 numbers was found; they are
  self-reported. The one third-party check located was on a different harness.
- **No trustworthy FRAMES SOTA for 2026.** Harness variance (single-shot vs agentic ReAct)
  makes published figures non-comparable.
- **Anthropic's multi-agent research +90.2% / 15× tokens figure is from June 2025** and I
  found no 2026 follow-up.
- Several high-traffic numbers (1,250× cost ratio, "RAG usage grew 400%", "60% of production
  LLM apps use retrieval") come from vendor/practitioner blogs, not measurements. They are
  reported above with that label and should not be used to justify a decision on their own.
- Two arXiv PDFs (2606.20898, 2602.20379) resisted text extraction; only abstract-level
  claims are carried here.
- Web-search budget for this session was exhausted before a targeted sweep on
  "whole-document retrieval + long context" as a named pattern; the recommendation in §1
  rests on the small-to-big F1 result and the context-rot curve rather than a paper that
  studies exactly that configuration. **Treat as reasoned inference, not a cited finding.**
- **The live MTEB/RTEB leaderboard could not be read** (Gradio SPA, JS-rendered). Every
  embedding ranking in §3 comes from a dated aggregator snapshot (2026-05-06 / 2026-05-17) or
  a vendor model card, and the aggregators demonstrably omit at least one major 2026 release
  (Harrier). **Verify at leaderboard.mteb.org before acting on a ranking.**
- **The RTEB leaderboard itself is under an unresolved conflict-of-interest dispute**
  (mteb#3934, opened 2026-01-14, still open). Voyage co-developed RTEB and had access to its
  private evaluation data; the private column has been removed. Voyage's leading position on
  that board should be read with that in mind.
- ❓ "Wholembed v3" could not be corroborated anywhere and is excluded from all
  recommendations above.
- The three most decision-relevant numbers in this document each rest on **a single study**:
  the verbatim-vs-artifacts gap (2601.00821), the agentic cost multiplier (2601.07711), and
  HiChunk's evidence-density critique (2509.11552). None has an independent replication.
  They are load-bearing here because they are the only controlled measurements that exist for
  their questions — not because they are settled.
