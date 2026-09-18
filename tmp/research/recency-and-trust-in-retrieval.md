# Recency and trust in retrieval

What is known, as of 2026-09-18, about making retrieval prefer newer and more
trustworthy claims, and about surfacing contradictions between sources of different
ages. Written against this repo's problem: dated claims about Google Business Profile
tactics, Dec 2025 → Sep 2026, from three source classes of very different reliability, in a
domain that rots in weeks.

Every claim below carries a URL and a date. `[unverified]` marks anything I could
not confirm by fetching the primary source.

> **Scope note.** `tmp/research/our-corpus.md` measures the repo at ~148k tokens — one
> context window — growing ~2.5M tokens/year. So the *ranking* half of this document is
> about a year early; the write-time and prompt halves apply today. See
> [first, when this formula applies at all](#first-when-this-formula-applies-at-all).

## the short answer

The measured result the literature keeps reproducing is uncomfortable for the obvious
design: **a naive recency term makes retrieval worse, and the write-time fix beats
the query-time fix by a wide margin.**

```
                        measured effect on answer quality
  naive "prefer newer"  ──────────────────────────────────►  −15.3%   Chronofy, Jul 2026
  decay tied to the
  query's time focus    ──────────────────────────────────►  +66.3%   Chronofy, same table
  write-time supersession
  (retire the old fact) ──────────────────────────────────►  0.20→0.95 acc   MemStrata, Jun 2026
                                                             58%→90%         VersionRAG, Oct 2025
```

Four things follow, in order of how much evidence backs them.

1. **Decay must key off the *query's* time focus, not off wall-clock age.** Chronofy's
   Table VI runs both on the same corpus: plain recency reranking drops accuracy 15.3%
   below a semantic baseline, while decay anchored to the time the *question* is about
   lifts Gold@5 by 66.3%. For this repo that distinction is mostly free, because nearly
   every question is implicitly "what works now" — but it means the decay term belongs
   downstream of a check that the user didn't ask "what did he say in December".

2. **Nobody needs to auto-detect the contradictions here, because a human already did.**
   Frontier models top out at 55% at *localising* which two passages disagree (MAGIC,
   EMNLP 2025 Findings), against 83.3% for humans, falling to 32.67% when the conflict
   spans two hops. This repo's `ARCHITECTURE.md` already records supersession
   structurally — new claim in place, old claim demoted beneath it with its date. That
   turns a 55%-accurate inference problem into a 100%-accurate read.

3. **Authority has to be an explicit multiplier, because the model's own priors get
   flipped by repetition.** Schuster et al. (Jan 2026) show 13 open-weight LLMs do
   prefer institutionally-corroborated sources — and that simply repeating a
   low-credibility source reverses that preference. A group call where four members echo
   a wrong tactic will out-vote one teacher lesson unless authority is scored outside
   the model.

4. **Freshness is a ranking bias, not a filter.** That is Microsoft's own wording in the
   Azure AI Search freshness-aware retrieval doc, and it matches every academic result:
   hard date filters lose the still-valid old claims, which in this corpus is most of the
   structural material (what a suspension *is*) as opposed to the tactical material (what
   trips one *this month*). The strongest confirmation is from the other end of the stack —
   I read Graphiti's retrieval code and **expired edges are never filtered out**; they come
   back with their invalidation dates attached and the caller narrates the change (§4). The
   state of the art in bitemporal memory keeps the dead fact visible on purpose.

The lazy, evidence-backed design for this repo is therefore: **keep resolving
contradictions at write time exactly as `ARCHITECTURE.md` already says, add a per-claim
`kind` tag (tactical / structural), and put a single decay term plus a three-tier
authority multiplier in the ranker.** Do not build a temporal knowledge graph. The
papers that build one are solving the case where nobody curates; this repo curates.

---

## 1. temporal / time-aware RAG

### How much does a decay term actually help?

This is the sharpest question and it has one clean answer and a lot of noise.

**The clean answer — Chronofy** ("Chronofy: A Temporal-Logical Decay Architecture for
Information Validity in Time-Aware Retrieval-Augmented Generation", Syed, Silaghi,
Abujar, Akter, arXiv:2607.20560, submitted 2026-07-17,
<https://arxiv.org/abs/2607.20560>). Evaluated on TimE-Lite News, 897 questions:

| Method | Accuracy | Gold@5 | Δ Accuracy |
|---|---|---|---|
| Vanilla (semantic only) | 0.450 | 0.341 | — |
| Recency (naive newest-first bias) | 0.381 | 0.200 | **−15.3%** |
| Oracle temporal proximity | 0.488 | 0.498 | **+8.4%** |

and the layer ablation, with an oracle temporal focus:

| Variant | Gold@5 | Δ |
|---|---|---|
| Semantic-only | 0.341 | — |
| + Layer 1 (temporal embedding subspace) | 0.385 | +12.7% |
| + Layer 2 (decay reranking) | 0.567 | **+66.3%** |
| Full (L1+L2) | 0.593 | +73.9% |

Their own sentence: *"Layer 2 (decay reranking) is the dominant contributor, lifting
Gold@5 from 0.341 to 0.567 (+66.3%)."* The catch is the word *oracle* — their
end-to-end temporal-focus extractor only covered 37.8% of queries, so the realised gain
in the parsed (non-oracle) variant was 2.0%. **The decay term is worth a lot; knowing
what time the question is about is the hard part.** For us that part is nearly free,
since the question is almost always "now".

The formula, verbatim (their Eq. 2):

```
w(e_ij) = q_e · c(tr_j) · exp(−β_j · (T_q − T_f))
```

`q_e` = source reliability, `c(tr_j)` = semantic confidence, `β_j` = learnable decay
coefficient *per fact type*, `T_q` = query time, `T_f` = fact timestamp. Note the shape:
**authority × relevance × exp-decay, multiplicative.** Their temporal-validity signal
(Eq. 3) uses a weakest-link min over the facts backing a statement:

```
v(s_i) = min_{e ∈ facts(s_i)} q_e · exp(−β_j(e) · (T_q − t_e))
```

**Per-fact-type decay coefficients they fit** — this is the most directly transferable
number in the whole literature, because it shows decay rates spanning two orders of
magnitude *within one corpus*:

- GDELT political events (their Table III): diplomatic actions β* = 0.3; conflict events
  (Assault, Fight) β* = 0.5–0.6.
- MIMIC-IV clinical (their Table V): diagnoses β* = **0.0** (stable, no decay at all);
  vital signs β* = 0.001; prescriptions β* = 0.001; lab results β* = 0.05.

The time unit for β is not stated on the pages I fetched `[unverified]` — treat the
*ratios* as the finding, not the absolute magnitudes. The finding is: **one global
half-life is wrong; fact class determines the half-life, and some classes should not
decay at all.**

Theoretical grounding they offer (their Proposition 1): the optimal decay coefficient is
`β_j = 2κ_j`, where κ is the mean-reversion rate under Ornstein-Uhlenbeck dynamics. This
is the only attempt I found to *derive* a half-life rather than tune it. I have not
checked the proof.

**The second clean answer — Grofsky**, "Freshness and the Limits of Heuristic Trend
Detection in Temporal RAG" (Matthew Grofsky, arXiv:2509.19376, v1 2025-09-20, v2
2026-06-26, <https://arxiv.org/abs/2509.19376>). Cybersecurity/NVD CVE data. A
**half-life recency prior** surfaces the newest relevant item where a cosine-only
baseline scores 0.00; Latest@10 of **0.60** vs **0.20** for a semantic-then-newest
baseline. The author's own hedge: results are *"partial and parameter-sensitive."* The
companion finding is a warning — the heuristic *topic-evolution* tracker scored 0.08
macro-F1, i.e. detecting that a topic has shifted is much harder than preferring the
newest item within a topic.

### Time-aware retrieval that does more than decay

**MRAG / TempRAGEval** — "MRAG: A Modular Retrieval Framework for Time-Sensitive
Question Answering" (Siyue Zhang, Yuxiang Xue, Yiming Zhang, Xiaobao Wu, Anh Tuan Luu,
Chen Zhao, arXiv:2412.15540, v1 2024-12-20, v2 2025-05-21,
<https://arxiv.org/abs/2412.15540>). This is where the TempRAGEval benchmark comes from.

- Benchmark: 1,000 test examples, 500 rewritten from TimeQA + 500 from SituatedQA, with
  temporal perturbations (rewriting "in 1750" into "latest … after 1700") and *manually
  annotated gold evidence* (up to two gold passages per question; annotators had to go
  to Wikipedia by hand for ~12.7% of questions).
- Pipeline: (1) LLM decomposes the question into **main content** and **temporal
  constraint**; (2) retrieve on main content, then summarise each passage into a single
  query-focused sentence to strip temporal distractors; (3) score semantic and temporal
  separately and **multiply**.
- Retrieval, TempRAGEval-TimeQA: Contriever+Gemma baseline 46.7% Answer Recall@1 /
  26.0% Evidence Recall@1 → MRAG with top-5 summarisation **58.6% AR@1 / 37.1% ER@1**.
- End-to-end, Llama3.1-8B-Instruct, TempRAGEval-TimeQA, 5 docs: RAG-Concat 44.0 EM /
  52.8 F1 → MRAG-Concat 49.2 / 59.2 → Self-MRAG **54.2 EM / 65.6 F1**.
- Headline: ~9.3% top-1 evidence recall and ~11% top-1 answer recall over baselines,
  translating to ~4.5% EM/F1 at the answer.

The lesson that transfers: **splitting the query into "what" and "when" before
retrieving is worth more than any decay constant.** Retrieval improves 11 points;
the answer improves 4.5. The decomposition is a prompt, not infrastructure.

**TimelyRAG** — "TimelyRAG: Semantic-Temporal Hybrid Retrieval for Time-Critical
Question Answering in Overlapping-Evolving Documents" (Youngeun Nam, Joeun Kim, Hwanjun
Song, Susik Yoon, Jae-Gil Lee, Byung Suk Lee, arXiv:2609.11572, submitted 2026-09-10,
<https://arxiv.org/abs/2609.11572>). Eight days old at time of writing, and the closest
framing to this repo's actual problem. Their distinction:

- **disjoint-evolving**: each update is an independent snapshot (news). All prior
  time-sensitive retrieval work addresses this.
- **overlapping-evolving**: *"amendments override earlier clauses while preserving most
  content, creating strong semantic overlap across versions"* — laws, policies,
  regulations. **And a tactical playbook where the September answer is the December
  answer with one paragraph replaced.**

They report up to **+28.6% nDCG@10** from incorporating temporal distance into ranking,
retriever-agnostic. Method detail beyond "temporal distance into ranking" is not on the
abstract page `[unverified]`. This is the paper to read in full if we build anything.

**VersionRAG** — "VersionRAG: Version-Aware Retrieval-Augmented Generation for Evolving
Documents" (Daniel Huwiler, Kurt Stockinger, Jonathan Fürst, arXiv:2510.08109,
2025-10-09, <https://arxiv.org/abs/2510.08109>). Hierarchical graph over version
sequences + intent classification routing. On their VersionQA benchmark (100 manually
curated questions over 34 versioned technical documents): **90% accuracy vs 58% naive
RAG and 64% GraphRAG**; implicit change detection 60% vs 0–10% for both baselines; 97%
fewer indexing tokens than GraphRAG. The 0–10% implicit-change number is the striking
one — **baseline RAG essentially cannot tell you that something changed unless the
change is stated in so many words.**

**TempRetriever** — "TempRetriever: Fusion-based Temporal Dense Passage Retrieval for
Time-Sensitive Questions" (arXiv:2502.21024, Feb 2025,
<https://arxiv.org/abs/2502.21024>). Fuses the query date and document date into the
dense encoder rather than post-hoc. Reported to beat `DateAsTag` and `DateAsToken`
baselines consistently `[unverified — I did not fetch the results table]`.

**Graph-shaped variants**, listed for completeness, not read in depth:
T-GRAG, "A Dynamic GraphRAG Framework for Resolving Temporal Conflicts and Redundancy in
Knowledge Retrieval" (arXiv:2508.01680, Aug 2025, <https://arxiv.org/abs/2508.01680>),
contributes Time-LongQA from corporate annual reports; TG-RAG, "RAG Meets Temporal
Graphs" (arXiv:2510.13590, Oct 2025, <https://arxiv.org/abs/2510.13590>), bi-level
temporal graph plus hierarchical time graph, contributes ECT-QA. Both `[unverified]`
beyond abstract.

### Benchmarks, in date order

| Benchmark | Cite | Date | What it tests |
|---|---|---|---|
| TimeQA | arXiv:2108.06314 | 2021, NeurIPS | time-scoped facts from Wikidata + Wikipedia `[unverified]` |
| ArchivalQA | arXiv:2109.03438 | 2021/SIGIR'22 | QA over a news archive `[unverified]` |
| StreamingQA | arXiv:2205.11388 | 2022, ICML | adaptation to a stream of new news `[unverified]` |
| RealTimeQA | arXiv:2207.13332 | 2022/NeurIPS'23 | weekly-refreshed questions about the present `[unverified]` |
| FreshQA / FreshLLMs | arXiv:2310.03214 | 2023-10 | fast-changing and false-premise questions `[unverified]` |
| HoH | arXiv:2503.04800 | 2025-03-03 | **impact of outdated info in the KB on RAG** — see below |
| TempRAGEval | arXiv:2412.15540 | 2024-12 | 1,000 perturbed TimeQA/SituatedQA with gold evidence |
| MAGIC | arXiv:2507.21544 | 2025-07-29 | inter-context conflict detection + localisation |
| FRESCO | arXiv:2604.14227 | 2026-04-14 | reranker bias under evolving semantic conflict |
| TIDE | arXiv:2608.08512 | 2026-08 | temporally evolving customs-law documents `[unverified]` |
| TimelyQABench | arXiv:2609.11572 | 2026-09-10 | overlapping-evolving regulation |

**HoH** — "HoH: A Dynamic Benchmark for Evaluating the Impact of Outdated Information on
Retrieval-Augmented Generation" (Jie Ouyang, Tingyue Pan, Mingyue Cheng, Ruiran Yan,
Yucong Luo, Jiaying Lin, Qi Liu, arXiv:2503.04800, 2025-03-03,
<https://arxiv.org/abs/2503.04800>). Two findings, stated qualitatively in the abstract:
outdated information *"substantially reduces response accuracy by distracting models
from correct information"* and *"can mislead models into generating potentially harmful
outputs."* Exact degradation figures are not on the abstract page `[unverified]`. This is
the benchmark whose premise most closely matches ours — the KB itself is the thing
that's stale.

### Decay shapes actually in use

| System | Formula | Constant | Implied half-life |
|---|---|---|---|
| LangChain `TimeWeightedVectorStoreRetriever` | `semantic_similarity + (1.0 - decay_rate) ** hours_passed` | `decay_rate = 0.01` (default) | ≈ 69 hours ≈ 2.9 days |
| Generative Agents (Park et al. 2023) | `α_rec·recency + α_imp·importance + α_rel·relevance`, recency = exp decay on hours since last access | decay factor **0.995**, all α = 1 | ≈ 138 hours ≈ 5.8 days |
| Elasticsearch `function_score` | gauss / exp / linear decay | `decay = 0.5` at distance `scale` (default) | = `scale`, by construction |
| Azure AI Search `freshness` function | magnitude-style boost over `boostingDuration` | ISO 8601, docs' example `P90D` / `P1095D` | boost window, not a half-life |
| Chronofy | `exp(−β·Δt)` per fact type | β ∈ {0.0, 0.001, 0.05, 0.3, 0.6} | class-dependent; 0.0 = never |

LangChain source and defaults:
<https://python.langchain.com/v0.2/docs/how_to/time_weighted_vectorstore/> and
`langchain.retrievers.time_weighted_retriever` — `param decay_rate: float = 0.01`,
docstring *"The exponential decay factor used as (1.0-decay_rate)\*\*(hrs_passed)."* Note
`hours_passed` is measured since **last accessed**, not since created, so frequently-read
memories stay fresh. The docs' own guidance: decay_rate 0 ⇒ memories never forgotten and
the retriever degenerates to plain vector lookup; decay_rate near 1 ⇒ recency score goes
to 0 for everything and it *also* degenerates to plain vector lookup.

**The LangChain additive form has a pathology worth naming**: `cosine + decay` with both
terms in [0,1] means a one-hour-old irrelevant document can outscore a perfectly relevant
year-old one. Every research system I found that reports numbers — Chronofy, MRAG — uses
**multiplication** instead. Copy the papers, not the framework.

Generative Agents: "Generative Agents: Interactive Simulacra of Human Behavior" (Park,
O'Brien, Cai, Morris, Liang, Bernstein, arXiv:2304.03442, 2023-04-07, rev 2023-08-06,
<https://arxiv.org/abs/2304.03442>). Verbatim from §Retrieval: *"decay factor is 0.995"*,
and *"all αs are set to 1"*. That an unweighted sum of three normalised scores was good
enough for the most-copied memory architecture of 2023 is itself a data point about how
much tuning buys.

Elasticsearch decay functions, verbatim from
<https://www.elastic.co/guide/en/elasticsearch/reference/current/query-dsl-function-score-query.html>:
*"The `decay` parameter defines how documents are scored at the distance given at
`scale`. If no `decay` is defined, documents at the distance `scale` will be scored
0.5."* Gaussian form:
`S(doc) = exp( − max(0, |fieldvalue − origin| − offset)² / 2σ² )`. Their date example:
`"origin": "2013-09-17", "scale": "10d", "offset": "5d", "decay": 0.5` — i.e. **no decay
at all for the first 5 days, then half-life 10 days.** The `offset` parameter is the
useful idea for us: a grace period before age starts counting.

---

## 2. knowledge conflict

### The taxonomy everyone cites

"Knowledge Conflicts for LLMs: A Survey" (Rongwu Xu, Zehan Qi, Zhijiang Guo, Cunxiang
Wang, Hongru Wang, Yue Zhang, Wei Xu, arXiv:2403.08319, v1 2024-03-13, v2 2024-06-22,
EMNLP 2024, <https://arxiv.org/abs/2403.08319>; repo
<https://github.com/pillowsofwind/Knowledge-Conflicts-Survey>). Three categories:

- **context-memory** — retrieved text vs the model's parametric knowledge.
- **inter-context** — two retrieved passages disagree. **This is ours.**
- **intra-memory** — the model contradicts itself.

### How well can a system detect that two passages disagree? Badly.

**MAGIC** — "MAGIC: A Multi-Hop and Graph-Based Benchmark for Inter-Context Conflicts in
Retrieval-Augmented Generation" (Jungyeon Lee, Kangmin Lee, Taeuk Kim, arXiv:2507.21544,
v1 2025-07-29, v3 2025-10-09, EMNLP 2025 Findings,
<https://arxiv.org/abs/2507.21544>). Two tasks: **ID** (is there a conflict?) and **LOC**
(which spans conflict?).

| Model | ID % | LOC % |
|---|---|---|
| GPT-4o-mini | **83.61** | **55.00** |
| Llama 3.1 70B | 72.86 | 37.92 |
| o1 | 68.06 | 49.72 |
| Claude 3.5 Haiku | 60.28 | 42.50 |
| Mixtral 8x7B | 37.92 | 17.40 |
| **Human** | **92.5** | **83.3** |

Single-hop conflicts: 62.99% LOC average. Multi-hop: **32.67%**. Their stated failure
mode: *"models struggle more to identify conflicts in our dataset, and even when they do,
they have difficulty pinpointing the exact portions where the conflict occurs"*, and o1
*"often takes a conservative stance, frequently predicting no conflict in ambiguous
cases."*

**This is the number that should decide our architecture.** If we ask the assistant to
notice at query time that a September claim contradicts a January one, we are buying a
coin-flip on localisation and a system that silently says "no conflict" when it is
unsure. If a human marks the supersession at write time, we are buying a read.

### What systems do once they detect one

**RAMDocs / MADAM-RAG** — "Retrieval-Augmented Generation with Conflicting Evidence"
(Han Wang, Archiki Prasad, Elias Stengel-Eskin, Mohit Bansal, arXiv:2504.13079,
2025-04-17, rev 2025-08-12, COLM 2025, <https://arxiv.org/abs/2504.13079>). The important
conceptual contribution is that **"conflict" is not one thing and the right response
differs per kind**:

- *ambiguity* → present multiple valid answers,
- *misinformation* → suppress,
- *noise* → filter.

Method: LLM agents debate the merits of an answer over multiple rounds, then an
aggregator *"collate[s] responses corresponding to disambiguated entities while
discarding misinformation and noise."* Numbers: Llama3.3-70B-Instruct gets only **32.60
exact match** on RAMDocs; MADAM-RAG improves AmbigDocs by up to **11.40%** and FaithEval
by up to **15.80%** absolute. RAMDocs also varies the *ratio* of documents supporting each
answer, which is the direct analogue of "four members on a call vs one teacher lesson".

**EvoTrustRAG** — "EvoTrustRAG: Evolution-Aware Conflict Attribution and Evidence
Handling for Reliable Retrieval-Augmented Generation" (Xi Nie, Hongwei Li, Shenghao Wu,
Wenshu Fan, Qiyang Song, Wenbo Jiang, arXiv:2608.07933, 2026-08-08,
<https://arxiv.org/abs/2608.07933>). The most on-point 2026 work for us: it reframes the
task from *"pick the trustworthy source"* to **conflict origin attribution**, and
distinguishes exactly the three cases we care about:

1. **knowledge evolution** — legitimate temporal progression (Google changed the rules),
2. **malicious manipulation** — intervention-like support patterns,
3. **unresolved uncertainty** — cannot be attributed; *remains visible to the generator*.

Training-free; builds a conflict evidence graph and evaluates hypotheses over temporal
relations, support structure and consistency. Numbers: **81.4%** average accuracy on
benchmark-native conflict settings; attribution macro-F1 **72.2% → 79.1%** over the
strongest baseline; error rate under coordinated attack **31.2% → 16.0%**. Dataset names
are not in the abstract `[unverified]`.

The case-3 behaviour — *keep the unresolved conflict visible rather than picking* — is
exactly what we want the assistant to do, and it is the first paper I found that names it
as a first-class output rather than a failure.

**ConflictRAG** — "ConflictRAG: Detecting and Resolving Knowledge Conflicts in Retrieval
Augmented Generation" (Chenyu Wang, Yueyuan Li, Yingmin Liu, Yang Shu, arXiv:2605.17301,
v1 2026-05-17, v2 2026-06-08, submitted to IEEE SMC 2026,
<https://arxiv.org/abs/2605.17301>). Opens by naming the assumption everything else
makes: *"RAG systems implicitly assume mutual consistency among retrieved documents — an
assumption that frequently fails in practice."* Three inter-document conflict types:
**factual, temporal, opinion**. Two-stage detector: a cheap embedding-based MLP
classifier plus selective LLM refinement — **62% reduction in API costs at 90.8%
detection accuracy**, 88.7% conflict-detection F1. Credibility assessed by
**Entropy-TOPSIS**, which they report beats manual weighting by **7.1%** accuracy. Overall
correctness gain 5.3–6.1% over baselines.

That 7.1% is the honest tax on hand-set authority weights. It is small.

**Abstention is separately hard.** "Prompt-Based Abstention Fails Under Misleading
Context: A Controlled Study of Small Frozen RAG Models" / GRAB-RAG (arXiv:2608.22228,
Aug 2026, <https://arxiv.org/abs/2608.22228>): under explicit abstention prompting, small
frozen models **still answer 41.6% of misleading questions, and 63% of those answers echo
the planted wrong entity verbatim**. The design implication the authors draw is to
separate *no-evidence* from *conflicting-evidence* as distinct verdicts rather than one
"abstain" signal. `[abstract-level; I did not fetch the full tables]`

Also in this cluster, read-if-building: "Trust or Abstain? A Self-Aware RAG Approach"
(arXiv:2605.18792, 2026, <https://arxiv.org/abs/2605.18792>) builds a model-specific
knowledge-conflict benchmark of ~69K query–context instances per backbone over five
conflict-QA datasets; an Evidence Sufficiency Benchmark grades evidence L1 (full support)
→ L5 (conflicting) and treats L3–L5 as abstain-required
(<https://www.techscience.com/cmc/v89n1/68467/html>); "When Evidence Conflicts:
Uncertainty and Order Effects in Retrieval-Augmented Biomedical Question Answering"
(arXiv:2605.14115) finds the *same* documents in a *different order* produce different
answers and different confidence. All `[unverified]` beyond abstract.

---

## 3. source authority / trust weighting

### The finding that matters most for this corpus

"Whose Facts Win? LLM Source Preferences under Knowledge Conflicts" (Jakob Schuster,
Vagrant Gautam, Katja Markert, arXiv:2601.03746, 2026-01-07,
<https://arxiv.org/abs/2601.03746>). 13 open-weight LLMs, synthetic sources to avoid
real-world name bias.

- LLMs **do** prefer institutionally-corroborated information (government, newspaper)
  over people and social media.
- *"These preferences can be reversed by simply repeating information from less credible
  sources."*
- Their mitigation reduces repetition bias by up to **79.2%** while retaining at least
  **72.5%** of the original preferences.

Read against our corpus: the teacher's course lessons are the "institutional" source and
the group call is "people". The model will default to the right preference — and lose it
the moment the call transcript repeats a claim three times, which transcripts do
constantly. **Authority must be a scored multiplier applied outside the model, not a
hope about the model's priors.**

### Recency vs authority, head to head

"LLMs Followed the Newer Timestamp Over the Reliability Flag" (Pebblous, 2026-08-20;
arXiv:2608.20116, <https://blog.pebblous.ai/blog/llm-evidence-arbitration-recency/en/>).
Seven open-weight instruction-tuned models (four Qwen3 1.7B–14B, Gemma-2-9B-It,
Llama-3-8B-Instruct, Mistral-7B-Instruct-v0.3); 2,000 synthetic instances per condition,
three seeds; binary forecasting over 16-step series. Cues pitted against each other:
text-vs-numbers, recent-vs-older timestamp, unflagged-vs-corrupted, context-vs-tool.

Result: **the timestamp cue beat the explicit corruption flag.** Recency produced *"much
stronger and more consistent arbitration behaviour"*; reliability *"produced larger
performance drops than the recency condition across most models"* — i.e. models followed
the newer source even when the text said the data was corrupted. Reported comparatively;
exact magnitudes are not given numerically `[unverified]`.

**Critical methodological caveat they raise and we must copy:** a *prompt* recency effect
(later in the context) is confounded with *temporal* recency (newer timestamp). They ran
each condition twice with placement flipped and found accuracy higher with the correct
source last in nearly every model; for Qwen3-4B, conflict accuracy *"moved by 0.20 on
average"* when answer labels changed, vs 0.01 for text-only. **If we put the newest claim
last in the prompt, we cannot tell whether the model used the date or the position.** The
free lesson: order retrieved claims oldest→newest so date and position agree, and stop
pretending the model read the date.

### Recency bias is already in the reranker, unasked

"Do Large Language Models Favor Recent Content? A Study on Recency Bias in LLM-Based
Reranking" (Hanpei Fang, Sijie Tao, Nuo Chen, Kai-Xin Chang, Tetsuya Sakai,
arXiv:2509.11353, 2025-09-14, <https://arxiv.org/abs/2509.11353>). TREC DL 2021/2022.
Seven models (GPT-3.5-turbo, GPT-4, GPT-4o, LLaMA-3 8B/70B, Qwen-2.5 7B/72B), all
systematically promote newer-dated passages:

- top-10 mean publication year pushed forward by up to **4.78 years**
- individual items moved by up to **95 ranks**
- a bare date tag reverses up to **25%** of pairwise preferences between
  equally-relevant passages

Larger models attenuate but do not remove it. Their stated risk: rerankers undervalue
authoritative older material.

**And the exact opposite, when recency is not tagged.** FRESCO — "FRESCO: Benchmarking
and Optimizing Re-rankers for Evolving Semantic Conflict in Retrieval-Augmented
Generation" (Sohyun An, Hayeon Lee, Shuibenyang Yuan, Chun-cheng Jason Chen, Cho-Jui
Hsieh, Vijai Mohan, Alexander Min, arXiv:2604.14227, 2026-04-14,
<https://arxiv.org/abs/2604.14227>). Built from Wikipedia revision histories paired with
recency-seeking queries. Finding: existing rerankers show *"a strong bias toward older,
semantically rich documents, even when they are factually obsolete."* Their fix is
instruction optimisation over a Pareto frontier between evolving and non-evolving tasks:
**up to +27% on Evolving Knowledge tasks** while holding non-evolving performance.

Reconciling the two: **an explicit date tag makes models over-prefer new; an implicit,
inferred-from-content date makes them over-prefer old and detailed.** Both failures are
avoided the same way — score the date arithmetically outside the model and hand the model
a ranked list, rather than asking it to weigh dates itself.

### The published source-authority priors

**This is the closest published thing to the number we need.** "Source-Aware Reranking for
Retrieval-Augmented Generation: A Reliability Prior Approach" (Yuktha Tata Koganti, Hugo
Garrido-Lestache Belinchon, arXiv:2607.22584, <https://arxiv.org/abs/2607.22584>; the
listing page says "v1, 08 Jun 2026" against a `2607` July-2026 ID prefix — **the
discrepancy is unresolved** `[unverified]`).

Formula, verbatim: **`score_SA(q,d) = sim(q,d) × λ(s_d)`** — a multiplicative source prior,
same shape as Chronofy's `q_e`. Their Table 1 priors (health domain):

| source type | λ |
|---|---|
| peer_reviewed | 0.95 |
| government | 0.90 |
| established_news | 0.75 |
| organization_report | 0.70 |
| blog | 0.50 |
| **ai_generated** | **0.30** |
| user_generated | 0.40 |
| unknown | 0.35 |

Reported Precision@5 **0.48 → 0.72** vs similarity-only. The authors' own disclaimer,
verbatim: these *"represent domain-informed assumptions … are not learned from data and
are not claimed to be optimal."* Small single-domain evaluation — treat as existence proof
of the *shape*, and as the only published number for "how much do you discount
AI-generated text" (0.30, i.e. a 3.2× discount against a peer-reviewed source).

**Reliability estimated instead of assumed** — RA-RAG, "Retrieval-Augmented Generation
with Estimation of Source Reliability" (Hwang, Park, Park, Kim, Park, Ok,
arXiv:2410.22954, 2024-10-30, v5 2025-10-14, <https://arxiv.org/abs/2410.22954>). Three
stages: estimate reliability by cross-checking sources against each other, selectively
retrieve from the top-κ sources balancing reliability against relevance, aggregate by
weighted majority vote. Their framing of the gap: standard RAG *"relies solely on
relevance between a query and a document, overlooking the heterogeneous reliability of
these sources."* No closed-form blend in the abstract `[unverified]`.

Two papers inject credibility at *generation* rather than retrieval, which is the relevant
counterfactual to reranking: **CrAM** (arXiv:2406.11497, 2024) modifies attention heads by
a credibility score defined as P(document contains no misinformation); **CAG**, "Not All
Contexts Are Equal: Teaching LLMs Credibility-aware Generation" (Pan et al.,
arXiv:2404.06809, 2024-04-10, v3 2024-10-09, EMNLP 2024 Main,
<https://arxiv.org/abs/2404.06809>) trains the model to consume credibility annotations
and *"supports customized credibility."* **CrediRAG** (Ram et al., arXiv:2410.12061, Oct
2024, The Web Conference '25) scores a Reddit post by the average source credibility of
retrieved similar news articles, then corrects over a post-to-post graph weighted by
commenter stance: +11% F1 over SOTA on >200k posts. Note its credibility values are
external per-outlet assumptions, not derived from the corpus.

Survey: "Trustworthiness in Retrieval-Augmented Generation Systems: A Survey"
(arXiv:2409.10102, Sep 2024, <https://arxiv.org/abs/2409.10102>) — six dimensions:
factuality, robustness, fairness, transparency, accountability, privacy. Reading list at
<https://github.com/Arstanley/Awesome-Trustworthy-RAG>.

### Trust propagation classics, and why they don't apply here

| Algorithm | Citation | Date | URL |
|---|---|---|---|
| PageRank | Page, Brin, Motwani, Winograd, Stanford TR 1999-66 | 1998/99 | <http://ilpubs.stanford.edu:8090/422/> |
| EigenTrust | Kamvar, Schlosser, Garcia-Molina, WWW'03 pp. 640–651 | 2003-05 | <https://nlp.stanford.edu/pubs/eigentrust.pdf> |
| TrustRank | Gyöngyi, Garcia-Molina, Pedersen, VLDB'04 pp. 576–587 | 2004-08-31 | <https://www.vldb.org/conf/2004/RS15P3.PDF> |
| Anti-TrustRank | Krishnan & Raj, AIRWeb'06 pp. 37–40 | 2006-08-10 | <https://airweb.cse.lehigh.edu/2006/krishnan.pdf> |

TrustRank's mechanics are worth knowing because the *shape* matches our situation even
though the algorithm doesn't: a human-labelled seed set of **fewer than 200 sites** was
enough to filter spam across a significant fraction of AltaVista's index, propagated over
20 power iterations with decay 0.85. The principle — **trust is expensive to assign and
cheap to propagate** — maps onto `structured/approved/` being a small human-promoted seed.
Anti-TrustRank inverts edge direction to propagate distrust backwards from bad seeds.
EigenTrust's "pre-trusted peers" seed is structurally the same device.

**But: I found no work applying TrustRank, Anti-TrustRank or EigenTrust inside a RAG
corpus.** They need a link graph. This repo has no citation graph — its links point
outward to Loom and Skool, not between bullets. The only crossover is PageRank, and mostly
as *Personalized* PageRank used for graph traversal (relevance), not authority: HippoRAG
(arXiv:2405.14831, NeurIPS 2024), MixPR (arXiv:2412.06078, Dec 2024), NodeRAG
(arXiv:2504.11544, Apr 2025).

The one exception is **RAGRank** — "RAGRank: Using PageRank to Counter Poisoning in CTI
LLM Pipelines" (Austin Jia, Avaneesh Ramesh, Zain Shamsi, Daniel Zhang, Alex Liu,
arXiv:2510.20768, v1 2025-10-23, v2 2025-12-15, <https://arxiv.org/abs/2510.20768>).
Verbatim formulas, including a recency term that composes with authority exactly the way
we want:

```
two-pass retrieve-then-authority:  top-k_{d∈D}[ R(d) · 1{d ∈ R_2k} ],  R_2k = top-2k_{d∈D}[ sim(q,d) ]
time decay:                        τ(dᵢ) = 1                          if t(dᵢ) ≤ r
                                          max(0, 1 − λ·(t(dᵢ) − r))   if t(dᵢ) > r
applied as:                        α′(dᵢ) = τ(dᵢ) · α(dᵢ)
author credibility:                C(x) = (1/|D_x|) Σ_{d∈D_x} α′(d),   γ(dᵢ) = Σ_{x∈A(dᵢ)} C(x)
```

Note `τ` is **linear decay with a grace period `r`**, not exponential — the same
grace-then-decay shape as Elasticsearch's `offset`. Their own stated limitation: *"databases
in practical enterprise systems often do not contain explicit citation links."* Which is
our situation too.

### Combining several factors into one score

**Reciprocal Rank Fusion** — Cormack, Clarke, Büttcher, "Reciprocal rank fusion outperforms
condorcet and individual rank learning methods", SIGIR '09 pp. 758–759, DOI
10.1145/1571941.1572114, <https://cormack.uwaterloo.ca/cormacksigir09-rrf.pdf>.

```
RRFscore(d) = Σ_{r∈R} 1/(k + r(d))
```

The paper's own rationale: reciprocal rather than exponential decay so *"the contribution
of lower-ranked documents does not vanish"*, and k *"mitigates the impact of high outlier
rankings."* **k = 60 was used without tuning** in the original experiments — it is a
two-page poster, not a k-sweep. Everything downstream inherited the number by imitation:
Weaviate's `rankedFusion` computes `1/(RANK + 60)`
(<https://weaviate.io/blog/hybrid-search-fusion-algorithms>) while **Qdrant's RRF uses
k = 2** with zero-based ranks (<https://qdrant.tech/documentation/concepts/hybrid-queries/>).
The live disagreement is the proof that k was never load-bearing.

Worth noting for our purposes: the original RRF paper already beat individual *rank-learning*
methods on LETOR 3. It is evidence for "unsupervised fusion ≥ learned combination" in its
own right.

**Rank fusion vs weighted sum.** Weaviate ships both and switched its default, which makes
it the cleanest published side-by-side: `rankedFusion` (rank-based) vs
`relativeScoreFusion` (min-max normalise each list to [0,1], then weighted sum with alpha)
— default since **v1.24**. Their stated reason for switching: relativeScoreFusion *"retains
more information from the original searches than rankedFusion, which only retains
rankings."* Qdrant offers a third, DBSF: per-list z-normalisation using 3-sigma extremes,
`ŝ = (s − (μ − 3σ)) / 6σ`, then sum.

**Does a hand-tuned linear (relevance, recency, authority) beat a learned reranker at
n≈300?** No published head-to-head on that exact triple `[unverified — a genuine gap]`.
But there are two results that settle it in practice:

1. **The annotation threshold.** Mokrii, Boytsov, Braslavski, "A Systematic Evaluation of
   Transfer Learning and Pseudo-labeling with BERT-based Ranking Models"
   (arXiv:2103.03335, v1 2021-03-04, SIGIR'21, <https://arxiv.org/abs/2103.03335>).
   Verbatim: outperforming BM25 *"requires over **100 annotated queries** on DPR data,"
   "at least **1–2K annotated queries** on MS MARCO data," "more than **8K annotated
   queries** on Yahoo! Answers."* Annotation cost: *"a single document-pair takes at least
   one minute on average,"* and *"a single query typically needs at least 50 of such
   judgements"* — over an hour per query. Below those thresholds the unsupervised baseline
   wins, and few-shot training actively degrades a transferred model via overfitting.
   **We would need >100 judged queries at ~1 hour each before a learned reranker was worth
   considering. That settles it.**
2. **The functional-form result.** In Microsoft's LambdaBM25F work: *"at each truncation
   level, for each content field, BM25F significantly outperformed the learned linear
   function"*, concluding *"BM25F is significantly better than a linear combination of
   input attributes"* (CIKM 2009,
   <https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/LearningBM25MSRTechReport.pdf>).
   The practical reading: **if you go linear, the saturation and normalisation of each
   input matters more than the weights.** Which is an argument for getting the decay shape
   right and then not agonising over the multipliers.

Supporting this from the conflict literature: ConflictRAG's Entropy-TOPSIS credibility
weights beat manual ones by only **7.1%** (arXiv:2605.17301, 2026), and Generative Agents
shipped α = 1 on all three of its terms.

### The one paper that scores exactly recency × authority × specificity

**REBEL** — "Relevance Isn't All You Need: Scaling RAG Systems With Inference-Time Compute
Via Multi-Criteria Reranking" (Will LeVine, Bijan Varjavand, arXiv:2504.07104, 2025-03-14,
<https://arxiv.org/abs/2504.07104>). Its five fixed secondary criteria are literally our
triple plus two: **Depth of Content, Diversity of Perspectives, Clarity and Specificity,
Authoritativeness, Recency.**

```
Final Score = Relevance + Σᵢ wᵢ × (Propertyᵢ)
```

All `wᵢ = 0.5` in their experiments. Relevance is scored 0–10; each secondary criterion
0–5. **So any single secondary criterion can move a document by at most 2.5 against a
10-point relevance range — a 25% ceiling per criterion.** That ratio is the most useful
transferable number in the section: it is one published team's answer to "how much should
authority be allowed to override relevance", and the answer is *a quarter, at most*.

Caveat: REBEL's criteria are scored by an LLM via chain-of-thought, not computed from
metadata. Our dates and source classes are metadata, so we can compute them exactly and
cheaply — which is strictly better.

### Production primitives that ship, with their real defaults

**Vespa** is the only vendor whose documented rank expression contains all three terms, and
its defaults are directly reusable (<https://docs.vespa.ai/en/reference/ranking/rank-features.html>,
<https://docs.vespa.ai/en/ranking/nativerank.html>):

- `freshness(name)` is built-in and normalised 0–1, defined verbatim as
  *"A number which is close to 1 if the timestamp in attribute `name` is recent compared to
  the current time compared to maxAge: `max( 1-age(name)/maxAge , 0 )`"*
  (<https://docs.vespa.ai/en/reference/ranking/rank-features.html>). It is **linear**, and
  it hits exactly zero at `maxAge`.
- **`freshness.maxAge` defaults to `3*30*24*60*60` seconds — the docs' own gloss is
  *"about 3 months"***; described as *"The maximal age in seconds when calculating
  `freshness(name)`. Ages older than this will not have any effect on this feature."*
  Verified at <https://docs.vespa.ai/en/reference/rank-feature-configuration.html>.
  Because the decay is linear to zero at maxAge, **the half-value point of Vespa's default
  is 45 days, not 90.**
- The logscale variant has a separate knob: **`halfResponse` defaults to `7*24*60*60` = 7
  days**, *"the age which gives an output of 0.5"* — i.e. Vespa ships a **7-day half-life**
  as its logarithmic default, an order of magnitude more aggressive than the linear one.
  The two defaults disagreeing inside one product is a fair indication of how little
  consensus exists on decay constants.
- Canonical combined first-phase, verbatim:
  `first-phase { expression { attribute(quality) * freshness(timestamp) + bm25(title) } }`
  — **multiplicative on authority × recency, additive on relevance.**
- Their blog-search profile replaces the linear built-in with exponential decay:
  ```
  function freshness() { expression: exp(-1 * age(timestamp)/(3600*12)) }
  first-phase { expression: (query(textMatchWeight) * (nativeRank(title,body)
                 + query(qualityWeight) * quality
                 + query(deservesFreshness) * freshness)) / normalization }
  ```
  **The parameter is literally named `deservesFreshness`** — Vespa exposes QDF as a
  per-query knob. This is the closest runnable QDF in public. Their stated rationale: the
  age/freshness relationship is non-linear and sensitivity should be higher for very fresh
  documents.

**Elasticsearch / OpenSearch** decay derivations, which are the same maths our formula
needs (<https://docs.opensearch.org/latest/query-dsl/compound/function-score/>):
`σ² = −scale²/(2·ln(decay))`; exponential `λ = ln(decay)/scale`; linear `s = scale/(1−decay)`.
`decay` defaults to 0.5, `offset` to 0. Linear hits exactly 0 past `2×scale`; gauss and exp
never reach 0. Elastic is steering off `function_score` toward `script_score`, where decay
on dates **cannot use `now`** — the timestamp must be passed in as a script param.
`rank_feature` (static boosts) and `distance_feature` (date/geo proximity) skip
non-competitive hits and are much cheaper. Operational trap: decay is computed at query
time against `now()`, so **an identical document scores differently each day** — any
cached or precomputed score needs explicit date maths.

**Qdrant** added score boosting in v1.14.0 (<https://qdrant.tech/blog/decay-functions/>,
2025-09-01): `final_score = $score + boost_1 + … − penalty_1 − …` with `exp_decay` /
`gauss_decay` / `lin_decay` over `(x, target, midpoint, scale)`. Verbatim recency example:

```json
{"exp_decay": {"x": {"datetime_key": "upload_time"},
               "target": {"datetime": "2025-08-04T00:00:00Z"},
               "scale": 604800, "midpoint": 0.1}}
```

**Qdrant's own documented warning is the most practical thing in this section**: a formula
runs only as a rescoring step over prefetches, and on a hybrid query *a decay score can
overwhelm RRF* — with two agreeing retrievers the top RRF score is 1.0, and that ceiling
rises with each extra prefetch, so the decay coefficient must be calibrated against the
observed fused-score range. **Measure your score range before you pick a decay constant.**

**Weaviate** has no native decay function in hybrid search; recency must be pre-normalised
into a property or applied at rerank time. **Pinecone**: I found no native decay/boost
scoring primitive documented, but this was not directly verified `[unverified — do not
assert absence]`.

### Query Deserves Freshness — documented vs folklore

**The origin is weaker than its fame.** "QDF" traces to Amit Singhal, quoted in a 2007 *New
York Times* piece by Saul Hansell. NYT blocks automated fetch, so the exact wording and
date are `[unverified from primary source]`. Secondary paraphrase, Search Engine Journal,
page dated 2022-06-29
(<https://www.searchenginejournal.com/google-algorithm-history/freshness-algorithm/>):
*"Mr. Singhal introduced the freshness problem, explaining that simply changing formulas to
display more new pages results in lower-quality searches much of the time. He then unveiled
his team's solution: a mathematical model that tries to determine when users want new
information and when they don't."*

- **Documented:** that a model exists which detects "hot" topics from news/blog publication
  volume plus Google's own query stream.
- **Folklore:** any specific QDF score, threshold, formula or decay curve, or the claim
  that "QDF" names a live component today. Google has never published a QDF spec. Search
  Engine Land's QDF guide returns 403 to automated fetch `[unverified]`.
- SEJ also draws a distinction worth keeping: QDF (2007) ≠ the Freshness Update (2011).
  Caffeine, which made the 2011 update possible, didn't exist in 2007.

**The 2011 Freshness Update is first-party.** "Giving you fresher, more recent search
results", Google Search blog, **2011-11-03**, by Amit Singhal
(<https://search.googleblog.com/2011/11/giving-you-fresher-more-recent-search.html>).
Claim: *"a significant improvement to our ranking algorithm that impacts roughly 35 percent
of searches."* Three query classes named: recent events / hot topics; regularly recurring
events (earnings, NFL scores); frequently-updated topics ("best SLR cameras").

**Correction to the folklore worth carrying:** 35% is searches *touched*, only ~6–10%
*visibly changed* (Search Engine Watch 2011-11-04,
<https://searchenginewatch.com/2011/11/04/google-gets-fresh-with-algorithm-update-affecting-35-of-searches/>).
That 6–10% is a fair prior for what a freshness term buys on a general query mix — and a
reminder that our corpus is nothing like a general query mix, because almost every query
here is a freshness query.

**The 2024 Google Content Warehouse leak.** Provenance: internal API docs committed to a
public Google GitHub repo by `yoshi-code-bot` ~2024-03-13, removed 2024-05-07; ~2,500 pages
/ 14,014 attributes; published by Rand Fishkin 2024-05-27
(<https://sparktoro.com/blog/an-anonymous-source-shared-thousands-of-leaked-google-search-api-documents-with-me-everyone-in-seo-should-see-them/>);
Google confirmed authenticity while warning against *"inaccurate assumptions … based on
out-of-context, outdated, or incomplete information."* The docs were auto-published as an
Elixir client and remain browsable field-by-field on HexDocs, which is the nearest thing to
a primary source. Verbatim:

- `PerDocData.semanticDate` — *"estimated date of the content of a document based on the
  contents of the document (via parsing), anchors and related documents."*
- `PerDocData.lastSignificantUpdate` — *"Last significant update of the document … as
  computed by the LSUSelector from various signals."*
- `PerDocData.datesInfo` — *"Stores dates-related info (e.g. page is old based on its date
  annotations). Used in **FreshnessTwiddler**."* — the closest thing in the leak to a named
  freshness re-ranking stage.
- `PerDocData.freshboxArticleScores` — *"scores of freshness-related classifiers."*
- `bylineDate` and `syntacticDate` are **not** in `PerDocData` — they live in
  `NlpSaftDocument`. `bylineDate` = *"the date shown in snippets in web search results."*
  `syntacticDate` = *"the document's syntactic date (e.g. a date explicitly mentioned in
  the URL or title)."* `QualityTimebasedSyntacticDate` carries a field literally named
  **`trustSyntacticDateInRanking`**.
- `CompressedQualitySignals.siteAuthority` — *"site_authority: converted from
  quality_nsr.SiteAuthority, applied in Qstar."* That one line is the entire basis for both
  the `siteAuthority` and the `Q*` claims. Neighbours: `unauthoritativeScore`,
  `authorityPromotion`, `contentEffort` (an LLM-based effort estimate for article pages).

Sources: <https://google-api-content-warehouse.hexdocs.pm/0.4.0/GoogleApi.ContentWarehouse.V1.Model.PerDocData.html>,
<https://hexdocs.pm/google_api_content_warehouse/0.4.0/GoogleApi.ContentWarehouse.V1.Model.NlpSaftDocument.html>,
<https://google-api-content-warehouse.hexdocs.pm/0.4.0/GoogleApi.ContentWarehouse.V1.Model.CompressedQualitySignals.html>.

| Claim | Status |
|---|---|
| Those field names exist in Google's internal API docs | **Confirmed** — readable verbatim on HexDocs; Google confirmed authenticity |
| `siteAuthority` is "applied in Qstar" | **Confirmed as a documentation string.** What Q* is, or its weight, is not in the docs |
| The three date fields are cross-referenced to raise freshness confidence | **Speculation** — a reasonable SEO inference (iPullRank 2024-05-28), not stated in the docs |
| NavBoost / Glue exist; NavBoost re-ranks on click data over a 13-month window | **Confirmed by sworn DOJ testimony** — Pandu Nayak, *US v. Google*, late 2023 |
| Q* confirmed in DOJ testimony or exhibits | **`[unverified]`** — asserted in one secondary source; not located in trial records. Treat Q* as leak-only |
| Any weights, formulas, or whether these fields are live | **Not in the leak at all.** The docs describe stored data, not scoring functions |

The idea that actually transfers, and is worth the whole section: **freshness should be
conditional on the query, not unconditional on the corpus.** Chronofy's −15.3% is the price
of unconditional freshness. Vespa's `query(deservesFreshness)` is the shipped form of the
fix. A one-line query classifier is the cheapest high-value component in this document.

### Azure AI Search — the clearest vendor statement of the rule

**The `freshness` scoring function** —
<https://learn.microsoft.com/en-us/azure/search/index-add-scoring-profiles>. Schema:

```json
"functions": [
  {
    "type": "magnitude | freshness | distance | tag",
    "boost": 10,
    "fieldName": "date",
    "interpolation": "constant | linear | quadratic | logarithmic",
    "freshness": { "boostingDuration": "P1095D" }
  }
]
```

`boostingDuration` is ISO 8601; negative durations handle future dates. Constraints: the
field must be `filterable` and a `DateTimeOffset`. `functionAggregation` (default `sum`)
is how multiple functions combine — **so Azure's own primitive for recency × authority is
an additive boost stack, not a product.**

**Azure freshness-aware agentic retrieval (preview)** —
<https://learn.microsoft.com/en-us/azure/search/agentic-retrieval-how-to-configure-freshness>,
`ms.date: 2026-06-02`, page updated 2026-09-17, REST api-version `2026-08-01-preview`.
The clearest statement of practitioner consensus I found anywhere, verbatim:

> *"Freshness is a ranking bias, not a hard filter. Older documents can still appear when
> they're strongly relevant to the query."*

> *"Enable freshness-aware retrieval when newer content is generally more useful or
> trustworthy than older content. Common examples include release notes, policy updates,
> runbooks, service advisories, and operational guidance."*

> *"Don't use freshness as a replacement for filtering. If a query must only return
> content from a specific date range, use a filter in the retrieve request or knowledge
> source configuration instead."*

Their worked example uses `boostingDuration: "P90D"`. Two operational details worth
stealing: the freshness field is **generated at ingestion time** (so it applies only to
content ingested after the policy exists), and their debugging advice is *"inspect the
`last_modified` field in the underlying index. Missing, stale, or inconsistent date values
reduce the quality of the freshness signal."* Which is a polite way of saying **the dates
are the hard part**, and is why this repo's invariant that every claim carries its date is
worth more than any ranker.

Note also that **Azure's `functionAggregation` defaults to `sum`** — so Microsoft's own
primitive for recency × authority is an additive boost stack, while Vespa's canonical
expression multiplies them and Chronofy/MRAG/Koganti all multiply. **No consensus on
additive vs multiplicative.** The argument for multiplying is that it cannot let a fresh
irrelevant item outrank a stale perfect one; the argument for adding is that it cannot let
one weak signal zero out the score. Multiplication with a floor gets both.

---

## 4. bitemporal knowledge graphs / memory

### The vocabulary

Bitemporal modelling is old and settled. **Valid time** = when the fact was true in the
world. **Transaction time** = when the database learned it. Formalised by Richard
Snodgrass and colleagues through the 1990s (TSQL2); standardised as system-versioned and
application-time-period tables in **SQL:2011**; implemented as first-class bitemporality
in **XTDB** and as `as-of` / `as-at` queries in **Datomic**.
`[unverified — cited from memory; primary sources not fetched in this pass.]`

The reason it matters here: this corpus has *both* clocks and they diverge. A claim has a
**said-on** date (when the person said it on the call) and a **valid-until** date (when
Google changed the rule out from under it) and the second is almost never observed
directly — it is inferred later, when someone says "it just stopped working last week".
Every system below models the first clock well and the second poorly.

### Zep / Graphiti

"Zep: A Temporal Knowledge Graph Architecture for Agent Memory" (arXiv:2501.13956,
2025-01, <https://arxiv.org/abs/2501.13956>); implementation at
<https://github.com/getzep/graphiti>. This is the closest *published* analogue to what we
want.

The model as documented: each edge carries **four timestamps** — `created_at` and
`expired_at` (transaction time: when Graphiti learned the fact and when it learned the
fact was wrong) and `valid_at` / `invalid_at` (valid time: when the fact became and
stopped being true in the world). Episodes are the raw ingested units; entities and edges
are extracted from them and keep a pointer back.

**How an edge actually gets invalidated — read from source on `main`, 2026-09-18**
(`graphiti_core/utils/maintenance/edge_operations.py`,
`graphiti_core/prompts/dedupe_edges.py`). It is **two stages, and only the first uses an
LLM**. Note there is no longer an `invalidate_edges.py` prompt or a `temporal_operations.py`
module — the names in the 2025 paper have moved, so cite the code, not the paper.

*Stage 1 — nominate candidates (LLM).* The `dedupe_edges.resolve_edge` prompt, response
model `EdgeDuplicate`, returns `contradicted_facts: list[int]` — *"List of idx values of
contradicted facts (from full idx range). Empty list if none."* The prompt is explicit that
supersession and duplication are not exclusive, verbatim:

> *"A fact from EXISTING FACTS can be both a duplicate AND contradicted (e.g., semantically
> the same but the new fact updates/supersedes it)."*

and its own worked examples separate the three cases: identical information →
`duplicate_facts=[0], contradicted_facts=[]`; *"same relationship but updated title —
contradiction, NOT a duplicate"* → `contradicted_facts=[1]`; *"different events on different
days"* → both empty. **That middle case is precisely our problem: the same claim with a new
value.**

*Stage 2 — decide invalidation (no LLM, pure interval arithmetic).*
`resolve_edge_contradictions()`, verbatim logic:

```python
# (Edge invalid before new edge becomes valid) or (new edge invalid before edge becomes valid)
if (edge.invalid_at is not None and resolved_edge.valid_at is not None
        and edge.invalid_at <= resolved_edge.valid_at) \
   or (edge.valid_at is not None and resolved_edge.invalid_at is not None
        and resolved_edge.invalid_at <= edge.valid_at):
    continue                                   # intervals don't overlap → no invalidation
# New edge invalidates edge
elif (edge.valid_at is not None and resolved_edge.valid_at is not None
        and edge.valid_at < resolved_edge.valid_at):
    edge.invalid_at = resolved_edge.valid_at   # valid time closes at the new fact's start
    edge.expired_at = edge.expired_at if edge.expired_at is not None else utc_now()
    invalidated_edges.append(edge)
```

Three things to take from this:

1. **Newest-wins is decided by `valid_at` comparison, not by an LLM judgement.** The LLM
   only says "these two are about the same thing"; the clock decides which one dies.
2. **Non-overlapping validity intervals are never treated as a contradiction** — two claims
   about different periods coexist. This is the machine-readable version of "don't confuse
   *changed* with *contradicted*".
3. **The old edge's valid interval is closed at exactly the moment the new one opened**, and
   its transaction-time `expired_at` is stamped separately. Both clocks, both recorded,
   nothing deleted.

`_extract_edge_timestamps()` fills `valid_at` / `invalid_at` by a *"lightweight LLM call"*
against the episode's reference time, and *"Skips if the edge already has timestamps set."*

Reranking options include RRF, MMR, node-distance and a cross-encoder.

**What the search layer does with expired edges — verified from source, and the answer is
"nothing".** I read `graphiti_core/search/search_utils.py` on `main`
(<https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/search/search_utils.py>,
read 2026-09-18). `expired_at`, `valid_at` and `invalid_at` appear **only in the RETURN
projection** of the retrieval queries:

```
expired_at: e.expired_at,
valid_at:   e.valid_at,
invalid_at: e.invalid_at,
```

They appear in **no WHERE clause and no ranking expression**. The search functions filter
on `group_id`, on embeddings, and on explicitly-supplied search filters — never on temporal
validity. **Expired and invalidated edges are returned alongside live ones, with their
timestamps attached, and the consumer decides.**

Filtering is opt-in, via `SearchFilters`
(<https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/search/search_filters.py>,
read 2026-09-18):

```python
valid_at:   list[list[DateFilter]] | None = Field(default=None)
invalid_at: list[list[DateFilter]] | None = Field(default=None)
created_at: list[list[DateFilter]] | None = Field(default=None)
expired_at: list[list[DateFilter]] | None = Field(default=None)
```

`DateFilter` carries a `datetime` and a `ComparisonOperator` (equals, not_equals,
greater_than, less_than, greater_than_equal, less_than_equal, `is_null`, `is_not_null`).
The nested list structure is AND within an inner list, OR between outer elements. **There
is no default that excludes expired edges** — an "only currently-valid facts" query is
`expired_at IS NULL`, written by hand.

This is the single most useful implementation fact in this document, because it means the
state of the art in bitemporal agent memory **does exactly what we want by default**:
return the superseded claim next to the current one, both dated, and let the answer layer
narrate the change. Graphiti did not build a "hide the old fact" mechanism; it built a
"carry the old fact's death certificate" mechanism. Our `ARCHITECTURE.md` demotion rule is
the same design in markdown.

Reranking options include RRF, MMR, node-distance and a cross-encoder. Reported benchmark
numbers on DMR and LongMemEval `[unverified — not fetched]`.

### Mem0

"Mem0: Building Production-Ready AI Agents with Scalable Long-Term Memory"
(arXiv:2504.19413, 2025-04, <https://arxiv.org/abs/2504.19413>). An LLM decides per
incoming fact among **ADD / UPDATE / DELETE / NOOP** against retrieved similar memories;
`Mem0^g` adds a graph variant. Evaluated on LOCOMO. `[unverified — not fetched.]`

The structural difference from Graphiti matters for us: **Mem0's UPDATE overwrites, so
the old value is gone.** Graphiti's `expired_at` keeps it. Our `ARCHITECTURE.md` says
*"Nothing is deleted for being wrong — only demoted, with its date still on it"*, which is
Graphiti's semantics, not Mem0's. Any tool we adopt has to preserve the demoted line.

LOCOMO numbers from memory-system papers have attracted methodology criticism
`[unverified — I did not locate a specific critique in this pass]`.

### Letta / MemGPT, Cognee, A-MEM and the rest

- **Letta / MemGPT** — memory blocks, core vs archival memory, sleep-time compute. I found
  **no explicit temporal invalidation mechanism**; supersession happens only if the
  rewriting agent happens to rewrite the block. `[unverified]`
- **A-MEM** (arXiv:2502.12110, Feb 2025) — Zettelkasten-style agentic memory with linked
  notes and memory evolution. `[unverified]`
- **Cognee**, **Memary**, **MemoryOS**, **MIRIX**, **Memobase**, **LangMem** — listed as
  the 2025 cohort; none confirmed in this pass to have valid-time/transaction-time
  separation. `[unverified]`

### The 2026 successors, which are the interesting ones

**MemStrata** — "Temporal Validity in Retrieval Memory: Eliminating Stale-Fact Errors for
AI Agents over Evolving Knowledge" (Neeraj Yadav, arXiv:2606.26511, 2026-06-25,
<https://arxiv.org/abs/2606.26511>). Maintains a bi-temporal ledger and applies
**deterministic `(subject, relation, object)` supersession rules** — a new object for an
existing (subject, relation) retires the old value, *without similarity thresholds and
without LLM calls*. Numbers:

- baseline RAG serves stale facts **15–40%** of the time when forced to answer;
  MemStrata **~0%**
- accuracy **0.95–1.00** vs RAG's **0.20–0.47** on their evolving-knowledge benchmarks
- retrieval latency **~2.1s** vs **16–18s** for LLM-reranking alternatives

Six local benchmarks, 7B model; dataset names not given in the abstract `[unverified]`.
Single-author preprint, three months old — treat the magnitude as indicative, not
established. But the *shape* of the claim is the one that matters: **a dumb deterministic
supersession rule at write time beat an LLM reranker at query time on both accuracy and
latency by an order of magnitude.**

**NuggetIndex** — "NuggetIndex: Governed Atomic Retrieval for Maintainable RAG" (Saber
Zerhoudi, Michael Granitzer, Jelena Mitrovic, arXiv:2604.27306, 2026-04-30,
<https://arxiv.org/abs/2604.27306>). Verbatim from the abstract:

> *"We propose NuggetIndex, a retrieval system that stores atomic information units as
> managed records, so called nuggets. Each record maintains links to evidence, a temporal
> validity interval, and a lifecycle state. By filtering invalid or deprecated nuggets
> prior to ranking, the system prevents the inclusion of outdated information."*

Their opening complaint is worth quoting for our purposes: RAG is *evaluated* on facts but
*retrieves* passages, and this *"unit mismatch between evaluation and retrieval objects
hinders maintenance when corpora evolve and fails to capture superseded facts or source
disagreements."* Measured on a nuggetised MS MARCO subset, a temporal Wikipedia QA set and
a multi-hop task: **+42% nugget recall, +9 percentage points temporal correctness, −55%
conflict rate, −64% generator input length.**

**This repo's note format is already a nugget store.** One bullet, one line, one date, one
link, elaboration underneath. `ARCHITECTURE.md` §Note format independently arrived at the
unit NuggetIndex argues for. What is missing is the **lifecycle state** and the **validity
interval** — and the lifecycle state is exactly what the newest-wins demotion already
encodes positionally but not machine-readably.

---

## 5. practical patterns that ship

### Which layer, according to who

```
  write-time                    ranker                      prompt
  ──────────                    ──────                      ──────
  MemStrata  (0.95 vs 0.20)     Chronofy   (+66% Gold@5)    put the date in context
  NuggetIndex(−55% conflict)    Azure      ("ranking bias")  and hope
  VersionRAG (90% vs 58%)       Grofsky    (0.60 vs 0.20)
  TianPan/CDC (asserted)        MRAG       (+11% AR@1)      ← measured to fail:
                                                              models over-prefer tagged
                                                              recency (2509.11353) and
                                                              under-prefer inferred
                                                              recency (FRESCO)
  ▲ biggest measured effects     ▲ real but smaller          ▲ uncontrolled
  ▲ costs human labour           ▲ costs one score fn        ▲ free
```

The practitioner consensus, such as it is: **write-time curation and query-time freshness
are complements, and query-time freshness cannot rescue a stale index.** Vector search has
no mechanism to prefer a newer document unless a freshness signal is injected explicitly,
and standard RAG eval suites have no temporal component at all — so a system can score
well on RAGAS-style metrics while returning information superseded weeks ago. (Asserted
across several practitioner writeups including
<https://tianpan.co/blog/2026-04-20-rag-knowledge-base-freshness-index-rot>, 2026-04-20,
and
<https://www.digitaldividedata.com/blog/why-your-retrieval-system-is-only-as-good-as-your-knowledge-base-curation>;
neither cites a controlled study.)

### Metadata filtering vs boosting

Every vector store supports date metadata filters. The universal caution, and the one
Microsoft states outright, is **don't filter when you mean boost**: a hard date filter
removes the still-valid old material. For this corpus that would be catastrophic — the
structural claims (what a suspension is, how reinstatement appeals work) are mostly from
the oldest, most authoritative source.

Azure's operational split is the one to copy:
- **filter** when the query explicitly scopes a date range ("what did he say in January"),
- **boost** otherwise.

### Freshness in the prompt

Cheap, universal, and measurably unreliable on its own:

- Explicit date tags cause **over**-preference for new (up to 25% of pairwise preferences
  flip on a date tag alone; arXiv:2509.11353, 2025-09-14).
- Dates that must be inferred from content cause **under**-preference for new (rerankers
  favour *"older, semantically rich documents, even when they are factually obsolete"*;
  FRESCO, arXiv:2604.14227, 2026-04-14).
- Prompt **position** confounds with date, and flipping position moves conflict accuracy
  by 0.20 on some models (arXiv:2608.20116, 2026-08-20).

Practical consequence: keep the dates in the prompt (the assistant must *cite* them), but
do not let them be the mechanism. Rank arithmetically, then present in a fixed order.
Ordering oldest→newest is preferable to newest→oldest because it makes position and
recency agree rather than fight, and it puts the current answer closest to the generation
point.

### Superseded-fact markers and changelogs as documents

The shipping patterns practitioners name, none of them with controlled measurements
`[unverified as a class]`:

- temporal metadata at index time: created, last-updated, and an explicit **staleness
  threshold**
- **validity intervals assigned by a human expert**, with superseded procedures explicitly
  marked obsolete (described in a CERN ALICE-FIT assistant writeup, arXiv:2511.17154)
- **quarantine by review date** — content past its review date is removed from retrieval
  automatically
- **tombstoning with deferred cleanup** for deletions, incremental re-indexing rather than
  full rebuilds (<https://tianpan.co/blog/2026-04-20-rag-knowledge-base-freshness-index-rot>)
- pruning: more documents means more noise once some are stale

The academic version of "changelog as a first-class document" is VersionRAG's **explicit
change tracking between document states**, which is the component that produced its
60%-vs-0–10% implicit-change-detection gap (arXiv:2510.08109, 2025-10-09). The lesson: if
you want the system to be able to say *"this changed"*, the change has to exist as a
retrievable object. It cannot be recovered by diffing two retrieved passages at query
time — that is the 32.67% multi-hop localisation number from MAGIC.

---

## 6. the write-time school

### What the write-time side measured

| System | Mechanism | Result |
|---|---|---|
| MemStrata (arXiv:2606.26511, 2026-06-25) | deterministic (s,r,o) supersession in a bi-temporal ledger | stale-fact rate 15–40% → ~0%; accuracy 0.20–0.47 → 0.95–1.00; 2.1s vs 16–18s latency |
| NuggetIndex (arXiv:2604.27306, 2026-04-30) | atomic nuggets with validity interval + lifecycle state, **filtered before ranking** | +42% nugget recall, +9pp temporal correctness, **−55% conflict rate**, −64% input length |
| VersionRAG (arXiv:2510.08109, 2025-10-09) | version graph + explicit change objects | 90% vs 58% naive RAG; implicit change detection 60% vs 0–10% |

These are the three largest effect sizes in this entire document, and all three come from
doing work before the query arrives.

### What the query-time side measured

| System | Mechanism | Result |
|---|---|---|
| Chronofy (arXiv:2607.20560, 2026-07-17) | per-fact-type exponential decay reranking | +66.3% Gold@5 with oracle time focus; **+2.0% without**; −15.3% accuracy if naive |
| MRAG (arXiv:2412.15540, 2024-12-20) | query decomposition + semantic×temporal product | +11% AR@1, +4.5 EM |
| Grofsky (arXiv:2509.19376, v2 2026-06-26) | half-life recency prior | Latest@10 0.60 vs 0.20 |
| TimelyRAG (arXiv:2609.11572, 2026-09-10) | temporal distance in ranking | +28.6% nDCG@10 |

Real, reproducible, and roughly a third the size.

### The honest counter-argument

Nobody in print argues *"don't curate"*. The argument against write-time is **it does not
get done**, and it is made from experience rather than measurement:

- a knowledge base is a perishable good with a shelf life, and no role inside an
  organisation has the incentive to maintain it — authors forget their files, IT owns the
  system but not the content, business teams assume IT maintains it
  (<https://dev.to/chen115y/dont-build-that-rag-knowledge-base-seven-reasons-it-will-fail-and-what-to-build-instead-2c3g>)
- a manually curated base drifts from reality as pricing, policy and features change,
  leaving *"a graveyard of accurate, when-written content"*; the proposed alternative is
  retrieving from live sources so the base *is* the source rather than a copy
- widely-repeated claims that ~60% of failed enterprise RAG projects fail on freshness
  rather than retrieval quality, and that accuracy degrades measurably within 90 days of
  go-live, **trace to no controlled study** — the authors who quote them flag them as
  industry commentary (<https://tianpan.co/blog/2026-04-20-rag-knowledge-base-freshness-index-rot>,
  2026-04-20). Do not repeat these numbers.

**This objection does not apply to this repo.** The maintenance cost is real but it is one
person's own notes on their own business, the corpus is a few hundred bullets, and
`ARCHITECTURE.md` already mandates the discipline. The failure mode the critics describe —
nobody owns the content — is absent when the owner is the only reader.

### What is genuinely unmeasured

- **No published comparison of write-time supersession against query-time decay on the
  same corpus.** MemStrata compares against unmodified RAG, not against Chronofy-style
  decay. The relative merits are inferred from separate papers with different baselines.
- **No published half-life for anything resembling a tactical marketing playbook.**
  Chronofy fitted β per fact type on GDELT and MIMIC-IV; nobody has done it for a domain
  that rots on an adversary's release schedule.
- **No published head-to-head of hand-tuned (relevance, recency, authority) vs a learned
  reranker.** The case has to be assembled from Mokrii's annotation thresholds plus the
  BM25F-beats-learned-linear result. ConflictRAG's 7.1% manual-vs-Entropy-TOPSIS gap is the
  nearest direct proxy and it is on a different task.
- **Every published source-authority prior is hand-assigned.** Koganti's λ table, REBEL's
  w = 0.5, Vespa's `quality` attribute — all human-set, all explicitly disclaimed as
  non-optimal by their own authors. **Nobody has published a learned source-authority prior
  for a curated corpus.** So our hand-set table is not worse than the state of the art; it
  *is* the state of the art.
- **No production writeup from any major vendor describing a shipped source-authority
  weight inside a RAG pipeline.** What ships is generic boost machinery that users wire to
  an authority field themselves.
- **TrustRank / Anti-TrustRank / EigenTrust have never been ported to a RAG corpus.** Only
  PageRank has (RAGRank, 2025-10), and most GraphRAG "PageRank" is PPR-as-traversal, not
  authority.
- **`k = 60` in RRF is an untuned constant from a two-page 2009 poster** that became an
  industry default by imitation. Qdrant shipping k = 2 is the live proof it was never
  load-bearing.
- **What Graphiti's search layer does with expired edges** — the single most useful
  implementation detail for us, unconfirmed in this pass.

---

## a concrete scoring formula we could adopt

### first, when this formula applies at all

`tmp/research/our-corpus.md` (measured 2026-09-18) puts the repo at **~148k tokens across
75 files** — smaller than one frontier context window — growing at roughly **2.5M tokens a
year** from the weekly call alone. `tmp/research/rag-sota-2026.md` concludes from that:
stuff the whole corpus into one prompt-cached context and ship.

**That is right, and it means the scoring formula below has no ranker to live in today.**
Everything in §1–§4 that is a *ranking* intervention is dormant until the corpus leaves the
single-window regime, roughly a year out. What is *not* dormant, and what this whole
document keeps pointing at, is the other two layers:

```
   today (148k, one window)              in ~a year (1–3M, retrieval required)
   ────────────────────────              ─────────────────────────────────────
   write-time  ████████████  all of it   write-time  ████████████  still all of it
   prompt      ████████                  prompt      ████████
   ranker      ·                         ranker      ██████        formula below
```

And the single-window regime makes the *prompt* findings more load-bearing, not less,
because the prompt is the only lever there is:

- With everything in context, the model decides what's current by reading dates — and that
  is the one thing it measurably does badly in both directions. Explicit date tags flip up
  to 25% of pairwise preferences (arXiv:2509.11353); inferred recency loses to older,
  richer text (FRESCO, arXiv:2604.14227); a newer timestamp beats an explicit corruption
  flag (arXiv:2608.20116); and repetition reverses the source-credibility preference
  (arXiv:2601.03746). **A group-call transcript that repeats a wrong tactic four times will
  beat a teacher's lesson that states the right one once, in a stuffed context, today.**
- The corpus's own measured skew makes this concrete: `our-corpus.md` finds 155 of 168
  citations in `structured/` come from the single 2026-09-18 call. That is not a recency
  bias, it is a last-thing-I-touched bias, and no ranker fixes it — **coverage is a
  write-time problem.**

So the order of work the evidence supports is: **write-time supersession discipline (already
mandated) → per-bullet `kind` tag and self-scoped phrasing → prompt-level ordering and
citation rules → and only then, when retrieval becomes necessary, the formula below.**

### Step 0, which is most of the value: classify the claim at write time

Chronofy's strongest empirical result is not a constant, it is that **β varies by two
orders of magnitude within one corpus and is 0.0 for some classes** (diagnoses β* = 0.0,
lab results β* = 0.05, Table V). Applying one half-life to this repo would decay "don't
compete with people who have 200+ reviews" (structural, true indefinitely) at the same
rate as "the video verification flow accepts a handheld pan" (tactical, dead in weeks).

So the one piece of schema to add is a per-bullet kind:

```
  kind: structural   →  H = ∞     (no decay)     ← what a thing is, why it works
  kind: tactical     →  H = 90d                  ← what to do this month
  kind: pricing      →  H = 180d                 ← rates, margins, what subs charge
```

Where H = 90d comes from. The two vendors who ship a default both centre on ~3 months,
though neither is a half-life and they disagree with each other:

- **Vespa's `freshness.maxAge` defaults to `3*30*24*60*60` s**, the docs' own gloss
  *"about 3 months"*, with **linear** decay to zero — so its half-*value* point is 45 days
  (<https://docs.vespa.ai/en/reference/rank-feature-configuration.html>). Its logscale
  variant separately defaults to a **7-day** `halfResponse`. The two defaults disagreeing
  inside one product tells you how little is settled here.
- **Azure's worked example in the freshness-aware retrieval doc is `P90D`**
  (<https://learn.microsoft.com/en-us/azure/search/agentic-retrieval-how-to-configure-freshness>,
  2026-06-02).
- The corpus's own witness — *"it worked for a month, then last week it just stopped"* —
  puts the observed lifetime of a working tactic at ~4–8 weeks, so a half-life of roughly
  two observed lifetimes leaves room for the tactic that happens to survive.

A 90-day half-life means a tactic from one observed lifetime ago still scores 0.79 (present
but demoted), from six months ago 0.25, from the December 2025 start of the corpus ~0.13.
That is the right shape for a domain where old tactics are usually wrong but occasionally
come back. **This is the least-defensible constant in the formula.** Two vendor defaults
land near 3 months, but both are untuned general-purpose guesses, neither is a half-life,
and one of the two vendors ships a 7-day alternative in the same file. Treat 90 days as a
starting point and tune it after the first ten queries that return the wrong answer.

### The score

Multiplicative, following Chronofy Eq. 2 and MRAG's semantic×temporal product, **not**
LangChain's additive form (which lets a fresh irrelevant bullet outrank a stale perfect
one):

```
score(claim | query, t_now)
    =  sim(query, claim)                     ∈ (0, 1]   cosine, the base ranker
    ×  A(source(claim))                      ∈ [0.4, 1] authority multiplier
    ×  R(age, kind)                          ∈ [0.1, 1] recency multiplier

R(age, kind) = max( floor , 2 ^ ( − max(0, age_days − grace) / H(kind) ) )

    grace = 30 days        no decay at all for the first month
    floor = 0.10           an old claim is demoted, never removed
    H(structural) = ∞  ⇒  R ≡ 1
    H(tactical)   = 90 days
    H(pricing)    = 180 days
```

**`grace = 30`** is Elasticsearch's `offset` parameter used as intended — their own date
example is `"scale": "10d", "offset": "5d"`, i.e. a flat period before age counts
(<https://www.elastic.co/guide/en/elasticsearch/reference/current/query-dsl-function-score-query.html>).
Thirty days here because the corpus's own evidence says a tactic survives about a month
before anyone notices it broke; decaying inside that window is decaying on noise.

**`floor = 0.10`** exists because of Azure's rule — *"Freshness is a ranking bias, not a
hard filter. Older documents can still appear when they're strongly relevant"* — and
because hard-filtering by date in this corpus would delete the teacher's foundational
material. With floor 0.10 and authority 1.0, a very old high-authority claim still
outranks a fresh low-authority one whenever its cosine is 4× better, which is the
behaviour we want for "what is a suspension".

**Authority tiers `A`.** These map this repo's `ref/` subtrees onto the one published prior
table I found — Koganti & Garrido-Lestache Belinchon's λ(s_d), arXiv:2607.22584:

| Source class | A | grounding |
|---|---|---|
| teacher's course lesson (`ref/skool_gmbpp/course/`) | **0.95** | their `peer_reviewed` tier, 0.95. The institutional source; Schuster et al. (arXiv:2601.03746, 2026-01-07) show LLMs already prefer this class, so we reinforce an existing prior rather than fight it |
| human's own vetted notes (`structured/approved/`) | **0.95** | human-promoted; TrustRank's small hand-picked trusted seed (Gyöngyi et al., VLDB 2004, <200 seeds sufficed) |
| another operator, first-hand, on a call (`ref/loom/`) | **0.70** | their `organization_report` tier, 0.70. First-hand and dated, but a sample of one |
| chatbot research output (`ref/research/`) | **0.30** | **their `ai_generated` tier, exactly 0.30.** Derived, not observed; no chain to a primary source; carries its own knowledge-cutoff staleness — the same disease we are treating |

The authors' own caveat applies in full: these *"are not learned from data and are not
claimed to be optimal."* But it is a published table from a health-domain RAG paper with
Precision@5 0.48 → 0.72, which is better grounding than making the numbers up.

**Sanity-check the spread against REBEL's ceiling.** REBEL (arXiv:2504.07104, 2025-03-14)
caps any single secondary criterion at a 25% swing against relevance (0–5 on a 0–10 scale,
`w = 0.5`). Our authority span 0.95 → 0.30 is a 3.2× ratio, which is far more aggressive
than REBEL's. That is deliberate and it is the choice to defend: a chatbot's guess about
GBP verification is not 75%-as-good as the teacher's lesson, it is a different kind of
object. If the assistant starts ignoring `ref/research/` entirely, raise 0.30 toward 0.50
(Koganti's `blog` tier) rather than flattening the whole table.

**The crossover this produces, stated explicitly, because it is the behaviour people will
argue about.** A chatbot claim published today scores `0.30 × 1.00 = 0.30`. A teacher
lesson at one half-life scores `0.95 × 0.50 = 0.475`; at two half-lives `0.95 × 0.25 =
0.2375`. So **a fresh chatbot claim overtakes an authoritative one at roughly 180 days** —
for *tactical* claims only, since structural ones never decay. That is the intended
semantics: in a domain rotting this fast, a six-month-old authoritative tactic really is
worse than a today-dated guess, and the assistant will say so with both dates attached.

If that crossover feels wrong in practice, move `A(chatbot)` — it is the single knob. Do
not move the half-life to fix an authority problem.

**Calibrate before you trust any of these constants.** Qdrant documents the failure mode
directly: a decay term applied on top of a fused score *can overwhelm the fusion*, because
the fused score's ceiling moves with the number of prefetches
(<https://qdrant.tech/blog/decay-functions/>, 2025-09-01). Whatever produces `sim` here,
measure its actual observed range on real queries before assuming the multipliers above
land where the arithmetic says.

**Specificity** deliberately has no term. The mission asked for recency × authority ×
specificity; the grounded answer is that specificity is a **unit** decision, not a score
term. NuggetIndex got +42% recall and −64% context length by making the retrieval unit an
atomic fact with its own provenance (arXiv:2604.27306, 2026-04-30), and this repo's note
format already is that unit. Retrieve bullets, not files, and specificity takes care of
itself. Adding a third multiplier would be tuning a fourth constant with no evidence behind
it.

### What the assistant does with the result

Contradiction surfacing is **not** a scoring problem here and should not be implemented as
one. MAGIC's ceiling is 55% at localising which two passages conflict, 32.67% when it takes
two hops (arXiv:2507.21544, EMNLP 2025 Findings). But `ARCHITECTURE.md` §Recency already
mandates that a human records supersession structurally — new claim in place, old claim
demoted underneath with its date. So:

1. Retrieve the bullet. Its demoted predecessors come with it, because they are indented
   under it in the same block. No detection required.
2. Answer with the top claim, its date, and its source link.
3. If the retrieved block contains a demoted line, say so, in EvoTrustRAG's
   knowledge-evolution framing (arXiv:2608.07933, 2026-08-08): *"as of 2026-09-18, X.
   Until 2026-06-02 the answer was Y."* Both dates, both links.
4. If two *separately sourced* bullets disagree and neither is marked as superseding the
   other — i.e. the human has not adjudicated yet — do **not** silently pick the newer
   one. Present both with dates and authorities. This is EvoTrustRAG's third case
   (*"unresolved uncertainty ... remains visible to the generator"*), it is RAMDocs'
   finding that ambiguity and misinformation need different handling (arXiv:2504.13079,
   COLM 2025), and it is **what Graphiti's search layer does by default** — expired edges
   are returned with their timestamps, never filtered (verified from source, §4). That
   unadjudicated pair is also exactly the thing to surface to the human for promotion,
   which closes the loop back into `structured/`.
5. Order the retrieved claims **oldest → newest** in the prompt, so prompt position and
   recency agree rather than confound (arXiv:2608.20116, 2026-08-20 — flipping placement
   moved conflict accuracy by 0.20 on Qwen3-4B).
6. Classify the query first: *"about now"* vs *"about a past state"*. Apply `R` only in the
   first case. This one branch is what separates Chronofy's +66.3% from its −15.3%.

**One phrasing rule falls out of Graphiti's interval test and is worth adopting in the
notes themselves.** Graphiti treats two claims as contradictory only when their validity
intervals *overlap*; non-overlapping claims are a timeline, not a conflict. Our bullets are
written as open-ended present tense ("the video verification flow accepts a handheld pan"),
which means every old bullet's interval runs to infinity and therefore overlaps every new
one — every update looks like a contradiction. A bullet written as *"as of 2026-01, …"*
closes its own interval and reads as history instead. **The cheapest single improvement to
the corpus is scoping tactical bullets to when they were observed**, which is one word at
write time and removes a whole class of false conflicts at read time.

### What I would not build

- A temporal knowledge graph. Graphiti/Zep's machinery exists to *infer* supersession that
  nobody recorded. This repo records it. Building the graph would re-derive, at 55%
  localisation accuracy, information already written down at 100%. Worth noting that
  Graphiti's *output* semantics — return the expired edge with its dates, let the caller
  narrate — is exactly what we want, and it is three lines of markdown convention here
  rather than a graph database.
- A learned reranker. Mokrii et al. (arXiv:2103.03335, SIGIR'21) measured the threshold:
  beating BM25 with a trained ranker needs **>100 annotated queries on the easiest dataset
  and 1–8K on realistic ones**, at roughly an hour of judgement per query, and below that
  threshold few-shot training actively degrades performance. At n≈300 documents we are not
  close. Grid-search three constants on a few dozen judged queries instead.
- Automatic contradiction detection as a *gate*. As a **background report** — "these two
  bullets look like they disagree and neither is marked as superseding the other, look at
  them" — it is worth its 83.6% ID accuracy, because a human checks the output. As
  something the answer path depends on, its 55% localisation accuracy is disqualifying.

---

## sources

Fetched and verified in this pass:

- Chronofy — arXiv:2607.20560, 2026-07-17 — <https://arxiv.org/abs/2607.20560>, full text <https://arxiv.org/html/2607.20560v1>
- Grofsky, Freshness and the Limits of Heuristic Trend Detection in Temporal RAG — arXiv:2509.19376, v1 2025-09-20, v2 2026-06-26 — <https://arxiv.org/abs/2509.19376>
- MRAG / TempRAGEval — arXiv:2412.15540, 2024-12-20 / 2025-05-21 — <https://arxiv.org/abs/2412.15540>, full text <https://arxiv.org/html/2412.15540v2>
- TimelyRAG — arXiv:2609.11572, 2026-09-10 — <https://arxiv.org/abs/2609.11572>
- VersionRAG — arXiv:2510.08109, 2025-10-09 — <https://arxiv.org/abs/2510.08109>
- HoH — arXiv:2503.04800, 2025-03-03 — <https://arxiv.org/abs/2503.04800>
- Knowledge Conflicts for LLMs: A Survey — arXiv:2403.08319, 2024-03-13 / 2024-06-22, EMNLP 2024 — <https://arxiv.org/abs/2403.08319>
- MAGIC — arXiv:2507.21544, 2025-07-29 / v3 2025-10-09, EMNLP 2025 Findings — <https://arxiv.org/abs/2507.21544>, results <https://arxiv.org/html/2507.21544v3>
- RAMDocs / MADAM-RAG — arXiv:2504.13079, 2025-04-17 / 2025-08-12, COLM 2025 — <https://arxiv.org/abs/2504.13079>
- ConflictRAG — arXiv:2605.17301, 2026-05-17 / 2026-06-08 — <https://arxiv.org/abs/2605.17301>
- EvoTrustRAG — arXiv:2608.07933, 2026-08-08 — <https://arxiv.org/abs/2608.07933>
- Whose Facts Win? LLM Source Preferences under Knowledge Conflicts — arXiv:2601.03746, 2026-01-07 — <https://arxiv.org/abs/2601.03746>
- Do LLMs Favor Recent Content? Recency Bias in LLM-Based Reranking — arXiv:2509.11353, 2025-09-14 — <https://arxiv.org/abs/2509.11353>
- FRESCO — arXiv:2604.14227, 2026-04-14 — <https://arxiv.org/abs/2604.14227>
- MemStrata / Temporal Validity in Retrieval Memory — arXiv:2606.26511, 2026-06-25 — <https://arxiv.org/abs/2606.26511>
- NuggetIndex — arXiv:2604.27306, 2026-04-30 — <https://arxiv.org/abs/2604.27306>
- LLMs Followed the Newer Timestamp Over the Reliability Flag — arXiv:2608.20116, 2026-08-20 — <https://blog.pebblous.ai/blog/llm-evidence-arbitration-recency/en/>
- Generative Agents — arXiv:2304.03442, 2023-04-07 / 2023-08-06 — <https://arxiv.org/abs/2304.03442>, §Retrieval at <https://arxiv.org/html/2304.03442v2>
- Azure AI Search, freshness-aware agentic retrieval (preview) — ms.date 2026-06-02, updated 2026-09-17 — <https://learn.microsoft.com/en-us/azure/search/agentic-retrieval-how-to-configure-freshness>
- Azure AI Search, scoring profiles — <https://learn.microsoft.com/en-us/azure/search/index-add-scoring-profiles>
- Elasticsearch function_score decay functions — <https://www.elastic.co/guide/en/elasticsearch/reference/current/query-dsl-function-score-query.html>
- Graphiti, read from source on `main` 2026-09-18 — search layer <https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/search/search_utils.py>, filters <https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/search/search_filters.py>, invalidation <https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/utils/maintenance/edge_operations.py>, contradiction prompt <https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/prompts/dedupe_edges.py>
- Vespa `freshness.maxAge` default (`3*30*24*60*60`, "about 3 months") and `halfResponse` default (`7*24*60*60`) — <https://docs.vespa.ai/en/reference/rank-feature-configuration.html>
- LangChain TimeWeightedVectorStoreRetriever — <https://python.langchain.com/v0.2/docs/how_to/time_weighted_vectorstore/>
- RAG knowledge base freshness / index rot — 2026-04-20 — <https://tianpan.co/blog/2026-04-20-rag-knowledge-base-freshness-index-rot>
- Source-Aware Reranking for RAG: A Reliability Prior Approach — arXiv:2607.22584 (listing says 2026-06-08 against a 2607 prefix; discrepancy unresolved) — <https://arxiv.org/abs/2607.22584>
- RA-RAG / Estimation of Source Reliability — arXiv:2410.22954, 2024-10-30, v5 2025-10-14 — <https://arxiv.org/abs/2410.22954>
- CAG / Not All Contexts Are Equal — arXiv:2404.06809, 2024-04-10, EMNLP 2024 Main — <https://arxiv.org/abs/2404.06809>
- CrAM / Credibility-Aware Attention Modification — arXiv:2406.11497, 2024 — <https://arxiv.org/abs/2406.11497>
- CrediRAG — arXiv:2410.12061, 2024-10, The Web Conference '25 — <https://arxiv.org/abs/2410.12061>
- Trustworthiness in RAG Systems: A Survey — arXiv:2409.10102, 2024-09 — <https://arxiv.org/abs/2409.10102>; list <https://github.com/Arstanley/Awesome-Trustworthy-RAG>
- RAGRank — arXiv:2510.20768, 2025-10-23, v2 2025-12-15 — <https://arxiv.org/abs/2510.20768>
- REBEL / Relevance Isn't All You Need — arXiv:2504.07104, 2025-03-14 — <https://arxiv.org/abs/2504.07104>
- Mokrii, Boytsov, Braslavski, transfer learning + pseudo-labeling for BERT rankers — arXiv:2103.03335, 2021-03-04, SIGIR'21 — <https://arxiv.org/abs/2103.03335>
- Cormack, Clarke, Büttcher, Reciprocal Rank Fusion — SIGIR'09, 2009-07 — <https://cormack.uwaterloo.ca/cormacksigir09-rrf.pdf>
- TrustRank — Gyöngyi, Garcia-Molina, Pedersen, VLDB'04, 2004-08-31 — <https://www.vldb.org/conf/2004/RS15P3.PDF>
- EigenTrust — Kamvar, Schlosser, Garcia-Molina, WWW'03, 2003-05 — <https://nlp.stanford.edu/pubs/eigentrust.pdf>
- Anti-TrustRank — Krishnan & Raj, AIRWeb'06, 2006-08-10 — <https://airweb.cse.lehigh.edu/2006/krishnan.pdf>
- PageRank — Page, Brin, Motwani, Winograd, Stanford TR 1999-66 — <http://ilpubs.stanford.edu:8090/422/>
- LambdaBM25F / learning BM25 — CIKM 2009 — <https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/LearningBM25MSRTechReport.pdf>
- Vespa rank features (`freshness`, `age`, default `maxAge` 7776000 s) — <https://docs.vespa.ai/en/reference/ranking/rank-features.html>; `deservesFreshness` example — <https://docs.vespa.ai/en/ranking/nativerank.html>
- OpenSearch function_score decay derivations — <https://docs.opensearch.org/latest/query-dsl/compound/function-score/>
- Qdrant decay functions and the RRF-overwhelm warning — 2025-09-01 — <https://qdrant.tech/blog/decay-functions/>; hybrid queries, RRF k=2 — <https://qdrant.tech/documentation/concepts/hybrid-queries/>
- Weaviate hybrid fusion algorithms (rankedFusion 1/(rank+60) vs relativeScoreFusion, default change in v1.24) — <https://weaviate.io/blog/hybrid-search-fusion-algorithms>
- Google, "Giving you fresher, more recent search results" — 2011-11-03 — <https://search.googleblog.com/2011/11/giving-you-fresher-more-recent-search.html>; 35%-touched vs 6–10%-changed correction — <https://searchenginewatch.com/2011/11/04/google-gets-fresh-with-algorithm-update-affecting-35-of-searches/>
- QDF origin, secondary paraphrase of the 2007 NYT/Singhal piece — 2022-06-29 — <https://www.searchenginejournal.com/google-algorithm-history/freshness-algorithm/>
- 2024 Google Content Warehouse leak — Fishkin 2024-05-27 <https://sparktoro.com/blog/an-anonymous-source-shared-thousands-of-leaked-google-search-api-documents-with-me-everyone-in-seo-should-see-them/>; field definitions verbatim at <https://google-api-content-warehouse.hexdocs.pm/0.4.0/GoogleApi.ContentWarehouse.V1.Model.PerDocData.html>, <https://hexdocs.pm/google_api_content_warehouse/0.4.0/GoogleApi.ContentWarehouse.V1.Model.NlpSaftDocument.html>, <https://google-api-content-warehouse.hexdocs.pm/0.4.0/GoogleApi.ContentWarehouse.V1.Model.CompressedQualitySignals.html>

Named from search results, abstract-level or `[unverified]`:

- TempRetriever — arXiv:2502.21024, 2025-02 — <https://arxiv.org/abs/2502.21024>
- T-GRAG — arXiv:2508.01680, 2025-08 — <https://arxiv.org/abs/2508.01680>
- TG-RAG / RAG Meets Temporal Graphs — arXiv:2510.13590, 2025-10 — <https://arxiv.org/abs/2510.13590>
- TIDE / Time Present and Time Past — arXiv:2608.08512, 2026-08 — <https://arxiv.org/abs/2608.08512>
- GRAB-RAG / Prompt-Based Abstention Fails Under Misleading Context — arXiv:2608.22228, 2026-08 — <https://arxiv.org/abs/2608.22228>
- Trust or Abstain? A Self-Aware RAG Approach — arXiv:2605.18792, 2026 — <https://arxiv.org/abs/2605.18792>
- When Evidence Conflicts (biomedical order effects) — arXiv:2605.14115, 2026 — <https://arxiv.org/abs/2605.14115>
- Zep: A Temporal Knowledge Graph Architecture for Agent Memory — arXiv:2501.13956, 2025-01 — <https://arxiv.org/abs/2501.13956>; <https://github.com/getzep/graphiti>
- Mem0 — arXiv:2504.19413, 2025-04 — <https://arxiv.org/abs/2504.19413>
- A-MEM — arXiv:2502.12110, 2025-02 — <https://arxiv.org/abs/2502.12110>
- CERN ALICE-FIT support assistant (expert-assigned validity intervals) — arXiv:2511.17154
- Evidence Sufficiency Benchmark (L1–L5 abstention calibration) — <https://www.techscience.com/cmc/v89n1/68467/html>
- Don't Build That RAG Knowledge Base — <https://dev.to/chen115y/dont-build-that-rag-knowledge-base-seven-reasons-it-will-fail-and-what-to-build-instead-2c3g>
- Knowledge base curation for retrieval systems — <https://www.digitaldividedata.com/blog/why-your-retrieval-system-is-only-as-good-as-your-knowledge-base-curation>

Cited from memory, primary source not fetched — treat as `[unverified]`:

- TimeQA arXiv:2108.06314 (NeurIPS 2021); ArchivalQA arXiv:2109.03438 (SIGIR 2022); StreamingQA arXiv:2205.11388 (ICML 2022); RealTimeQA arXiv:2207.13332 (NeurIPS 2023); FreshLLMs/FreshQA arXiv:2310.03214 (2023-10)
- Bitemporal modelling — Snodgrass/TSQL2 1990s; SQL:2011 temporal tables; XTDB; Datomic `as-of`
- HippoRAG arXiv:2405.14831 (NeurIPS 2024); MixPR arXiv:2412.06078 (2024-12); NodeRAG arXiv:2504.11544 (2025-04) — PPR-as-traversal, not authority
- NavBoost / Glue, 13-month click window — Pandu Nayak sworn testimony, *US v. Google*, late 2023. `Q*` as a DOJ-confirmed term: **not located in trial records; treat as leak-only**
- Pinecone native decay/boost primitive — absence not directly verified; do not assert
