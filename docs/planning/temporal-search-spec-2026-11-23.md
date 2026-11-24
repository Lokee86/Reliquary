# Reliquary Temporal Search Specification

Parent index: [Planning](./INDEX.md)

## Purpose

Current REL search ranks Memory text and vectors across the eligible owner snapshot. A relevant result from the wrong period can consume a top-k slot or an agentic candidate offer. Archive hybrid search ranks whole fragments, which can cover turns on several dates. A date on one turn in a fragment gives no evidence that a topic mentioned in another turn was discussed on that date.

## Overview

Reliquary already captures source timestamps in Memory revisions and Archive turns. Chronos resolves dates and ranges inside Memory text and can verify a stored interpretation against the original body and source time. Search needs to use those two sources of evidence before it limits the candidate set.

## User and developer outcomes

| Search intent | Date basis | Observable behavior |
| --- | --- | --- |
| “What did I say about the gateway on Friday?” | Source time | Return topical evidence whose recorded source falls inside Friday. |
| “Find references to the gateway rollback on Tuesday.” | Mentioned time | Return topical evidence with a Chronos date or interval that overlaps Tuesday. Include the phrase and resolved interval. |
| “Which gateway configuration was active on Tuesday?” | Claim validity | Perception can support this when it derives validity periods from changes and corrections. The search in this spec supplies dated evidence for that work. |

A short result limit still finds a qualifying older hit. A Memory or Archive turn saying “yesterday” continues to refer to the day before its source timestamp when searched after import or reopen. Explicit searches report which date basis and window they applied.

## Target architecture

| Layer | Responsibility |
| --- | --- |
| Root and CLI | Accept an explicit date basis and time window. Validate the request before provider work. Expose the applied window and matched evidence on temporal results. |
| Gateway | Capture the authorized owner snapshot or Archive generation. Carry the filter through candidate selection, agentic state, and any prepared delivery. Reuse the existing authority and stale-cut checks. |
| Application | Combine the filter with the captured source metadata and eligible result population. Adapt accepted Memory records to Chronos and use the same date predicates across passive search, agentic retrieval, and Archive search. |
| Retrieval and Chronos | Retrieval owns the shared filter values, interval predicate, and query state. Chronos interprets the temporal expression and validates any stored interpretation. Application uses its existing dependencies on both. |
| Owner-session and Archive | Expose source metadata at the captured cut. Supply the accepted source timestamp for Memories and the exact turn membership for Archive fragments. |

The passive Memory path runs through `src/retrieval.rs`, `instance_passive_retrieval.rs`, and `owner_retrieval.rs`. Its snapshot already contains the Memory revision and body, and the result already exposes the source time. Filter that population before either the hybrid ranker or the exact-semantic router applies a limit.

Reuse the Memory body and source-time adapter currently in `reliquary-application/src/dream_temporal.rs`. Place the shared validation at the Application boundary so Dream and search have one conversion from stored proof to Chronos input. Keep match policy with each consumer. Dream compares Memories for graph organization. Search compares a candidate's dated evidence with the requested window.

### Request and result

- One required topic query and an optional temporal filter. The filter contains a `source` or `mentioned` basis and one nonempty half-open interval `[start_ns, end_exclusive_ns)`. The canonical values are signed Unix nanoseconds. Reject invalid or unrepresentable bounds, or an end at or before the start, as invalid input.
- Add opt-in temporal entry points at the Root boundary for passive Memory search, agentic query, and Archive hybrid search. Reuse the existing options and result types inside those paths. Temporal responses add the normalized filter and citation-ready evidence grouped by result ID. The caller receives the ordinary source and score metadata alongside the temporal evidence.
- In the CLI, add `--time-basis source|mentioned`, `--from RFC3339`, and `--before RFC3339` to the supported search and query invocations in their respective phases. Require all three flags together. Interactive JSONL can supply an optional structured filter on `query`. Validate its fields with the same parser and existing line limits.
- API and CLI callers can supply exact RFC3339 bounds with offsets. A conversational adapter resolves a phrase such as “last Tuesday” using the request time and the caller's calendar zone, then passes the normalized window and topical query explicitly. Freeze that resolved window for the query. Bounds derived from two local midnights handle daylight-saving changes. Repo Chronos calendar analysis currently uses UTC for stored timestamp references. Use that established interpretation for historic relative expressions and preserve explicit offsets in the source evidence.

