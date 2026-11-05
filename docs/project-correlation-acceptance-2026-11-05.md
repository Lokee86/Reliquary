# S08 Project Environment acceptance

S08 requires an independent Git/Lore revision and file-correlation boundary: VCS remains source authority and REL retains only proven correlations. Its F04 prerequisite is already accepted.

## Acceptance matrix

| Requirement | Executable evidence |
| --- | --- |
| Independent repository authority | Existing Project Environment tests cover exact historical Git bytes, repository identity, nested project paths, current revision, protected snapshots with an unchanged user index/HEAD, and idempotent Lore checkpoints/history. New Application tests admit both Git and Lore references and reread the original VCS bytes after later edits. |
| Source-authorized correlation | Gateway requires a host-configured repository grant matching principal, REL owner and repository. Preparation verifies exact revision/path/hash outside owner locks. Its opaque proof retains the revocable grant and Instance binding through finalization. Wrong owner, principal, repository, path and hash are rejected; revocation after preparation leaves the native queue pending. Later attachment hydration requires the same grant before and after VCS I/O; a REL grant alone cannot read revoked project bytes. |
| REL holds proven references | Application reuses native turn publication and adds Archive-owned attachment descriptors containing repository/revision/path, content identity and length. Archive stores no copy of the project file bytes or VCS ancestry. |
| Retry and reopen | The existing native queue/Archive receipt boundary is reused. Exact retries preserve the accepted reference; a different file cannot replace it. Reopened owners resolve the original bytes from VCS despite working-file edits. Committed retries also work on read-only mounts. |
| Dependency and ownership boundaries | Project Environment performs VCS I/O; owner-session owns Archive publication; Gateway owns grants and queue arbitration. Three existing immutable Archive value types are reexported through owner-session. The added Lore dependency is test-only. |

## Verification

S08 is accepted. The final locked offline workspace suite passed **480 tests across 47 suites**, with zero failures and local loopback enabled for mock HTTP fixtures. Locked metadata, workspace compilation, formatting and whitespace checks passed. The architecture guard passed with **23 production packages and 76 approved production edges** (13 test-only internal edges).

G2 remains dependent on V05 vector/context integration and V06 import/history/maintenance acceptance.
