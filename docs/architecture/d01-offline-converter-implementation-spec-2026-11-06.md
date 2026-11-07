# D01 — isolated 2.0 to 2.1 converter

Parent index: [Architecture](INDEX.md). Parent plan: [post-G4 execution](post-g4-parallel-execution-plan-2026-11-06.md).

**Status: implementation-ready specification; D01-T01 has an isolated synthetic implementation checkpoint in progress; overall D01 remains unaccepted.** Canonical acceptance prerequisites remain G4 and F03. Implementation on synthetic inputs may start after PG-C01; conversion of the six authentic sources requires F03-V04. D02 is independent and must not be used as D01's self-certification.

## Purpose

Produce current-format owners without losing stable identities, provenance, accepted history, privacy or operational authority. The mature old storage cannot be made current by replacing a prefix or replaying text through Insomnia: both approaches can change hashes, clocks, IDs, grouped receipts and historical observations.

## Overview

Use an isolated offline source decoder and target writer. The source decoder understands exactly the verified current old 2.0/FRS07 families; the target writer uses the actual accepted 2.1 codecs. Neither is a fallback in the normal product. Originals are read-only; each output is a create-only candidate on a separate path. Full independent comparison belongs to D02; only D03/G5 may stage and activate a generation.

## Target architecture and test seams

Preferred implementation boundary:

`old-source-reader (pinned FRS07 snapshot) -> hashed typed transfer stream -> target-2.1-writer (rebuilt codecs) -> candidate owner`

The two binaries run in separate processes/workspaces, avoiding conflicting old/new Root packages and any production dependency on the reference. The transfer stream is temporary, versioned, strictly validated and private. It carries semantic records with original clocks/identity plus physical-source coordinates required for relocation. It is NOT the independent F03 expectation.

Keep converter tooling outside the 23-package production DAG, in a dedicated worktree/tooling directory recorded by D01-T01. A tiny coordinator invokes old/new binaries, enforces custody and assembles receipts; it does not own semantic policy. Map each family to its actual owner codec. Existing `RelSession`/`PhySession`, grouped publication/replay, historical proofs and verified-copy facilities remain the validation authority. Public `Rel::create`, Insomnia inference and ordinary ingest are not lossless-history writers and must not be misused as such.

If preserving an accepted old value needs a current owner import/encoding primitive, add the narrow value-based operation at that owner's legitimate boundary, with regression tests. No unrestricted mutable store bag, generic restore framework, Root back-edge or long-lived compatibility feature. A source state not representable in current 2.1 is a specific conformance gap: retain originals, record the exact field/history obligation and resolve the target owner before accepting it. Do not silently omit it, reset its history, or change the file version without a separate format decision.

## Implementation decisions

### Classified record map

Start from actual source-frame inventory plus the earlier marker inventory. Produce `codec-coverage.json` with each observed carrier, nested family, accepted version, source decoder, target encoder, semantic identity behavior, physical digest changes, relocation obligations, retained proof and test. Every accepted source frame is accounted for. Unknown/future/ambiguous carriers stop conversion; retired invalid fixtures are not an allowlist. Derived caches may be rebuilt only with proof that they carry no accepted authority; retain source events, profile IDs and receipts.

Stage 5's old-prefix mapping is a naming decision, not permission to assume the rebuilt payload layouts match the old ones. Inspect current 2.1 writers and replay contracts. In particular, original grouped Insomnia/Dream, FRS07 import-settlement authority and accepted version allocation need typed translation rather than detached row insertion.

### Stable identity and time

Preserve owner UUID/scope, history generation, all stable semantic IDs, original body/provenance identities, local/global versions and transaction times. The six F03 identity algorithms keep their original binary preimage domains. Do not re-run inference, Dream classification, Freshness propagation or embeddings to recreate missing state. Recompute only encoding-bound checksums and transport digests; any externally stable raw-byte-derived identity must have an explicitly reviewed mapping/resolution before output is admissible.

### Two-pass references and grouped records

Inventory the accepted record/reference closure before writing. Allocate/encode target records with recorded old-to-new physical locations; resolve all forward/back references through a complete relocation map. Retain ordered grouped publication boundaries and version reservations. Validate payload hashes, offset/range bounds, owner scope, endpoint existence, profile generation bindings, and referenced unavailable payload descriptors. A dangling or cross-owner physical reference is a failure, not permission to substitute current content.

Freshness source authority and destination framing are separate values. Imported Dream populations and accepted-use receipts keep original authority; re-encoding must not credit events twice. Preserve incomplete/held work only where the target has an exact supported state; otherwise stop explicitly rather than marking work complete.

### Publication and retry

Use a unique candidate directory and exclusive writer; refuse existing output or source-alias destinations. Hash and stat source before/after. Sync complete candidates, close all writer leases, open through normal current-only owner admission, and validate immediate structural proofs before returning a candidate receipt. No-replace publication must be atomic at the single-file boundary on the selected filesystem, including collision races. Cross-file publication is not atomic and is not attempted here.

