# Multiplexing P5-E E5 acceptance — 2026-10-08

Parent: [P5-E specification](multiplexing-phase5e-live-runtime-surface-2026-10-06.md)
Wave: [Next execution plan](multiplexing-phase5-next-parallel-execution-plan-2026-10-07.md)
Branch: `feat/multiplexing-p5-live-runtime`
Prior draft: `79a2d3424d821fed85ced6a833bb70e7c6677457`
Accepted N0 base: `999fbf7d85c9f892a1bf979293588033bcc1484b`

## Purpose

Close the E5 local functional acceptance gaps with existing owner mechanisms and record the exact supported contract, verification evidence and remaining convergence gates. This supersedes the native receipt, original-token cancellation and E scenario requests in the [earlier draft record](multiplexing-p5e-live-runtime-execution-2026-10-07.md).

## Overview

The recipient adapter still shares one GatewayRuntime and verified InstanceRuntime. It owns a viewer, one P4 stream recipient and a presentation flag for delivered resync status. It creates no Cva, journal, semantic queue, branch coordinator, worker or capability router. The P3 generation owner supplies the missing narrow cancellation/tool/file seams; Archive/Echo/file storage and P4 distribution retain their existing authority.

| Gap | Completed behavior | Authority |
| --- | --- | --- |
| Durable new session | Immutable first turn is journal-accepted at revision zero with no parent; exact retry returns the same receipt. Archive branch creation follows existing asynchronous processing. | P3 journal/coordinator |
| Replacement reconnect | Caller-held LiveResumeToken names owner, conversation, branch, operation/message, epoch/cursor and checkpoint. Fresh session validates identity and rejects stale epochs/future hints; replacement starts at a current full prefix without replay. | Original instance admission plus P3/P4 |
| External cancellation | Capture CancelSharedGeneration at the original session mount/fence/grant generation; release admission before provider I/O; revalidate the captured token before durable acknowledgement. | P3 cancellation plus existing router |
| Native tools | Trusted callback accepts exact generation-scoped Echo sequence, invocation/result identity and ordering. Exact retries are unchanged; conflicting or out-of-order events fail. Shared source validation rejects malformed historical Echo projections. | P3 validation and immutable synchronized Echo |
| Native generated files | Final assistant turn atomically ingests ordinary embedded attachments under exact accepted principal/generation source. Typed FileId/content identity and ordered attachment receipt survive reopen; changed retries fail. | Existing Archive turn/file ingestion |
| Diagnostics | Fresh capability/worker facts and conversation pending/Unknown/cancellation counts; delivered resync status is recipient-local. Error/diagnostic DTOs omit user content and provider details. | Existing owner observations |

## Contract and limits

Close/disconnect removes delivery only. Accepted work survives recipient replacement, detach and runtime restart. A moved session keeps the same opaque recipient; a different authenticated instance opens its own session. Original sessions cannot be revived by regrant or remount.

Pending host cancellation requires the existing linearizable cancel/dequeue contract. A Running host operation returns durable CancellationRequested rather than a fabricated terminal result. Unknown ownership remains held through capability withdrawal/reconnect; E never replays it through Native. The controlled host fixture proves E behavior against the existing full contract, not production provider conformance.

Tool sources accept at most 256 Echo events and 1 MiB aggregate content. Generated finalization accepts at most 64 files and 8 MiB total bytes. Existing exact-source Echo reads materialize their source before the adapter checks bounds; this does not claim a preallocation bound for arbitrary historical Echo content. Tool events are scoped historical receipts, not a generalized semantic log or permission to execute a tool. New callbacks are trusted embedding/service operations and must not be mapped to untrusted owner-ID transport mutations.

Artifacts are visible only after native completion and only through the finalized assistant's exact attachment receipt. An ancestor, project path or host payload cannot supply that identity. External host-specific tool/artifact receipts and project-file provenance remain with their corresponding owners.

## Scenario acceptance

The final focused suite contains **17 live-runtime unit scenarios**, plus **2 public consumer scenarios**. Existing foundation tests remain part of the unrestricted root run.

