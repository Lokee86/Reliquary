# Ego host integration acceptance

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Record mounted Ego publication and the outgoing request checks for the shared Core, Hermes and Mercury.

## Overview

Both hosts use Core `5e4d5b0ba1ff0b498bd025bbc87394e702f48b18`. The Windows checks use disposable profiles, synthetic owners and local scripted inference. The Mercury dependency resolved that Git revision from the isolated checkout. Both host adapters count UTF-8 bytes as a conservative bound for request packing and the saved PHY cap.

## Verified behavior

| Boundary | Result |
| --- | --- |
| Core controls | The mounted check passed exact Identity approval, stable Pin lookup, a priority-only edit, stale revision rejection, and admission-scoped writable PHY detach and reopen. The retained PHY owner check passed verified-copy provenance, refusal of foreign and malformed citations, and retained proof after a source tombstone. |
| Core worker | Core published both summary forms through the managed worker, packed all four Ego sections, reopened exact source coordinates and completion receipts, and cleared completed work. A read-only REL refreshed after 25 new settlements without a Perception route and left the REL audit cursor unchanged. A PHY refresh waited at four distinct accepted changes and advanced its verified summary after the fifth. |
| Shared audit | Five mounted checks covered complete initial paging, both bootstrap cases, independent Perception progress during an Ego storage hold, and durable handoff replay before the owner cursor advances. |
| Focused contracts | The 13 Ego, 10 shared audit and seven Ego Application checks passed. They cover Freshness-based packing, exact citation binding, point replacement and suppression, independent audit lists, pending-work replay, and the PHY compression and verification gates. |
| Hermes `9b045940` | The vendored native build and all three wire checks passed. Both native wire fixtures reached the signed service, indexed an ordinary Memory on a disposable REL, and captured Ego and Memory recall in the same outgoing request. The change was confined to the exact leading SOUL block plus the Ego package. The rest of the system prompt, prior messages, and saved history passed the preservation checks. A mismatching prefix kept the request intact. |
| Mercury `b8faf72` | The native Gateway sent an ordinary request to the local provider with the approved Identity in the persona slot, each of the other three Ego sections once, existing host blocks and prior messages preserved, and built-in notes disabled. The same fixture verified the separate local Identity approval and rejected approval from a different peer. The existing native compatibility fixture also passed explicit scoped Memory recall, private Archive search, Archive and Echo tool recovery, and Perception verification and review. |

## Reproduction

Run `managed_ego_publishes_and_reopens_both_summary_forms` with `RELIQUARY_EGO_CAPTURE_FIXTURE` set to a new directory to export its synthetic owners. The test writes `atlas.rel` and `personal.phy` after it checks the reopened exact source coordinates and completion receipts. Give each host fixture its own copy. [Mounted Core pair test](../../crates/reliquary-runtime-gateway/src/instance_perception/tests/ego_pair.rs)

## Related docs

- [Implementation spec](../planning/ego-implementation-spec-2026-11-27.md)
- [Ego product API](../public-product-api.md#ego)
- [Hermes verification](https://github.com/Lokee86/reliquary-hermes/blob/main/docs/verification.md)

## Notes

Stock Hermes v0.23.0 supplies the Context Engine hook. Its probe used a test companion from the embedding-capable Hermes SDK for hosted inference. The second probe ran Hermes `168a923` with its own registered inference worker. Hosted summary refresh and ordinary Memory recall require that SDK. The ongoing processing profile remained outside the test setup.
