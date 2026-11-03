# ADR 0043: REL is the canonical code identity; retire CVA

Parent index: [Architectural decisions](INDEX.md)

**Status:** Accepted **code naming direction** — 2026-11-03. The concrete persisted-format and stable-identity decisions are now defined by [ADR 0048](0048-rel-current-format-and-stable-identity-domains.md) and [Stage 5 migration design](../cva-to-rel-format-and-identity-migration-2026-11-03.md); implementation sequencing remains Stage 6. No source change or corpus conversion is implied by this ADR alone.

## Context

The product already exposes typed `.rel` (Reliquary) and `.phy` (Phylactery) owner kinds. Earlier naming ADRs 0018 and 0020 retired CVA as the product file name but left its private implementation naming unresolved, and kept original byte markers unchanged for stability.

The project owner now explicitly directs that **CVA references be eliminated from active code and replaced with REL terminology during the architecture overhaul**. This closes the old private-naming question. It is not permission to weaken format, source identity, recovery or current-only reader requirements.

A source-grounded count from tracked Rust files on this branch found **1,415 `.rs` files**, with case-insensitive `cva` in **644 files** and approximately **12,122 textual occurrences** (including tests, filenames, strings, byte markers, and imports). The count is a lexical impact measure, not distinct Rust symbols. For example, `Cva` occurs ~2,806 times and `CvaError` ~760 times; these counts overlap. Current internal identifiers include `cva_*`, and current-format stored records include CVA-prefixed eight-byte magic such as `CVAPUB01` and `CVAFRS07`. Rust source also includes deterministic identity/hash domain separators beginning with that spelling. The current corpus is still unrecovered on this branch for CVAFRS07 Freshness semantics.

## Decision: target terminology

1. **`REL`** is the acronym in documentation, protocol concepts and user-facing language; **`Rel`** is the idiomatic Rust type prefix; **`rel_`** is the Rust module/function/variable prefix where a scope prefix is needed.
2. The old public `Cva`/ `CvaError` type family is replaced by the **`Rel` / `RelError`** domain names at the eventual public API cut. The **`RelSession`** internal type is the canonical opened REL mutable owner aggregate proposed in the [owner-session design](../rel-phy-owner-session-design-2026-11-03.md). Avoid duplicate internal/public mutable aggregates.
3. `Phylactery`, `PhySession` and PHY terminology remain semantically distinct. This is not a blanket `Cva -> Rel` replacement in PHY-specific logic or code whose concept is simply `Container`, `Archive`, `Owner` or `Gateway`; rename by actual responsibility.
4. Rename `cva_*` modules/files, locals, API parameters, error strings, CLI flags/options and generated test identifiers to the correct domain name, or delete the old symbol when its behavior moves to a workflow.
5. **No permanent `type Cva = Rel` alias**, duplicate live paths, deprecated bridge façade, `cva` compatibility module or backward-facing public code symbol survives the final architecture.
6. Current immutable historical documentation and commit history can retain their period-accurate spelling. Active source and tests have a **zero-case-insensitive-`cva` target** at final completion, including byte-string source literals. The final scan must have defined tracked-source scope and audit exclusions; archived binary fixtures are checked for format separately.

## Crucial separation: Rust names versus persisted identity

Changing `Cva` to `Rel` in a Rust type is normally an API refactor. Changing `b"CVAPUB01"`, `b"CVAFRS07"`, source-prefix tags, digest inputs, recorded constants, domain-separated hashes or identifiers is **a different operation**: it changes the bytes existing REL/PHY files expect.

Therefore the no-CVA-source target requires a **separately designed current-format wire migration**, not an unchecked source-text substitution and not a hidden runtime compatibility layer.

The wire-format decision must:
- inventory every live current-format magic, nested carrier, checksum/digest input, deterministic identity domain, foreign reference, stored source ID, retained proof, replay validator, and owned record index affected by old spelling;
- distinguish **renamable serialized labels** from **identity-defining input bytes**. A length-preserving prefix replacement (CVA/REL both three characters) does not prove stable checksums, hashes, source identity, provenance or byte-offset bindings;
- define a canonical format/version boundary with explicit typed fail-closed rejection of the previous format at normal open, consistent with the current-only hard-cut policy;
- migrate all canonical REL and PHY development files **offline onto disposable copies**, preserve/verify exact owner identity and required stable external identifiers, all historical/payload/proof/clock/reference states, and independently reopen the promoted corpus before committing corpus replacement;
- avoid fabricated ID equivalence. If a legacy domain-separated hash cannot be removed while preserving the ID or exact references, that is a **blocking explicit architecture/storage choice**, not a reason to silently reassign identity or disguise old bytes as renamed constants;
- preserve internal and external consumers' expectations, including Warlock/Hermes, CLI, integrations, file paths and documented public interfaces, through a deliberately coordinated hard cut;
- delete the temporary migration/conversion machinery and old normal readers once the canonical corpus is verified, with no permanent `CVA*` Rust constants, aliases, dispatch or test fixtures pretending old formats are current.

This naming ADR originally deferred the format version and byte-level conversion. [ADR 0048](0048-rel-current-format-and-stable-identity-domains.md) now selects current-only **2.1** with a REL header and typed offline conversion, while preserving versioned deterministic hash-input bytes. The pre-existing six-file CVAFRS07 acceptance gap remains an **implementation prerequisite**: it must be recovered and measured before converting the corpus.

## Migration/verification gates

1. **Before code cuts:** finish the owner-session architecture and complete a symbol/format/deterministic-ID audit. Capture the present source and ABI/API reference inventory; separate code naming from serialized identity.
2. **Architectural implementation:** migrate behavior to one canonical `RelSession` and explicit workflows with new names, keeping tests authoritative; no token-only rename of the existing 35-field `Cva` aggregate as completion.
3. **Wire design and offline conversion:** choose and approve canonical new marker/version and identity rules, convert copies, compare full retained owner history, timestamps, typed references, query/retrieval outputs and stable identity, then promote only verified current files. No legacy normal-read fallback.
4. **Consumer hard cut:** move Rust/CLI, tests, examples, public API and actual Warlock/Hermes integration call sites together at a controlled integration gate. A deliberately red intermediate build is permitted only when the cut plan specifies local checkpoints and restoration criteria.
5. **Final checks:** `cargo fmt --check`, workspace/CLI locked checks and tests, exact current-corpus reopen, persistence/history/import/recovery/freshness/retrieval invariants, format mismatch and no-mutation negative tests, consumer conformance, performance baselines, and a case-insensitive zero-CVA scan across tracked active Rust source and generated Rust.
6. **Retire old names:** no `Cva`, `CvaError`, `cva_*`, old unretired magic values or private compatibility alias in the final compiled code. Historical documents may record what existed.

## Consequences and alternatives

- **Selected:** comprehensive canonical REL naming with a verified hard-cut strategy and storage-format consequences explicitly accounted for.
- **Rejected:** rename only the public type and leave `Cva` internally. It perpetuates confused implementation ownership.
- **Rejected:** mechanically rewrite CVA-prefixed on-disk magic or hash domains with search/replace. It would risk making existing files unopenable and corrupting identity/provenance.
- **Rejected:** keep both names indefinitely behind aliases and wire compatibility readers. It creates two mental models and defers the user-requested cleanup.
- **Resolved by ADR 0048:** new current-only 2.1 wire signatures and a one-time typed converter; opaque stable identity-domain preimage bytes are preserved explicitly. Regenerating externally visible IDs remains rejected without a separate approved rekey.

This naming decision does **not** by itself approve the recommended storage architecture option or authorize Rust implementation changes.