| Scenario | Evidence |
| --- | --- |
| Two adapters/instances, one REL | Shared durable admission, identical checkpoint delivery, independent recipient close/resync |
| New session and no duplicate writer | One first-turn receipt and one native generation under repeated admission/start |
| Transport move/replacement | Recipient move preserves position; replacement resumes full latest prefix and produces no replay |
| Stale authority/cursors | Old grant cannot revive; stale epoch, foreign identity and future delivery/checkpoint hints reject |
| Tool/file ordering | Result-before-call, wrong source, reused/conflicting sequence, late new event and changed file retry reject |
| Durable receipt recovery | Public consumer shuts down and recreates GatewayRuntime, remounts REL and verifies exact tools/files, retry receipts and no second work item |
| Pending/Running cancellation | CancelledBeforeStart and cooperative RunningCancellationRequested remain distinct |
| Revocation during provider callback | Regrant proceeds while callback is blocked; old-token acknowledgement rejects; durable intent survives for fresh recovery |
| Downgrade/Unknown | Host ownership stays held; reconnect does not hand off or publish a native substitute |
| Bounds and historical malformed data | Oversized/new work rejects, exact accepted retry succeeds; generic malformed Echo is not accepted as valid tool order |
| Privacy and ownership | Diagnostics/errors redact bodies/provider detail; source guard rejects raw owner handles, escape operations and another gateway |

## Verification

| Gate | Result |
| --- | --- |
| Root formatting and locked check | Passed |
| Focused live-runtime suite | 17 executed, passed |
| Unrestricted root tests | 1,116 unit and 130 integration tests passed, including 2 public live consumers |
| CLI formatting, locked check and tests | Passed; 19 tests |
| P2/P3 enforced ownership and P3 fixtures | Passed; 7 fixtures |
| P4 coverage guard | Passed; 71 fixtures, 151 public APIs classified |
| Pitlord policy validation | Passed; 15 areas, 6 rules |
| Documentation structural and change-impact checks | Passed |
| Diff whitespace check | Passed |
| Fresh architecture refresh | Incomplete: Lexicon indexing timed out after 1,200 seconds (exit 124), before Arcana sync and Pitlord enforcement |

The source matrix passed on the final Rust implementation. The architecture refresh was attempted and its subprocess tree terminated on timeout; policy validation and owner guards do not substitute for fresh graph enforcement.

Logs are retained under `target/e5-verification/` in the E worktree; this record carries the reviewable gate results. Existing compiler and MSVC linker warnings remain. The temporary resume re-export warning from the first matrix was removed by keeping the contract DTO beside the other live types.

## Remaining convergence gates

The native E contract and local scenario acceptance can be imported into N3. Named root exports remain a single D/convergence-owned cut. Export candidates include the existing live adapter/session/status/checkpoint types plus LiveResumeToken, LiveToolEventKind, LiveToolEvent and LiveGeneratedArtifact.

N3 still needs accepted E/F/G core convergence, B/C execution-only provider integration, G-external acceptance, then final Reliquary/Arcana pins and Warlock verification. The E full-contract host fixture does not replace those gates or certify a release.

## Documentation impact

- Inspected: architecture, invariants, storage format, maintainer map, current limitations, roadmap, API, E specification/execution and next-wave plans; existing Archive/Echo ingestion contracts.
- Updated: this acceptance record, canonical current-scope/owner/API/storage notes, E specification status, historical draft continuation link, next-wave checkpoint and documentation index.
- Not affected: REL/PHY/container/journal codecs, rollback/history, configuration and CLI command behavior.
- Compliance check: structural and change-impact checks against origin/main passed.
- Known documentation gaps: final named exports and production execution-only consumer acceptance are recorded as convergence work.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: E owns recipient adaptation; P3 validates captured cancellation and native tool/file callbacks; Archive/Echo/file owners persist the same existing source receipts; P4/B retain delivery/routing/workers.
- Pitlord enforcement impact: no area/rule changes; source guard scans every production interaction_live module and allows only exact classified cancellation/finalization delegates.
- Other verification impact: final root/CLI matrix, focused/public disk-reopen contracts and P2/P3/P4 ownership guards.
- Known architectural gaps: fresh Lexicon indexing timed out after 1,200 seconds, so refreshed Arcana/Pitlord enforcement remains unverified. Named convergence exports and production provider integration remain separate.

## Related docs

- [Architecture](architecture.md)
- [Architectural invariants](invariants.md)
- [Storage format](storage-format.md)
- [Rust API](api.md)
- [Current limitations](current-limitations.md)
- [Maintainer map](maintainer-map.md)

## Notes

Local E acceptance is bounded by the evidence above. It is not final Warlock migration, a new generalized event store, or multiplexing release/stress acceptance.
