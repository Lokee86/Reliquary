# V05 and V06 continuation

V05 is accepted at `62dbdcab`. Its final evidence is recorded in [V05 vector publication and bounded context](v05-vector-context-acceptance-2026-11-05.md). V06 and G2 remain open.

V05 adds owner-local profile and Memory vector publication for REL and PHY. The publication path rechecks active profiles, revisions, body identities and exact existing row bytes. An interrupted packed payload write can be resumed through its content identity. Gateway captures sources before calling the embedding endpoint and rechecks authorization before publication. Archive generation preparation now uses retained fragment definitions and the Archive clock. Provider calls use bounded batches, and publication requires complete coverage of the current fragment population.

Context assembly uses the host's conversation view and separately authorized Memory and repository evidence. Compaction checkpoints are local to the host workflow and bound to their source scope. Verification must cover changed transcript prefixes, grants, source revisions and final request budgets.

The combined worktree passed 495 tests across 47 suites, locked offline metadata and workspace compilation, formatting, whitespace, and the architecture guard with 23 packages and 76 production edges. The Gateway integration fixtures cover REL and PHY vector publication, Archive generation reuse on reopen, host-owned context with an empty REL transcript, and repository grant revocation. Archive vector preparation checks retained fragment definitions when it reuses rows. Independent owner reopen still needs to enforce this source proof for every generation before V05 acceptance.

V06 adds historical Memory reads at an explicit owner cut and native import writer operations with an exclusive epoch fence. Remaining acceptance work includes an end-to-end native import coordinator with durable scoped Freshness settlement and validated completion, and verified-copy pruning, rewrite, reconciliation and promotion. The history and import seams alone do not close V06.

The scoped import writer now shares native branch advancement and retry validation. Retrying an accepted ancestor after its branch advances leaves the branch and journal intact. Independent REL reopen binds each Archive import receipt to the owner epoch open at its physical position, checks its source namespace and format, and compares the receipt digest when a completion exists. Orphan receipts and receipts that precede their matching epoch fail owner admission.

The current V06 worktree passed 496 tests across 47 suites. Formatting, whitespace and the 23-package, 76-edge architecture guard passed.

REL and PHY prefix reconciliation now select a verified copy source only when one physical history is an exact prefix of the other. Comparison streams frame bytes with a fixed buffer. The copy uses the selected cut's existing identity and digest checks, then reopens through normal owner admission. Both input paths remain intact. Divergent histories, transformed pruning and replacement promotion remain outside this implemented path.

## V06 native-import continuation — November 5, 2026

The normalized source-to-owner workflow now stages turns, import-priority Episodes and an Archive mapping receipt under a durable exclusive epoch. Scoped import workers can claim and publish authenticated grouped completions without removing the fence for ordinary writes. Once all mapped source turns have completed import-priority Episode work, the owner can persist a Freshness checkpoint bound to the epoch start, receipt digest, physical prefix and owner-derived score projection, then append a separately durable completion marker. Reopen checks marker order and prefix binding. The PHY admission guard rejects the new REL-only settlement marker.

The new application integration test covers restart after staging, incomplete-work refusal, exact source retries without extra frames, import-scoped work completion, restart between Freshness settlement and final completion, completed-import retries and independent reopen. At this checkpoint, the affected application and owner-session suites passed 77 application unit tests, 91 owner-session unit tests and 6 owner-session integration tests (174 total). The 23-package/76-edge production architecture guard passed.

This verifies the native-import *durable workflow and checkpoint*; it does **not** establish parity with the reference implementation's full historical Freshness attribution and propagation model, which still requires G2 evidence. V06's maintenance/rewrite, divergence handling and copy promotion are likewise pending. The native-import portion of V06 is committed as `cee2b807` (`Implement resumable native import and durable Freshness settlement`). The remaining history, verified-copy maintenance and reconciliation changes are still uncommitted. V06 itself and G2 remain open.
## V06 history and reconciliation checkpoint

Owner-cut Memory reads reject future versions and distinguish a missing Memory, a visible revision and a visible tombstone. Pruned bytes stay absent at older cuts. REL and PHY prefix reconciliation independently admit the prefix owner and reopen the selected extension as a separate copy. Divergence reports the common frame count and leaves both inputs intact. This path accepts exact extensions and refuses divergent semantic merges.

Terminal Memory pruning uses the current retention manifest to verify retained frames and tombstones. Canonical REL and PHY writers refuse transformed snapshots because the current admission proof requires the manifest to end the file. Broader domain pruning, policy-driven rewrite and mutable replacement with recovery remain open. A mutable promotion needs an append-safe proof that continues to validate the pre-transform cut and prevents later publications from referencing unavailable payloads. V06 remains in progress and G2 remains gated.

The locked offline workspace suite passed with Windows hard-link and local HTTP fixture access enabled. Formatting, whitespace and the 23-package, 76-edge architecture guard passed.

## V06 verified snapshot publication checkpoint

Terminal Memory pruning now covers shared bodies when every referring Memory is explicitly selected and terminal. The unchanged retention manifest identifies each removed body once, while the retained frame digest binds all referring histories. Independent admission checks that the referring histories are terminal, the body has no vector references, and the retained prefix uses supported frame families. Canonical REL initialization records are supported alongside PHY initialization records.

Pruning streams frames into a sibling staging directory, syncs the transformed file and independently reopens it through owner admission. Publication creates the destination name atomically on a filesystem that supports hard links. An existing destination blocks publication and the original remains in place. A process interruption before publication can leave a staging directory. The published snapshot remains read-only. This does not implement replacement of an active owner, broader non-Memory pruning, policy-driven history rewrite, or semantic merge of divergent histories. V06 and G2 remain open.

Verification passed for 504 locked offline workspace tests, including REL and PHY shared-body pruning, independent reopen, and destination-collision preservation. Formatting and the 23-package, 76-edge architecture guard passed.