```powershell
reliquary rel search project.rel --query "gateway rollback" --mode lexical --time-basis source --from 2026-11-20T00:00:00-08:00 --before 2026-11-21T00:00:00-08:00
```

```json
{"request_id":"q1","command":"query","text":"gateway rollback","temporal_filter":{"basis":"mentioned","from":"2026-11-20T00:00:00-08:00","before":"2026-11-21T00:00:00-08:00"}}
```

## Implementation decisions

### Phase 1 — Passive REL Memory search

1. Implement both date bases in `rel search` and the corresponding opt-in Root entry point. Ship source-time selection first within the phase, then add mentioned-time selection before closing the phase.
2. For source time, test membership using the accepted Memory revision's `source_time_ns`. An absent source timestamp is ineligible for this filter. Use the recorded source timestamp even when import or modification occurred later.
3. For mentioned time, run the shared Chronos analysis on the exact Memory body with its recorded source timestamp. Accept a concrete anchor or a closed interval whose normalized span overlaps the requested window. Test instants by membership. Preserve the precision and original date phrase in the result so a month or range can be presented at its actual precision. A missing source timestamp leaves a relative reference unresolved. Explicit dates in the same body can still match.
4. Use validated stored interpretations when available. If the stored interpretation fails verification, use only deterministic matches from the current body and source time. A phrase earns a mention match only when its evidence can be tied to the original text. Unresolved expressions, standalone durations, unanchored event offsets, and bare recurrence rules provide insufficient evidence for this window.
5. Apply temporal eligibility before truncation in lexical, hybrid, and exact-semantic modes. Metadata-only source-time checks can run before scoring. For mentioned time, a topic candidate check may avoid unnecessary parsing as long as it happens before any top-k or candidate cap. Rank eligible results with the mode's existing topical scoring and stable tie-breaking. Explain the match with the source timestamp or the bounded set of matching phrases, intervals, and precision. Include the requested basis and window even for empty results.
6. A compatible profile with zero in-window matches returns an empty result. A genuinely unavailable profile follows the existing availability error. Apply search limits and scoped missing-vector counts to the eligible population. Lexical temporal search requires view access and runs locally.
7. Read the current cut and existing REL data. Keep temporal analysis outside writer locks. Derive matches on demand for temporal queries and measure the cost on the existing representative REL fixtures before considering an index. Include proof identity and Chronos rule version in any later cache key alongside the body and source timestamp.

### Phase 2 — Agentic REL retrieval

Apply the same predicate before Freshness chooses from the lexical and semantic candidate union. A model evaluates topical relevance among eligible candidates. Provide the resolved window, date basis, and matched evidence in the offer and preview. Keep the existing candidate budget and limit the size of returned temporal evidence under the existing response budget.

A query retains its filter through topical refinement and traversal. Recheck both readable anchors and offered neighbors against the live authorized cut and the active filter. A refinement keeps the existing policy of reconsidering prior candidates that were declined. To change or clear the window or basis, issue a new query so candidate discovery runs again. Previously confirmed deliveries retain their duplicate-suppression state within the external turn.

Bind the filter to the same query generation as the searched IDs and the prepared delivery. Serialize an optional normalized filter in the manifest when active, and restore it wherever recovery restores that manifest's query frontier. Old manifests with the field omitted decode as unfiltered. A replay validates the existing digest and owner/turn bindings. The new field must travel through the typed prepare and recovery path, including when recovery merges receipt evidence into a newer active turn. In that case the current query retains its own window.

Inspection and ranking are read operations. Only the existing confirmed external consumption path can award Freshness credit, and only for the exact delivered Memory revisions.

### Phase 3 — Archive hybrid search

Scope Archive candidates to the active generation and its accepted Archive cut. Project each fragment's exact source path into readable turn references with per-turn timestamps. Apply the source predicate to those timestamps. For mentioned-time search, run Chronos on each relevant turn with that turn's original timestamp, so the matched phrase is linked to its turn ID. Test membership per turn, since an interval between the fragment's endpoints can contain dates on which no turn occurred.

Score a fully eligible fragment with its existing hybrid scorer. For a fragment with a mix of eligible and ineligible turns, calculate lexical evidence and score from the eligible turn text only. Use the existing lexical component weight for these partial hits and report their basis and scoring components. Packed fragment embeddings include the surrounding turns, so the mixed-fragment path needs a positive eligible-turn lexical match to emit a hit. Report matched turn IDs or a bounded list, and label the rest of the fragment as context. This conservative first pass trades some semantic recall at a date boundary for grounded results while reusing the active generation.

