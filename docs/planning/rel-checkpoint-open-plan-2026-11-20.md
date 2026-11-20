# REL checkpoints and Hermes mount implementation plan

Parent index: [Planning](./INDEX.md)

## Purpose

Implement the [checkpoint and mount specification](rel-checkpoint-open-spec-2026-11-20.md) from Core `7611f9a4` and plugin `a93efd3`. Prove the actual mounted startup path on the existing 28-day pair and on a representative year-scale REL.

## Overview

The path-mount change can land on its own with the current replay. The checkpoint work moves the leased REL admission and historical readers together to one canonical path. Within that work, the Core subgraph may be temporarily incomplete while replay and history consumers move. Restore the build and verify the production entry points at integration. The persistent compatibility boundary is existing native 2.1 files and the public detached snapshot API.

## Work packages

| Work | Depends on | Deliverable and exit check |
| --- | --- | --- |
| 1. Mount the owner by path | Baseline | Add the trusted path mount at [Root Gateway](../../src/host.rs) and [runtime Gateway](../../crates/reliquary-runtime-gateway/src/gateway_mounts.rs). Check an expected ID before installing the mount. Change Hermes Registry bootstrap, configured startup and live Apply to stage the actual mounts once and derive or check their identities. Migrate the standalone CLI local host. Remove the successful startup double-open path and the separate full bootstrap verification. Exit with open counts and existing role-switch evidence. |
| 2. Establish canonical replay | Baseline | Split the [Container](../../crates/reliquary-container/src/lib.rs) leased header from scan and clock reconstruction. In [owner-session](../../crates/reliquary-owner-session/src/session/replay.rs), move the owning decoders and cross-domain validations to the common replay path. Inventory each accepted carrier's continuation fields and cut lookups before moving it. Before moving replay, freeze the existing full-verification report and selected current and historical observations for the pinned 28-day copy with the [D02 reader](../../tools/d02-parity/reader/README.md). Compare both new admission paths to that evidence and preserve the explicit audit path. Exit with cold replay equivalence for native 2.1, including a grouped publication and a reservation later bound by a publication. |
| 3. Page history from the owner's file | 2 | Introduce the on-disk historical indexes and resolve their payloads through the same leased Container. Switch the store readers and proof lookups to the exact indexed cut. Move eager body and vector hydration out of admission. Keep detached snapshot behavior at its public boundary. Route ordinary CLI REL info and bounded history reads through the scoped Root reader. Exit with exact history parity, a scoped read trace, and a detached-read-then-writer contract check. |
| 4. Restore and publish checkpoints | 2 and 3 | Implement the private cache manifest, prefix verification, typed continuation restore and suffix replay. Add durable initial publication and refresh from the existing writer boundary. Check the fallback and repair rules. Exit with a checkpoint hit on the copied 28-day REL, unchanged and appended-owner parity, and exact refusal and recovery for incomplete tails or incorrect checkpoint evidence. |
| 5. Integrate the plugin and measure | 1 and 4 | Vendor the final Core source at its exact commit using the existing provenance process. Build an isolated native Hermes host. Measure Core leased open and signed Hermes startup separately from full verification. Check role and session bindings, a durable REL write after restart, and historical inspection. Record startup numbers, peak working set, bytes and frames read, and refresh cost. |

Work packages 1 and 2 are independent. Package 1 can use the existing full replay and the same path-mount API that package 4 will accelerate. The final package confirms that startup, Apply, configured reopen, and the standalone local host each use the canonical mount path.

## Focused verification

1. **Replay and lazy reads.** Extend the existing Root public API history and owner-session replay fixtures. Compare full verification with a checkpoint at a clean cut plus a valid suffix that crosses a physical clock reservation and contains a grouped change. Read an older Memory revision and its exact Archive provenance, and check the affected Graph or vector projection. Cover recorded unavailable or tombstoned history in the same fixture. Assert the cut and owner results match. Count old payload decodes on the unchanged hit and during one bounded historic read. Reuse the existing detached Root consumer seam to prove that opening a detached snapshot still allows a later writer mount.
2. **Bad cache and interrupted write.** Use one parameterized cache check with missing, stale or damaged checkpoints, plus a foreign entry for a file with the same owner ID. Edit old bytes without changing the length to exercise the prefix match. Reuse the Container tail-repair fixture on a checkpointed copy. A torn append must fail admission, and explicit repair must restore the exact accepted bytes and owner identity. Interrupt one checkpoint publication after the REL sync and prove the ledger and prior usable checkpoint recover.
3. **Actual Hermes mounts.** Extend the existing native registry and signed HP03 Setup/restart fixtures. Count semantic admissions for first startup, configured restart, and Apply across General, project, and PHY roles. Replace one reviewed candidate with a different owner and prove rejection before the new mount or grants become visible. Use the existing switch tests for stale generation, old-session pinning and revoked PHY access. On the copied 28-day pair, create a REL Memory and read the same ID, revision and body after a fresh native process.
4. **Scale on real entry points.** Reuse the G5 timing and custody harness with the normal leased open and signed attach paths. Use verified 28-day copies and one valid corpus of approximately 1 GiB with representative native history. Match the larger corpus to a smaller control with the same current projection size and vector dimensions. Generate a matched pair through the existing native writers if suitable retained files are unavailable. Run three fresh processes per case after one file-cache warmup, on the same Windows machine with builds idle. Record the initial full rebuild once. Compare steady checkpoint-hit startup and peak RAM, and record the separate prefix hash and suffix replay costs. Use these copied workloads for the feature acceptance.

## Acceptance budgets

These are local, warmed-filesystem targets on the reference Windows machine. Apply them to median elapsed time over three fresh processes with a valid checkpoint and the standard configured host profile. Use the 28-day PHY with both benchmark REL sizes.

| Workload | Core leased REL open | Configured Hermes attach |
| --- | ---: | ---: |
| 28-day pair | 1.5 s | 2.5 s |
| Approximately 1 GiB of REL history | 3.0 s | 5.0 s |

For the year-scale fixture and its smaller fixed-current-state control, keep peak Core working set within 1.5×. An unchanged hit must decode zero historical payloads. A valid suffix may decode the frames it contains and the indexed prior facts it requires. Use frame and page counts from the first bounded history request to confirm it touches only the requested history ranges. Keep wall-clock assertions in the opt-in performance run. Record the running memory cost of the current projection and the observed cold rebuild separately.

## Integration and evidence

Run the existing locked offline build checks for Core, the CLI and the vendored plugin host once at integration. Run the affected replay, storage, Root and native registry suites, the signed HP03 case, and the repository architecture, formatting and docs checks. Use the [development workflow](../development/workflow.md) for the baseline commands. Check production call sites for the retired bootstrap and mount chain and for whole-file replay reached from a successful checkpoint hit. Confirm that explicit verify and maintenance still enter the common replay from the beginning.

Update the canonical storage, owner-session, Gateway, API and CLI docs and the plugin's source provenance and verification record alongside the implementation. Keep the perf receipt with source and binary pins, corpus hashes, copy custody, method, phase timings and failure cases under [acceptance evidence](../acceptance/INDEX.md). Preserve the previous G5 results for full verification.

## Related docs

- [Specification](rel-checkpoint-open-spec-2026-11-20.md)
- [Owner sessions](../architecture/owner-session.md)
- [Development workflow](../development/workflow.md)
- [G5 open and recovery results](../acceptance/g5-owner-open-and-recovery-2026-11-20.md)

## Notes

Use disposable corpus copies and existing synthetic providers. The local acceptance ends with the pinned native host, workload measurements and recorded proof.