A crash leaves only an unaccepted candidate, never an active file. Resume requires the same source hash, code/schema/map digest and validated checkpoint boundary. Otherwise discard only task-owned incomplete output and restart. Retrying an already completed conversion may return its matching receipt or create a new candidate; it cannot overwrite a different destination or reuse an unverified partial file. Operational queue/host sidecars are inventoried and transformed separately only where they actually exist; unsupported pending host state remains Held for D03 reconciliation.

## User/developer outcomes

A reviewer can identify exactly how every source family was transformed, reproduce a candidate from immutable inputs, detect unknown data before promotion and hand independently readable 2.1 owners to D02. Normal runtime dependencies and current-only format admission remain unchanged.

## Tickets

| Ticket | Hard predecessors | Deliverable | Acceptance and evidence |
| --- | --- | --- | --- |
| D01-T01 — isolate and classify | PG-C01 | Tooling layout; full family map; synthetic old-format builder; explicit target primitive gaps | No production old-reader dependency; malformed/unknown carrier refuses; exact code/schema hashes and coverage tests |
| D01-T02 — preserve one complete owner history | D01-T01 | REL and PHY synthetic conversion, typed versions/lineage/Archive/Memory and nested carriers | Fixed semantic identities, source references, clocks and history match independent hand-authored cases; 2.1 normal open; original bytes unchanged |
| D01-T03 — preserve all families and relocation | D01-T02 | Graph/Entity/Ego/Freshness/Dream/import/vector/external/operational mapping and full relocation | Multi-family positive and dangling/stale/forged/mixed-format negatives; no re-inference/reinforcement; all observed families covered |
| D01-T04 — candidate transaction and faults | D01-T03 | Crash-safe create-only coordinator, receipts/checkpoints and output lifecycle | Faults before/after each sync/publication, collision races, wrong source on resume, read-only failures and disk-full simulation preserve originals and publish no accepted partial |
| D01-T05 — convert authentic verified cohort | D01-T04, F03-V04, G4 | Six separate candidates plus complete receipts/maps/structural reopen evidence | Exact pinned input hashes, no unknown families, 6/6 current-only reopen, no source mutations; independent implementation review and commit; hand candidates to D02, not activation |

## Executable command contract

D01-T01 creates and documents a disposable command with the following operations (names are target interfaces, not existing commands):

```text
converter inspect --source-root <copies> --custody <source-custody.json> --output <inventory>
converter convert --source <copy> --expected-sha256 <hash> --source-admission <file> --output <new-path>
converter resume --checkpoint <file> --source <copy> --output <same-candidate-path>
converter verify-structure --candidate <path> --receipt <file>
```

`convert` authentic-source mode requires a PASS source-admission artifact and the complete code/schema binding. Synthetic fixture mode is explicitly separate and cannot produce an authentic-source acceptance receipt. Commands never accept overwrite/repair-current/activate options. stdout is structured JSON, diagnostics go to stderr, and a nonzero exit distinguishes input/admission mismatch, unsupported family, conflict/collision, incomplete source, target mismatch and I/O failure. Do not log private payloads.

Tests run separately in old-reader and target-writer workspaces with locked/offline dependencies and one job. Re-run G4, GM/P01 and the architecture guard for any product-owner correction. Compile and test target-only dependencies to prove there is no accidental old-format linkage. Record full test counts and explicit selected/ignored tests, not a package build as acceptance.

## Completion gate

D01 passes when its reviewed typed converter generates all six structurally admissible candidates with original source custody and complete conversion receipts. This is not semantic corpus acceptance: D02 must read them independently and match the frozen F03 expectation. D01 remains provisional for any source family awaiting a target representation fix. Preserve converter source/binary provenance privately until D03 retirement; it never ships in the runtime release.

## Out of scope

Activation, customer traffic, automatic source repair, choosing new Memory content, old-format runtime fallback, migration of arbitrary unverified user corpora, schema redesign for convenience or changing retained external identities.

## Related docs

- [Shared evidence contract](post-g4-corpus-evidence-contract-2026-11-06.md)
- [F03 verification](f03-historical-verification-implementation-spec-2026-11-06.md)
- [D02 independent parity](d02-independent-parity-verifier-implementation-spec-2026-11-06.md)
- [D03 activation](d03-safe-activation-implementation-spec-2026-11-06.md)
- [Stage 5 decisions](../cva-to-rel-format-and-identity-migration-2026-11-03.md)

## Notes

Missing acceptance evidence does not prevent D01-T01..T04 on synthetic inputs. It does prevent T05 authentic conversion. The existing pre-rebuild offline converter is reference material only; do not treat it as a 2.1 implementation or link it into Root.
