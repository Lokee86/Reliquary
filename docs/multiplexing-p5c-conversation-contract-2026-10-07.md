# P5-C canonical conversation contract

Status: implemented in the dedicated `feat/multiplexing-p5c-conversation-contract` worktree; full verification is time-boxed and must be rerun before merge.

The GatewayRuntime surface is the hard-cut boundary for Warlock. Every method takes the exact `InstanceRuntime` and REL owner where owner-specific access is required. Reads use `ViewRel`; durable edits use `ManageRel`; conversation cursor admission uses `SubmitConversation`.

| Operation | Canonical method | Grant | Durability/visibility |
|---|---|---|---|
| Open conversation cursor | `open_conversation_for(instance, owner, conversation, resume_from)` | `SubmitConversation` | runtime-managed cursor |
| Conversation summaries/transcript | existing `conversation_summaries_for`, `conversation_transcript_for` | `ViewRel` | authoritative Archive read |
| Branch enumeration | `conversation_branch_start_node_ids_for` | `ViewRel` | authoritative Archive read |
| Title edit | `set_conversation_title_for` | `ManageRel` | `with_rel_mutation`, synchronized gateway publication |
| Echo read | existing `echo_events_for`, `read_echo_source_for` | `ViewRel` | canonical Echo store |
| Echo write | `put_echo_event_for` | `ManageRel` | `with_rel_mutation`, synchronized publication |
| Compaction read | `conversation_compactions_for` | `ViewRel` | canonical Archive compaction records |
| Compaction write | `put_conversation_compaction_for` | `ManageRel` | `with_rel_mutation`, synchronized publication |
| Source attachments | existing `files_for_source_for`, `project_file_ref_for`, `file_bytes_for` | `ViewRel` | canonical Archive/file records |
| Memory provenance | existing `read_memory_provenance_for` and external resolver | `ViewRel` | owner-checked source resolution |

No mutable `InteractionRuntime` is exported. No private physical Echo/Archive store is accessed. Existing legacy host methods remain compatibility-only and are not part of the Warlock hard-cut table.

Verification attempted: `git diff --check` passed. `cargo fmt --check` and `cargo check` exceeded the short execution window in this environment; do not treat the test suite as green. Focused P5-C authorization, durability, and stream-visibility tests remain a follow-up if the current branch’s existing gateway fixtures are insufficient.

Documentation impact:
- Inspected: architecture, invariants, storage format, maintainer map, limitations, roadmap, version-history plan, branching decision, and current gateway/legacy implementations.
- Updated: this contract table.
- Not affected: storage format and ownership rules.
- Compliance check: `git diff --check` passed; docs policy not completed in the timebox.
- Known documentation gaps: focused fixture names and final verification results require completion before merge.

Architecture impact:
- Standards added or changed: none; this is a canonical API hard-cut seam.
- Ownership or boundary impact: GatewayRuntime now owns authorized Archive/Echo/compaction mutation admission.
- Pitlord enforcement impact: existing `with_rel_mutation` and exact-grant checks are reused.
- Other verification impact: full fmt/check/test and source guard remain required.
- Known architectural gaps: active-session selection is still represented by existing runtime/session mechanics and is not duplicated here.