Reject a purported mention match when its supporting turn text cannot be read. For source-time matches, a fully eligible fragment with authorized vector-only evidence may retain the existing metadata-only behavior if its accepted turn coordinates and timestamps are available. Apply limits after date eligibility and scoring. Preserve existing generation, branch and source-cut identity on the result.

## Migration strategy

Use a compatibility-preserving rollout. Root option structs are public and used in downstream struct literals. Retain their shape, the unfiltered method signatures and ranking behavior, and the existing CLI output when temporal flags are absent. Isolate adaptation at the entry and response boundaries and use a single ranking path per existing search mode with an optional filter. Keep the stable digest bytes for old manifests by omitting the new field when there is no filter.

Ship the three phases in order. Passive source-date search can be built against current `main`. Use the restored Chronos intake path for acceptance involving newly ingested model-derived temporal proofs. Search consumes those proofs through the existing Memory record contract. Use existing REL 2.1 files and re-derive searchable dates on demand. The design fits the existing package dependency direction.

## Testing decisions

| Seam | Acceptance cases |
| --- | --- |
| Chronos and Application | Exclusive upper bound, exact lower bound and overlap. Explicit dates with missing source time. A relative date in historical evidence, a coarse interval overlapping a day, and a stale stored proof. Compare the same instant under two explicit offset windows and verify membership in the intended local day. |
| Root and Gateway passive search | Put a high-scoring out-of-window item ahead of a lower-scoring qualifying one and use a limit of one. Exercise lexical, hybrid, and exact-semantic modes. Verify zero qualifying results, correct evidence, existing authorization, and identical unfiltered behavior on reopen. |
| Agentic query and recovery | More than ten high-Freshness items outside the window cannot consume the offer budget. Exercise refinement, a new window, traversal across the boundary, stale-cut rejection, and replay of both legacy and temporal manifests. Confirm a read-only query and exact consumption replay follow the existing Freshness rules. |
| Archive and CLI | Exercise a fragment with no actual in-window turn even though its endpoints span the window. Exercise a mixed fragment whose only useful topical evidence is outside the window, and one with valid in-window evidence. Test multiple branches, per-turn relative dates, index reuse on reopen, malformed flags, JSONL compatibility, and byte-stable passive reads. |

Extend the existing executable seams in `src/retrieval/tests.rs`, Gateway `instance_retrieval/inspection_tests.rs` and recovery tests, Application `agentic_retrieval_tests.rs`, and `cli/tests/cli.rs`. The current tests for ranking before truncation, the ten-candidate offer, Archive hybrid ranking, and view-only lexical search provide baseline witnesses. Use a host-supplied named-zone window across a daylight-saving boundary when validating caller integration.

## Out of scope

Perception is the planned owner for claim-validity intervals. A point-in-time question about what was true needs an additional model for when claims started, ended, and were corrected. Date-only browsing, compound conditions across both date bases, recurrence expansion, automatic reinterpretation of free-text search, and a persistent temporal index are follow-on capabilities. This spec targets the existing REL search surfaces.

## Completion gates

- Phase 1 delivers both date bases through Root and CLI, with before-limit behavior and explicit evidence. Phase 2 delivers the same contract through the agentic entry point and verifies state recovery. Phase 3 proves the exact in-window Archive turn evidence and boundary scoring.
- Verify source-offset handling, cutoff and precision behavior, backward compatibility, and authorized owner-cut isolation. Run targeted checks for each phase, then the repository's locked offline workspace, CLI and Root-only consumer gates, architecture guard and documentation checks before integration. Use a Windows runner that can run file publication and loopback fixtures for the full acceptance suite.
- Record relevance and latency on existing REL fixtures for ordinary and filtered queries. Inspect call paths and package edges against the approved DAG, with an Arcana impact and caller check when a current graph is available.

## Related docs

- [Root product API](../public-product-api.md)
- [CLI reference](../reference/cli.md)
- [Semantic processing](../architecture/semantic-processing.md)

## Notes

### Baseline

Source inspected on Reliquary `main` at `22f8eb73`. The restored Insomnia and Dream work is on `restore/chronos-parity` at `56180f1c`. That commit is a separate branch at the time of writing. Arcana had no current snapshot for the checkout, so the structural mapping uses the checked package DAG, direct source inspection, and existing test seams.
