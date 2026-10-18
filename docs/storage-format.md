# Reliquary Storage Format
Parent index: [Documentation index](INDEX.md)
## Domain tombstone records

`CVAMEMT1`, `CVAENTT2` and `CVARELT1` encode direct Memory, Entity and Relationship deletion, respectively. Each stores the target identity, terminal revision and owner domain version; the existing container semantic publication supplies the global version. `CVAENTT1` remains the Entity merge/retirement record with a replacement identity. Rebuild rejects later publication for either terminal Entity form, reconstructs current exclusions, and retains earlier revisions and references. These records do not authorize payload destruction.

## Ego terminal lifecycle

The existing Ego Identity/Anchor deleted-record encoding is unchanged. Replay rebuilds terminal lifecycle coordinates from each deletion record's `ego_version` and retains nondeleted revision payloads for historical reads. No extra lifecycle journal or clock is persisted. A later revision for an already deleted Identity/Anchor is rejected. Legacy single-Identity framing continues to rebuild the active `Default` Identity.

## Scoped Historical Freshness settlements

`CVAFRS05` is the scoped-epoch variant of the existing `CVAFRS04` binary settlement. It retains, in order: owner UUID; import, start, corpus, manifest, input and two pinned-policy digests; base/activity/global/input-byte/Archive/full-Graph/Memory-Graph u64 cuts; sorted cohort IDs; sorted sparse replacement records; and sorted superseded event IDs. Counts and byte-string lengths are little-endian u32, as in CVAFRS04. Before the final SHA-256 checksum it adds one length-prefixed canonical UTF-8 JSON `ImportEventScope`, followed by a 32-byte immutable-prefix evidence digest.

The scope fields, in declared order, are owner, import_id, start_evidence_digest, born_memories, affected_recipients, source_nodes, start_prefix, freeze_prefix, base_cut and freeze_cut. UUID/digests/Memory IDs are integer-byte arrays; source Nodes are sorted conversation/Node pairs. Memory sets are strictly sorted and unique. Unknown fields, noncanonical encodings, trailing bytes, bad checksums and inconsistent cuts/cohorts reject. Cva verifies the immutable-prefix digest again on reopen. Start prefix is the exact ImportStart frame offset; freeze prefix equals input_bytes. The input digest checks every byte at commit-time freeze. The separate domain-separated SHA-256 immutable-prefix digest covers each non-compaction frame's 16-byte ObjectRef coordinates and payload through the freeze boundary; mutable `CVACMP1` allocations are excluded so later compaction cannot invalidate accepted scoring. Cva additionally rederives the exact start marker, receipt Nodes and import births within the frozen prefix.

CVAFRS05 permits an empty replacement/cohort and equal base/settlement cuts, including zero, only with authenticated epoch evidence. No admission record or activity anchor is synthesized. Replay suppresses superseded import identities, preserves legitimate pre-start/interval history already folded into replacements, and distinguishes later same-cut receipts by accepted identity in the frozen prefix. Unaffected records are not replaced. Equal-cut identity checks use bounded scans rather than a retained set of all old event IDs; their cost scales with the number of such receipts and prefix length.

CVAFRS03 decoding and CVAFRS04 read/write behavior remain unchanged. Divergent reconciliation that cannot preserve ordered settlement and physical-boundary authority remains refused. Import readiness adds no durable record or codec: `ImportComplete` still binds the exact receipt, start and raw accepted-settlement digest. At completion time, the coordinator derives blockers from existing Episode/Insomnia completions, Memory records and routing metadata, Dream pass journals, and per-mention EntityResolution records. It requires import-attributed local evidence only; external PHY Memory refs are unsupported. The read-only readiness report exposes vector `NotRequired` only when there are no source Nodes and no Memory outputs, otherwise `SupportedUnavailable`, which does not certify vector completeness. Import schedules no Ego mutations. A pending import-owned Dream pass remains blocking after archival or terminal deletion; archived births without an already-pending pass do not require a new pass.

## Native import receipt record

`CVAIMPT1` is an Archive semantic record: eight magic bytes followed by UTF-8 Serde JSON `imports::ImportReceipt` schema 1. It uses the existing `CVAAREC1` Archive record-version publication. The exact JSON fields are the exported receipt DTO fields: import key, source namespace/profile, conversation identity/title, source message revision/ancestry/replacement/delete marker and native Node references, source artifact reference/hash/native FileId, and source/native branch/head/canonical facts. Bodies remain in existing Node/content/File/turn-attachment records. Unknown fields, invalid schema/key and corrupt native references fail closed.

Rebuild applies only Archive-versioned receipt records. Reconciliation republishes identical source evidence through Archive; reclamation retains published receipt backing. Older readers that do not recognize this new semantic family cannot safely open versioned receipt-bearing files. Existing files without receipts retain their format and open behavior. [Native import contract](native-import.md) owns identity/retry and snapshot semantics.

## Import epoch boundary records

`CVAIEPS1` and `CVAIEPC1` are owner-level import metadata, not semantic global-version records. Each is eight magic bytes, a little-endian u32 JSON byte length, and exact Serde JSON for `ImportStart` or `ImportComplete`; trailing or truncated bytes fail closed. Start records bind owner UUID, deterministic import ID, canonical import key, source namespace/format, time and pre-import global/Archive/Memory/Entity/Relationship/full-Graph/Memory-Graph/vector cuts. Completion additionally binds the exact start digest, Archive receipt digest, post-import cuts and Historical Freshness settlement digest.

Start admission and replay require a canonical 64-byte lowercase hexadecimal import key, bounded nonempty NUL-free source text, and an owner-bound deterministic ID. Admission validates before appending or closing the fence.

Rebuild permits at most one open epoch, requires start-before-completion ordering, validates owner and deterministic identity before store mutation, validates start and receipt digests, and requires monotone cuts and times. A normal REL has no epoch records. `Cva::complete_native_import(import_id, now_ns)` is the exclusive normal completion path. The production store append requires an opaque coordinator authority; direct append is a test-only seam. The coordinator derives exact receipt and accepted scoped-settlement evidence, records the SHA-256 of the raw `CVAFRS05` payload, and syncs the completion frame before reopening ordinary write admission. Reopen checks that the bound settlement frame physically precedes the completion frame. Existing record layouts and semantic clock allocation are unchanged.

The store codec's nonempty-digest check is only structural; coordinator and reopen validation bind the digest to the exact accepted settlement payload. Completion derives readiness at runtime from existing store evidence; this adds no readiness record or format version. Routed PHY refs return typed `Memory/Unsupported`. On uncertain append/sync errors, the next attempt rescans physical markers before retrying. Focused pending and terminal-failure recovery tests verify that reopen retains the blocker, open epoch, write fence and bytes.

## C8 embedding retention

C8 uses the existing exact Archive Fragment, PackedVectors, ArchiveVectors and VectorGeneration records without a format/version change. Each successful batch publishes a canonical FragmentId-ordered union, preserving prior rows byte-for-byte, and synchronizes its generation. Intermediate backing records without a published generation are not source-readiness evidence. Container's internal bounded-read guard limits hydration allocation without gaining semantic authority.

No durable fill report, event queue or embedding ledger is added. The ephemeral H1 vector_proof embedding_pass is not-requested for no-fill, or the frozen C7 requested-complete typed map with a digest of final fragment/bindings/availability/mode/evidence records. Only its resulting manifest identity enters CVAFRS04. Ordinary receipt framing remains unchanged; legacy CVAFRS03 decode remains supported.

## Purpose
This document is the exact reference owner for persistent records currently implemented by Reliquary Memory v2.
## P5-B persistence boundary

P5-B changes no schema or persisted journal framing. Dispatch switches/wake state and the router digest cache remain ephemeral; existing owner receipts, vector profiles/bindings and derived queues reconstruct work. Durable host intent/cancellation/terminal proof remain in the REL journal. Private vector profiles/batches synchronize before runtime visibility/wake.

## Overview
The shared container supports two typed semantic file kinds. A Reliquary `.rel` contains the full existing source/workspace owner composition: Archive source/history records, embedded and repository-backed file references, durable Project repository-revision correlations, durable interaction-stream checkpoints, turn-attached Echo execution evidence, mutable conversation-compaction state, Memories, Graph relationship state, clock-neutral Dream maintenance/pair-history records, derived Community snapshots and semantic-name records, Insomnia operational/completion records, vector backing/bindings, compatibility profiles, and vector generations. Most owners remain append-oriented; conversation compaction is an explicitly mutable variable-width owner with immediate free-space reclamation. A Phylactery `.phy` contains the narrower user-global owner set: latest-wins principal profile metadata, Memories, Graph, clock-neutral Dream maintenance/pair-history records, derived Community snapshots and semantic-name records, Packed Vectors, Memory Vectors, Compatibility Profiles, and Ego Identity/Personality/Anchor/web-synthesis records. REL files may contain Ego Anchors and one cached owner-local web synthesis but reject PHY-only Identity/Personality records. Each top-level type opens the same physical stream but dispatches and validates only its permitted owners.

## Reliquary and Phylactery file identity — implemented

New Reliquary files use a 40-byte header carrying authoritative `file_kind = Reliquary`, a zero semantic-scope discriminator, and a 16-byte durable owner UUID. Current human-facing files use the generic `.rel` extension. Organizational labels such as Organization, Project, Program, or Subproject are append-only REL metadata and are not file identity.

The existing 16-byte `.cva` header and earlier/current-length typed REL headers remain readable as legacy data. Legacy typed discriminators are compatibility input only; opening them does not make the type behavioral. `migration::migrate_file` repacks an eligible legacy source into a separate current homogeneous REL while preserving or deliberately deriving stable semantic identities.

Phylactery `.phy` uses the same 40-byte typed header with authoritative `file_kind = Phylactery`, no Reliquary scope, and its own durable owner UUID. A `.phy` is not a legacy CVA and cannot be opened through the Reliquary/CVA lifecycle. Conversely, Phylactery rejects typed REL and legacy CVA files. See [ADR 0020](decisions/0020-reliquary-and-phylactery-file-kinds.md) and [ADR 0021](decisions/0021-typed-reliquary-scopes-and-connections.md).
## Exact contract
All integers and multi-byte scalar values are little-endian.
### Container header
| Offset | Size | Field | Value |
| --- | ---: | --- | --- |
| `0` | 8 | magic | `CVA\0\r\n\x1a\n` |
| `8` | 2 | major | `1` |
| `10` | 2 | minor | `0` |
| `12` | 4 | header length | `16` for legacy CVA; `24` for earlier typed files; `40` for current typed files |
| `16` | 1 | file kind | `1=Reliquary`; `2=Phylactery` |
| `17` | 1 | semantic scope discriminator | current Reliquary/Phylactery: `0`; legacy typed REL compatibility: `1=Organization`, `2=Project`, `3=Connection` |
| `18` | 6 | reserved | zero |
| `24` | 16 | durable owner UUID | UUID bytes; current typed files only |
Physical chunks follow:
```text
u64 payload_length
N bytes payload
```
`ChunkRef` stores the length-prefix offset and payload length.

### Conservative physical reclamation

`Reliquary::reclaim_storage(output)` and `Phylactery::reclaim_storage(output)` perform a copy rewrite into a separate file while preserving the source header and durable owner UUID. The source is never replaced in place. Every retained payload is copied byte-for-byte except version metadata that embeds physical `ObjectRef` locations; Archive, Memory, Entity, Graph, and Vector-Generation references are relocated to the copied backing chunk.

The reclamation root set is deliberately narrower than future historical retention/vacuum. Published semantic history is never pruned. The pass may remove only:

- unversioned Archive/Memory/Entity/Graph/Vector-Generation backing payloads that never became published authority;
- `CVACONT1` content objects not referenced by any published Archive record;
- standalone `CVAMBDY1` bodies not referenced by a published Memory record, grouped Insomnia completion, or Memory-Vector binding;
- Archive-Vector sets not referenced by any published Vector Generation;
- Packed-Vector matrices not referenced by any surviving Memory-Vector or activated Archive-Vector set;
- conversation-compaction chunks explicitly marked free.

The rewritten output is reopened through the normal REL/PHY lifecycle and must preserve owner UUID plus all applicable semantic watermarks. This maintenance operation therefore reclaims never-published/staged backing garbage and allocator holes without requiring timeline/pin semantics. Retention-aware deletion of published historical lines remains future work.

### Global version ticket
Current ticket:
```text
8 bytes   "CVAVERS2"
u64       global version
i64       transaction_time_ns
```
Legacy ticket:
```text
8 bytes   "CVAVERS1"
u64       global version
```
Global versions begin at `1` and are semantically consecutive. Version order remains the canonical owner-local semantic mutation order. Current ordinary semantic publications persist one `CVAVERS2` ticket whose transaction timestamp records when the REL/PHY persistence layer allocated that version. `transaction_time_ns` is knowledge/system time only: it is distinct from source chronology (`source_time_ns`) and from world-valid time interpreted by Chronos.

Legacy `CVAVERS1` tickets remain readable but have no transaction timestamp; reopen does not invent one. Wall-clock historical-cut lookup therefore resolves only a contiguous global-version prefix whose included versions all have known timestamps no later than the requested cut. It does not skip an untimestamped version or a later-timestamped earlier version to reach a higher global version. This remains truthful when divergent reconciliation preserves branch transaction times that are not monotonic in the newly linearized version order. Current `CVAINSC5` completion transactions, and legacy `CVAINSC4` completions, are the atomic-range exception: they carry one explicit persistence-owned transaction timestamp for their contiguous embedded local REL Memory-version range, so those versions do not require standalone `CVAVERS2` chunks. Older `CVAINSC2`/`CVAINSC3` embedded ranges remain readable but untimestamped for transaction-time purposes. External owner-qualified Memory references and clock-neutral Memory routing metadata consume no REL global/Memory versions. Immutable backing objects do not independently consume versions.

### Durable owner identity

Current REL/PHY files store a 16-byte UUID directly in the container header. Current homogeneous RELs use canonical external owner ID `rel-<uuid>`; Phylactery uses `phy-<uuid>`. Legacy typed REL headers with an owner UUID retain their historical `proj-`, `org-`, or `con-` prefix so identity remains stable while those files are opened or reconciled. Owner identity consumes no semantic/global version ticket. Copies, moves, renames, reconciliation repacks, and format migration preserve or deliberately derive the UUID. Earlier 16-byte legacy CVA and 24-byte typed files have no header UUID and require explicit migration before owner-ID-based reconciliation.

Legacy `CVAWKFM1` / `CVAWKSP1` workspace-metadata chunks may remain physically present in old RELs, but current runtime semantics ignore them and do not write them.

### Phylactery principal profile

PHY principal presentation metadata is append-only and latest-wins. It consumes no semantic/global version ticket and never changes the header-derived `phy-<uuid>` owner identity.

```text
8 bytes   "CVAPHYP1"
u8        display_name_present (0/1)
if 1:
  u32     display_name UTF-8 byte length
  N bytes display_name UTF-8
```

The display name is non-empty after API normalization and is limited to 256 UTF-8 bytes. Reopen scans all valid profile records and retains the latest one; a PHY with no profile record resolves to `PhylacteryProfile::default()` with no display name. Display name is mutable presentation metadata only. It may be projected into the matching REL principal Entity's aliases/summary, but the principal Entity canonical name and identity remain derived from `phy-UUID`. Early feature builds briefly appended a second optional username field under the same `CVAPHYP1` magic; current readers accept and discard that trailing field, while current writers emit only the display-name field.

### REL metadata

Current REL organizational/context metadata is append-only and latest-wins:

```text
8 bytes   "CVARELM1"
u8        type-label present: 0=false, 1=true
optional  string type label
u32       dependency count
repeated  string dependency REL owner ID
```

The type label is optional display/organization metadata only and must not select Reliquary behavior. Dependency IDs are sorted and unique and represent explicit directed context dependencies. Self-dependency is rejected by the REL owner; graph-cycle and mounted-closure validation belongs to the multi-REL host because one REL cannot inspect the complete graph by itself. REL metadata consumes no Archive, Memory, Graph, or Vector Generation version.

### Ego owner records

Ego persistence is owner-local, append-only, and clock-neutral with respect to the existing Container semantic global clock. Ego maintains its own contiguous `ego_version` beginning at `1`; each logical document/Anchor also has an optimistic local revision beginning at `1`. Missing Ego records mean empty Ego state.

PHY-only Identity records now support multiple stable Identity documents plus one active selection. New Identity revisions use:
```text
8 bytes   "CVAEIDN2"
u64       ego_version
16 bytes  Identity UUID
u64       identity revision
u8        deleted: 0=false, 1=true
u8        activate: 0=false, 1=true
string    UTF-8 Identity display name; empty only for tombstones
string    UTF-8 Identity text; empty only for tombstones
```
The first live Identity is created with `activate=1`; later Identity creation/update does not implicitly replace the active Identity. Deletion appends a tombstone. An active Identity cannot be deleted while another live Identity exists, so switching is explicit. Deleting the only Identity leaves the PHY with no active Identity.

Active Identity swaps are independent state transitions and do not revise either Identity document:
```text
8 bytes   "CVAEIDA1"
u64       ego_version
16 bytes  active Identity UUID
```
The target must identify a live Identity. Re-selecting the already-active Identity is an API no-op and writes no record.

Legacy single-Identity `CVAEIDN1` records remain readable. They are projected as one stable reserved Identity with display name `Default` and are active on reopen; new writes use `CVAEIDN2`/`CVAEIDA1`.

PHY-only Personality:
```text
8 bytes   "CVAEPER1"
u64       ego_version
u64       personality revision
u64       source_memory_version
string    UTF-8 Personality text
```
`source_memory_version` records the PHY Memory watermark from which the behavioral projection was synthesized. Personality is derived state in this contract; explicit user-authored conditioning belongs in Identity, Anchors, or the underlying communication memories. Reliquary open rejects either PHY-only record family.

PHY/REL Anchor:
```text
8 bytes   "CVAEANC1"
u64       ego_version
16 bytes  Anchor UUID
u64       Anchor revision
u8        deleted: 0=false, 1=true
u8        priority: 1=High, 2=Normal, 3=Low
string    UTF-8 Anchor text; empty only for tombstones
```
Live Anchor text is limited to 2,500 Unicode scalar-value characters. Updates append a new revision. Deletion appends a tombstone and preserves history.

PHY/REL cached Memory-Web synthesis:
```text
8 bytes   "CVAESYN1"
u64       ego_version
u64       synthesis revision
u64       source_memory_version
string    UTF-8 synthesis text
```
The source Memory version is a deterministic stale-input watermark; it may trail the owner's current `memory_version` but may not exceed it. Direct writes and reopen validation reject impossible future source cuts. The record does not itself run synthesis or claim Memory/Dream authority. Identity, Personality, and synthesis storage impose no policy-level text cap beyond the underlying length-prefixed record representation; context-budget bounds belong to later Ego synthesis/assembly policy. Ego records consume no `CVAVERS1`, Archive version, Memory version, Graph version, or Vector Generation version.

### Interaction-stream checkpoints

Interaction-stream checkpoint:
```text
8 bytes   "CVAISTR1"
string    message ID
string    session/conversation ID
u8        parent present: 0=false, 1=true
optional  string parent message ID
u8        role: 0=User, 1=Agent
i64       timestamp_ns
u8        status: 0=Streaming, 1=Interrupted
string    cumulative UTF-8 content
```

Strings in this record use `u32 byte_length + UTF-8 bytes`. Interaction-stream records are append-only checkpoint revisions keyed by message ID; reopening keeps the latest valid revision for each message. Metadata is immutable across revisions and content must be append-only. `Interrupted` is terminal within one physical history.

These records are durable transcript state, not Archive semantic records. They consume no `CVAVERS1`, Archive version, Memory version, or Vector Generation version. A live runtime may expose the latest `Streaming` checkpoint as streaming only while the matching in-memory message is still active; after restart that same on-disk record is interpreted as interrupted. If the same message ID later exists as a completed Archive node, transcript resolution suppresses the checkpoint copy and uses the completed Archive turn.

Divergent Reliquary reconciliation retains and merges interaction-stream records independently of Archive semantic history. Compatible prefix histories keep the longest visible content and retain `Interrupted` if either side recorded interruption; non-prefix text or immutable-metadata divergence is rejected rather than silently discarding user-visible output.

### Echo sidecar records

Echo is durable turn-attached execution evidence stored inside REL files but outside the Archive/Memory semantic search populations. The initial record is append-only and keyed by `(conversation_id, message_id, sequence)`:

```text
8 bytes   "CVAECHO1"
string    conversation ID
string    message/turn ID
u64       sequence
i64       timestamp_ns
u8        model round present
optional  u32 model round
u8        event kind
optional  string correlation ID
optional  string name
string    content
```

Event kinds are `ReasoningSummary`, `Commentary`, `ReasoningTrace`, `ToolCall`, `ToolResult`, `ActivityStarted`, `ActivityCompleted`, and `ActivityFailed`, plus `ToolContextAccepted` (binary tag `8`). Records use typed binary fields and length-prefixed UTF-8 strings; they do not embed a JSON envelope. Sequence is authoritative for execution order. Identical replay is idempotent; conflicting events at the same turn sequence fail closed.

`ToolContextAccepted` retains a trusted embedding host's explicit requesting-context acknowledgment in this existing adjunct. Warlock's `warlock.memory_context_accepted.v1` content is a V1 JSON envelope containing the exact canonical prepared manifest and its SHA-256 digest. It is private receiver evidence, excluded from model/UI execution rendering; an ordinary ToolResult containing identical JSON is not proof. Strict manifest restoration and current owner authorization precede accepted-use replay. The existing Freshness receipt store remains the sole reinforcement idempotence authority; this adds no second delivery journal or persisted controller. Older readers that do not recognize tag 8 fail closed rather than silently interpreting it as a ToolResult.

Echo consumes no global, Archive, Memory, Graph, or Vector Generation version. Existing current REL files require no physical rewrite: absence of `CVAECHO1` means the Echo store is empty, and new records may be appended normally. Explicit legacy REL migration republishes Echo records into the new REL, and divergent REL reconciliation preserves Echo from both copies. Provider continuation state is not Echo semantic evidence and is not represented by this record family.

### Project repository correlation records

Repository-associated RELs may persist an append-only correlation between one REL semantic cut and one exact project-repository revision. These records are repository-neutral and are not project-file history themselves.

Current record:

```text
8 bytes   "PRJCOR02"
u64       correlation sequence
u64       REL global version cut
u64       REL Archive version cut
u64       REL Memory version cut
u8        repository kind: 1=Lore, 2=Git
u8        repository management: 1=WarlockManaged, 2=External
string    durable repository ID
string    repository-relative selected project path
string    opaque repository revision/commit reference
```

Legacy `PRJCOR01` omits the repository-management byte. Reopen assigns the compatibility default: Lore = `WarlockManaged`, Git = `External`.

Correlation sequence begins at `1` and is contiguous. Repository kind and durable repository ID must remain stable across one REL's correlation history; the selected project path and management policy may change within that repository. Project paths are normalized repository-relative paths using `/`; `.` represents repository root. Absolute paths, backslashes, empty path components, and `.`/`..` components are rejected.

A correlation write is idempotent only when the latest record already has the same REL cut, repository revision, and management policy. Correlation records consume no REL global, Archive, Memory, Graph, or Vector Generation version. Their `RelSemanticCut` is a cross-history observation, not another semantic mutation clock.

Strict-copy/extension reconciliation preserves correlation history verbatim. A true divergent semantic repack allocates new REL clocks, so branch-local cuts cannot be reused. Reconciliation instead requires the `(ProjectRevisionRef, ProjectRepositoryManagement)` histories to be prefix-compatible and, when the merge changes canonical state, emits one fresh correlation at the final merged REL cut using the latest compatible project revision. Divergent repository histories fail closed.

### Project-backed file bindings

Project-backed transcript/provenance attachments may reference exact historical repository files without copying their payload bytes into REL content storage. The durable binding is:

```text
8 bytes   "PRJFILE1"
32 bytes  FileId
u8        repository kind: 1=Lore, 2=Git
u8        content-hash present: 0=false, 1=true
optional  32-byte SHA-256 content hash
string    durable repository ID
string    repository-relative selected project path
string    opaque repository revision/commit reference
string    repository-relative file path
```

A `FileId` may have at most one Project-file binding. Replaying the identical binding is idempotent; a different binding for the same `FileId` is rejected. Repository ID and revision must be non-empty. Project and file paths are normalized repository-relative `/` paths; the file path must remain inside the selected project subtree unless the subtree is repository root (`.`).

`PRJFILE1` is clock-neutral repository-reference metadata. The associated Archive attachment manifest remains the transcript/provenance-visible file identity, while project-file bytes are resolved from the recorded repository revision and verified against the stored content hash when present. REL deliberately stores no duplicate `CVACONT1` payload for a repository-backed file.

### Conversation compaction store

Conversation compaction is durable derived conversation state stored only in REL files. Records are keyed by conversation plus branch checkpoint (`through_message_id`) and carry a generation plus UTF-8 summary. The store uses variable-width physical chunks rather than append-only semantic publication. Replacement writes the new live record and syncs it before reclaiming the superseded allocation. Reclaimed chunks are marked free, reused before file growth, split when oversized, coalesced when adjacent, and physically truncated when a coalesced free extent reaches end-of-file.

Free extents and obsolete/superseded allocations are allocator state, not durable semantic history. Reopen recovers incomplete supersession and completes reclamation. Sibling-branch live checkpoints may coexist. Compaction records consume no `CVAVERS1`, Archive version, Memory version, or Vector Generation version and are excluded from semantic ancestry comparison. Explicit legacy REL migration republishes only live compaction records; free/reclaimed physical extents are not copied.

### Archive records
Format marker:
```text
8 bytes   "CVAAFMT2"
u32       schema = 1
```
Content:
```text
8 bytes   "CVACONT1"
32 bytes  ContentId = SHA-256(content)
u32       byte length
N bytes   arbitrary content bytes
```
Node:
```text
8 bytes   "CVANODE2"
i64       timestamp_ns
32 bytes  ContentId
string    node ID
string    conversation ID
string    parent node ID; empty = none
string    role
string    principal_id; empty = none
```
Legacy `CVANODE1` omits `principal_id` and reopens with no principal; Reliquary never invents one during decode or reconciliation.
Branch/session head:
```text
8 bytes   "CVABRCH1"
u8        canonical
string    branch ID
string    conversation ID
string    leaf node ID
```
Current branch enumeration is reconstructed from the latest visible revision for each `(conversation ID, branch ID)` pair; it adds no persistent record type.
Conversation metadata (current writes):
```text
8 bytes   "CVACONV3"
string    conversation ID
string    title; empty = none
u8        active: 0=false, 1=true
u8        policy: 1=SequentialChronological, 2=VisibleAutoBranch
u64       policy revision (nonzero)
```
Conversation metadata is conversation-owned Archive semantic state, independent of branch topology. Records are append-only revisions keyed by conversation ID and published through ordinary Archive/global version metadata. Reopening keeps the latest visible revision; identical metadata publication is idempotent. Policy revision starts at 1 and increments only when the policy value changes; title/active writes preserve it. Overflow fails before publication.

Legacy `CVACONV1` contains only the conversation ID/title strings and decodes inactive. `CVACONV2` adds the active flag. Both remain readable and interpret missing policy as SequentialChronological at revision 1; opening does not rewrite them. Unknown policy tags, zero revisions, truncated records and trailing bytes fail closed. A conversation without metadata has the same default. No separate policy file or queue record is introduced.
Conversation tombstone:
```text
8 bytes   "CVACTMB1"
u64       authoritative Archive version
u64       authoritative global version
string    conversation ID
strings   sorted node IDs
strings   sorted branch IDs
ids       sorted FragmentIds
ids       sorted EpisodeIds
ids       sorted attached FileIds
u64s      conversation-metadata Archive versions
```
Each collection is prefixed by a `u32` count; each typed ID is 32 bytes. The record is published through ordinary `CVAAREC1` metadata, and its embedded coordinates must equal that publication. Reopen deterministically reconstructs the same direct closure before accepting the terminal state. Current conversation/branch/metadata/turn/fragment/Episode/attachment views exclude the target; explicit historical views retain access. Identical re-delete is clock-neutral. No downstream semantic owner is traversed or mutated.
Fragment:
```text
8 bytes   "CVAFRAG1"
32 bytes  FragmentId
string    conversation ID
string    start node ID
string    end node ID
```
The same deterministic window calculation may also be used transiently for read-only live-branch search. Those transient windows are ordinary in-memory `Fragment` values only: search does not write `CVAFRAG1` or `CVAAREC1`, advance Archive/global clocks, or alter the persisted Fragment population.
Episode:
```text
8 bytes   "CVAEPIS1"
32 bytes  EpisodeId
u8        origin: 1=Live, 2=Import
u8        boundary: 1=Size, 2=Inactivity, 3=CreateMemory, 4=ImportEnd
i64       source_through_ns
i64       finalized_at_ns
string    conversation ID
string    start node ID
string    end node ID
```
`EpisodeId` is SHA-256 over the conversation ID, start node ID, and end node ID in order, with each value encoded as `u64 byte_length + UTF-8 bytes`. Episodes are Archive semantic records and are published through ordinary `CVAAREC1` metadata.

File manifest:
```text
8 bytes   "CVAFILE1"
32 bytes  FileId
32 bytes  ContentId
u64       original byte length
string    filename
string    MIME type; empty = none
```
File bytes reuse the content-addressed `CVACONT1` store. `FileId` is SHA-256 over the `CVAFILE1-ID\0` domain separator, `ContentId`, byte length, length-prefixed filename, and length-prefixed MIME type. Identical bytes therefore share one content object while distinct filenames or MIME metadata remain distinct file manifests. `CVAFILE1` remains the standalone semantic representation for files introduced outside a source turn.

Native source turn with attachments:
```text
8 bytes   "CVATURN2"
i64       timestamp_ns
32 bytes  node ContentId
string    node ID
string    conversation ID
string    parent node ID; empty = none
string    role
string    principal_id; empty = none
u32       attachment count
repeated attachments:
    32 bytes  FileId
    32 bytes  ContentId
    u64       original byte length
    string    filename
    string    MIME type; empty = none
```
The turn body and attachment bytes are stored first as content-addressed `CVACONT1` backing objects. One versioned `CVATURN2` then publishes the source node, its optional principal identity, attachment file manifests, and the source-to-file relationship together. Legacy `CVATURN1` omits `principal_id` and decodes it as absent. An unversioned native-turn record is inert. Attached files therefore require no later source-association record or importer-side repair step.

File-to-Memory link:
```text
8 bytes   "CVAFMEM1"
32 bytes  FileId
32 bytes  MemoryId
```
The `(FileId, MemoryId)` pair is the link identity. The file must exist in Archive when the link is written; `Cva` validates the Memory target after both Archive and Memories have rebuilt. This record represents a later semantic relationship and is intentionally separate from native source attachment provenance.

Archive semantic metadata:
```text
8 bytes   "CVAAREC1"
u64       global version
u64       Archive version
u64       record chunk offset
u64       record payload length
```
Archive versions begin at `1` and are contiguous. A semantic Archive payload without valid metadata is inert. Nodes, native source turns, branches, fragments, episodes, standalone file manifests, and file-to-Memory links are semantic Archive payloads. One `CVATURN1` consumes one Archive/global version regardless of its attachment count. Fragment creation Archive versions are retained in the derived fragment index so later generation coverage can be validated.

### Phylactery owner composition

A current `.phy` initializes and requires the persistent formats for Memories, Entities, the currently implemented transitional Relationship store, Graph, Packed Vectors, Memory Vectors, and Compatibility Profiles, and accepts optional Ego Identity/Personality/Anchor/web-synthesis records. The Relationship format remains part of the current on-disk schema for compatibility and replay, but ADR 0034 now treats it as relational-Observation profile persistence rather than a separate semantic authority. Community snapshots and Community-name records are optional clock-neutral chunks and appear only after their explicit maintenance/edit operations. It does not initialize or accept Archive/Episode semantics, Files/attachments, Insomnia operational/completion state, Archive Vectors, Vector Generations, or interaction-stream checkpoints as Phylactery owners.

The same Memory record codec is reused, but current Phylactery validity is stricter about ownership: `source_episode_id`, `source_node_id`, `content_source_conversation_id`, `content_source_node_id`, `grounding_source_conversation_id`, and `grounding_source_node_id` must all be absent because those are same-owner REL provenance fields. A PHY Memory may instead carry optional `MemorySourceRef`, an identifier-only cross-owner provenance field containing the originating REL owner ID, optional source-turn PHY `principal_id`, Episode ID, primary source node ID, and optional authority/grounding conversation-node identities. No source body is copied into PHY, and an unavailable referenced REL does not invalidate the PHY. `source_time_ns` remains separate semantic chronology.

The existing disposable lexical index indexes REL Archive Fragments and filenames, so it is not part of `.phy`. A user-Memory lexical index, if required, is a separate future derived owner/design.

### Memories
Format marker:
```text
8 bytes   "CVAMEMF2"
```
Standalone content-addressed Memory body:
```text
8 bytes   "CVAMBDY1"
32 bytes  MemoryBodyId = SHA-256(exact body bytes)
u32       body byte length
N bytes   body bytes
```
The exact body bytes are:
```text
u64       title UTF-8 byte length
N bytes   title UTF-8
u64       content UTF-8 byte length
N bytes   content UTF-8
```
Memory record:
```text
8 bytes   "CVAMEMR8"
32 bytes  MemoryId
u64       revision
32 bytes  MemoryBodyId
u8        archived: 0=false, 1=true
optional  MemoryId superseded_by
optional  MemoryId parent_id
optional  EpisodeId source_episode_id
optional  i64 source_time_ns
optional  MemorySourceRef source_ref
          string owner_id
          optional string principal_id
          32 bytes EpisodeId source_episode_id
          string source_node_id
          optional string content_source_conversation_id
          optional string content_source_node_id
          optional string grounding_source_conversation_id
          optional string grounding_source_node_id
i64       created_at_ns
i64       updated_at_ns
string    category
string    memory_type
string    authority_kind
string    scope
string    lifecycle_state
string    temporal_status
optional  string source_node_id
optional  string content_source_conversation_id
optional  string content_source_node_id
optional  string grounding_source_conversation_id
optional  string grounding_source_node_id
string    mutation_id
optional  verified temporal inference
          32 bytes MemoryBodyId binding
          optional i64 source_time_ns binding
          string model
          string contract_version
          u8 resolution count (1..=32)
          repeated resolutions:
              u32 start_byte
              u32 end_byte
              u8 indication kind: 1=Explicit, 2=Calendar, 3=Relative, 4=Boundary, 5=Recurrence, 6=Duration
              string original evidence
              string canonical_expression
```
Optional fixed IDs, optional `source_time_ns`, optional `source_ref`, and optional strings use a one-byte `0`/`1` presence flag followed by the encoded value when present. `source_ref` stores identifiers only; it does not embed source content. `source_time_ns` records source-derived semantic chronology independently of the reference and is distinct from Memory creation/update bookkeeping. `authority_kind` is one of `direct`, `correction`, `adoption`, `retention`, or `unknown`; current Insomnia writes the first four, while legacy/manual records may use `unknown`. `temporal_status` is the Insomnia proposition-time classification `current`, `future`, or `historical`; `unknown` is accepted only as the compatibility value for legacy/manual Memories whose original classification was not persisted. This field is independent of Dream-owned `lifecycle_state`: temporal status describes how the proposition was framed when extracted, while Dream lifecycle describes the Memory's current owner-local semantic state. `MemoryId` for an automatically assigned new Memory is SHA-256 over `"continuity-memory-id\0"`, the mutation-ID byte length as `u64`, and the mutation-ID UTF-8 bytes. A Memory's `MemoryBodyId` cannot change across revisions. A current R7 record may additionally carry verified nondeterministic Chronos inference. The inference is valid only when its embedded `MemoryBodyId` and `source_time_ns` exactly match the enclosing record. It stores the producing model/contract plus each original unresolved source span/kind/evidence and a non-empty canonical expression that passed deterministic same-kind Chronos verification. Lifecycle/metadata revisions may carry it forward only while that binding remains exact; a changed body/reference binding drops the prior inference as stale. Deterministic Chronos products are never stored in this field.

Legacy `CVAMEMR6`, `CVAMEMR5`, `CVAMEMR4`, `CVAMEMR3`, and `CVAMEMR2` records remain decodable. R6 has durable `temporal_status` but predates verified temporal inference and therefore reopens with `temporal_inference = None`. R5 retains `source_time_ns` and `source_ref` but predates durable `temporal_status`; reopen assigns `temporal_status = "unknown"` rather than guessing a lost classifier result. R4 additionally predates the external source reference and reopens with `source_ref = None`. R3 lacks both `source_time_ns` and `source_ref`. R2 additionally lacks `authority_kind`; reopen assigns `authority_kind = "unknown"` rather than inferring provenance that was never persisted. An explicit metadata-only Memory revision may backfill `unknown` temporal status without changing the immutable Memory body, source chronology, provenance, or Dream lifecycle.

Standalone Memory publication metadata:
```text
8 bytes   "CVAMEMV1"
u64       global version
u64       Memory version
u64       record chunk offset
u64       record payload length
```
Memory versions begin at `1` and are dense. Normal direct Memory publication may store/deduplicate a standalone body, append `CVAMEMR8`, allocate one global version, and append `CVAMEMV1`; a standalone Memory record without valid version metadata is inert. `CVAMEMR7` remains readable and its `MemorySourceRef` decodes with no `principal_id`. Successful current Insomnia processing uses the `CVAINSC5` transaction described below instead: newly required local REL Memory bodies, `CVAMEMR8` records, their body-bound routing metadata, and their contiguous global-version range become visible through the one outer completion chunk and do not emit separate body/record/version/global-ticket chunks before it. User-owned PHY Memories are separate owner publications and are referenced from the REL completion by owner-qualified `MemoryRef`, not embedded as REL Memory records.

Clock-neutral Memory Entity-routing metadata may also be stored as a standalone attachment (used by PHY publication, migration, and semantic reconciliation). New writes use:
```text
8 bytes   "CVAMRTE2"
32 bytes  MemoryId
32 bytes  MemoryBodyId
u32       Entity-mention count (0..=64)
repeated Entity mentions:
    u8    text field: 1=title, 2=content
    u32   start UTF-8 byte offset
    u32   end UTF-8 byte offset
    string exact mention text
```
Every mention string is limited to 512 UTF-8 bytes. `MemoryId + MemoryBodyId` must identify the current immutable Memory body, and mention offsets must be valid character boundaries that slice to the stored text exactly. One Memory may have at most one routing attachment: identical replay is idempotent and a different attachment for the same Memory conflicts. `CVAMRTE2` consumes no global or Memory version and carries no Entity ID/type/identity decision.

Legacy `CVAMRTE1` remains readable. It has the same Entity-mention prefix followed by `u32 lexical-term count` and repeated exact lexical strings. Those former model-generated lexical terms are consumed only for compatibility validation while decoding and are discarded; they are not republished into current routing metadata. Lexical Memory search is now fully derived from complete current Memory title/content through the disposable owner-local Memory lexical index, so no lexical strings are persisted in new routing attachments.

### Entities

Entity owner format marker:
```text
8 bytes   "CVAENTF1"
```

Current Entity revision payload:
```text
8 bytes   "CVAENTR2"
32 bytes  owner-local EntityId
16 bytes  global Entity UUID
u64       Entity revision
i64       created_at_ns
i64       updated_at_ns
string    canonical_name
u16       alias count
repeated:
    string alias
string    kind
string    semantic summary
string    mutation_id
```

Legacy `CVAENTR1` records omit the 16-byte global UUID and remain readable. They reopen with an explicitly missing global UUID until `backfill_entity_global_ids` publishes one new revision per current legacy Entity. Backfill does not compare Entities across owners or infer cross-scope equivalence. Ordinary Entities receive a random UUID at first materialization; reserved principal Entities derive a deterministic UUID from their durable `phy-UUID` identity.

Entity publication metadata:
```text
8 bytes   "CVAENTV1"
u64       global version
u64       Entity version
u64       record chunk offset
u64       record payload length
```

Entity versions begin at `1` and are dense. One Entity revision consumes one owner-global semantic version and one Entity-local version. A record without matching version metadata is inert. Automatically assigned owner-local `EntityId` is SHA-256 over the domain separator `"reliquary-entity-id\0"` plus mutation-ID length and bytes. The global Entity UUID is a separate durable cross-scope identity token: it does not replace owner-local `EntityId` or ownership, is preserved through revisions/reopen/migration/reconciliation, and is not currently used to infer cross-owner equivalence. Canonical name, aliases, kind, and summary may change across revisions without changing either identity; `created_at_ns` is invariant for an existing Entity. Aliases are normalized, sorted, unique, and bounded. Entity records intentionally contain no supporting Memory IDs or graph adjacency; Memory↔Entity association belongs to Graph. Legacy REL/PHY files without this owner reopen with an empty EntityStore.

### Graph
Format marker:
```text
8 bytes   "CVAGFMT1"
u32       schema = 1
```

Legacy Memory node mapping remains readable:
```text
8 bytes   "CVAGNODE"
32 bytes  MemoryId
u32       dense NodeId
```

Typed non-Memory node mapping:
```text
8 bytes   "CVAGNOD2"
u8        SemanticNodeKind: 1=Memory, 2=Entity, 3=Observation
32 bytes  owner-local semantic node ID
u32       dense NodeId
```

Memory nodes continue to use `CVAGNODE` for byte-level compatibility and decode as `SemanticNodeRef::Memory`. Dense Arcana IDs are internal topology indexes; stable semantic identity remains the typed owner-local reference.

Legacy Memory-only relationship mutation remains readable and is still emitted for Memory-only transactions:
```text
8 bytes   "CVAGMUT1"
32 bytes  source MemoryId
32 bytes  target MemoryId
u16       Memory relationship kind
u8        active: 0=retracted, 1=active
u8        origin: 1=Dream, 2=User
```

Typed semantic relationship mutation:
```text
8 bytes   "CVAGMUT2"
u8+32     source SemanticNodeRef
u8+32     target SemanticNodeRef
u16       semantic relationship kind
u8        active: 0=retracted, 1=active
u8        origin: 1=Dream, 2=User, 3=Perception
```

Legacy Memory-only atomic batches use `CVAGBAT1`; typed/mixed atomic batches use `CVAGBAT2` and repeat the corresponding typed endpoint/kind/active/origin fields. Memory relationship kind codes remain `1=topical`, `2=factual`, `3=causal`, `4=recurrent`, `5=references`, `6=duplicate-of`, `7=supersedes`, and `8=structural-parent`. Semantic kind `100=entity-association` is the ordinary directional `Memory -> Entity` Perception relation. Semantic kind `101=principal-association` is the reserved directional `Memory -> principal Entity` relation used for PHY-backed user identity; it is durable but omitted from ordinary semantic traversal/projection and exposed only through explicit principal lookup. Observation relation families are not yet accepted because no durable Observation owner exists.

One batch cannot contain the same oriented `(source, target, kind)` identity more than once. Already-visible no-op states are removed before publication. Endpoint validation is relation-specific: Memory relation families require two same-owner Memories; `entity-association` requires an existing same-owner Memory source and non-principal Entity target; `principal-association` requires an existing same-owner Memory source and an Entity whose reserved kind is `principal`.

Graph version metadata is unchanged:
```text
8 bytes   "CVAGVER1"
u64       global version
u64       graph version
u64       mutation payload chunk offset
u64       mutation payload length
```

`graph_version` begins at `1`, is dense across **all** semantic Graph transactions, and is the optimistic concurrency clock for Graph publication. One non-empty transaction consumes one file-global semantic version plus one Graph-local version regardless of change count. A mutation payload without matching version metadata is inert. Retraction appends `active=0` rather than deleting history.

Graph also derives a non-persisted `memory_graph_version`: the latest full `graph_version` containing a Memory-to-Memory relation mutation, or `0` when no Memory relation has ever been published. Dream/Leiden/Memory-retrieval derived state keys to this Memory projection watermark, while Graph writes continue to compare against the full `graph_version`. Entity-only mutations therefore remain visible in the semantic Graph without invalidating unchanged Memory-only topology.

The Arcana kernel materializes two derived in-memory views from the same accepted relation authority: the full typed semantic topology and a Memory-only topology used by existing Dream traversal and Communities. Pre-Graph CVAs with no Graph records reopen at version `0`.

### Transitional Relationship persistence

The current pre-unification Relationship records are persisted separately from Graph relation mutations. ADR 0034 now treats this format as transitional structured persistence for relational Observations rather than a separate semantic object family. Current REL and PHY files initialize:

```text
8 bytes   "CVARELF1"   Relationship format marker
```

A Relationship backing record uses:

```text
8 bytes   "CVARELR1"
32 bytes  RelationshipId
u64       revision
i64       created_at_ns
i64       updated_at_ns
string    kind
u16       participant count
repeat participant:
  string  EntityRef.owner_id
  32 bytes EntityRef.entity_id
  u8      role present
  [string role]
u16       evidence count
repeat evidence:
  string  MemoryRef.owner_id
  32 bytes MemoryRef.memory_id
string    compact summary
string    mutation_id
```

Its current version-publication record is:

```text
8 bytes   "CVARELV1"
u64       global version
u64       relationship version
u64       backing-record chunk offset
u64       backing-record payload length
```

`relationship_version` begins at `1` and is dense within the containing REL/PHY. A backing record without valid version metadata is inert. Participant/evidence lists are normalized into deterministic order before publication. Local-owner Entity/Memory references must resolve; owner-qualified references into another REL/PHY may remain unresolved. This history is physically/version-wise separate from Arcana Graph history, but ADR 0034 no longer grants it independent semantic authority from Observation; migration/convergence must preserve the current format until relational Observation persistence supersedes or incorporates it.

### Dream maintenance records

Dream maintenance metadata is owner-local, append-only, and clock-neutral. It is stored in both REL and PHY containers but is not a Memory revision, Graph relationship, Archive mutation, or semantic global-version claimant.

Current cooldown record:

```text
8 bytes   "CVADREM2"
32 bytes  MemoryId
u64       last satisfied provenance-relative Dream epoch
i64       last successful Dream processing timestamp_ns
```

The timestamp is runtime scheduling metadata. It may anchor maintenance eligibility when semantic source chronology cannot be recovered, but it never substitutes for source chronology in Dream temporal analysis, duplicate ordering, causality, supersession, or other semantic reasoning.

Legacy cooldown record:

```text
8 bytes   "CVADREM1"
32 bytes  MemoryId
u64       last satisfied provenance-relative Dream epoch
```

`CVADREM1` remains readable. It has no successful-processing timestamp; legacy state may use existing Memory update bookkeeping only as a one-time non-flooding maintenance bootstrap until a successful Dream pass writes `CVADREM2`.

Completed unordered Dream-pair evaluation:

```text
8 bytes   "CVADRP01"
32 bytes  lower canonical MemoryId
32 bytes  upper canonical MemoryId
```

The two IDs are stored in canonical byte order, so evaluating the pair from either source direction has one identity. Self-pairs are invalid. A record means Dream successfully completed evaluation/publication for that immutable Memory pair, including a `none` classification or withheld relation that produced no Graph edge. Candidate discovery excludes these pairs, as well as pairs already connected by an active Graph edge, before bounded candidate ranking.

Cooldown records reconstruct the maximum satisfied epoch and latest available successful-processing timestamp per Memory. Pair records reconstruct a set union. Divergent REL reconciliation applies those same monotonic merge rules. Explicit legacy REL/PHY migration copies both record families after their Memory endpoints have been republished. These records consume no `CVAVERS1`, Archive version, Memory version, Graph version, or Vector Generation version.

### Community snapshots

Community snapshots are optional derived records. There is no required Community format marker; absence means that no partition has been materialized for the current file history.

```text
8 bytes   "CVACOMM1"
u32       schema = 1
u64       generation
u64       derived Graph version
u32       community algorithm version
u64       Leiden seed
f64       Leiden resolution
f64       partition quality
u32       community count
repeated communities:
    32 bytes  CommunityId
    u32       member count
    N×32      ordered MemoryIds
```

Community generations begin at `1` and are contiguous across Community snapshot chunks, but this generation is derived-state bookkeeping only: it consumes no `CVAVERS1` ticket and is not a semantic timeline. The latest snapshot is current only when its derived Graph version equals current `memory_graph_version`.

A current snapshot partitions every current Memory-projection node exactly once; Entity and Observation nodes are excluded from the Dream/Leiden projection unless a later decision explicitly changes that contract. Communities are stored in ascending `CommunityId` order and members in ascending `MemoryId` byte order. `CommunityId` is SHA-256 over `"reliquary-community-v1\0"`, the 16-byte durable owner UUID, member count as little-endian `u64`, and ordered member IDs. This v1 identity is exact-membership identity and does not imply continuity after membership changes.

Algorithm version `1` is the legacy monolithic Leiden baseline. It projects active oriented Graph relationships onto unordered structural pairs, collapses multiple relationship identities between the same unordered Memory pair to one edge with weight `1.0`, and clusters the complete owner projection with Leiden modularity at resolution `1.0` and seed `0x4c454944454e0001`. Version `1` snapshots remain readable derived state.

Algorithm version `2` keeps the same structural projection, modularity resolution, and seed but computes the complete partition through deterministic scan-and-merge. A graph-local BFS order is divided into fixed 2,048-node leaf shards. Shard-local Leiden runs may execute concurrently; their results are reduced through a binary merge tree. Every original structural edge participates exactly once: same-shard edges enter the leaf run, while a cross-shard edge enters the lowest merge level containing both endpoints. Parent runs use weighted coarse edges/self-loops representing the complete child subgraphs. Worker count is deliberately not persisted because it changes scheduling only; the persisted partition must be worker-count invariant. The projection and all reduction summaries are derived computation only and do not change persisted Graph orientation or vocabulary.

A stale or older-algorithm snapshot may remain physically present. Explicit refresh appends a new complete snapshot using the current algorithm version. `CommunityStats.current` requires both current `graph_version` and current community algorithm version. Semantic repacks may omit Community snapshots and rebuild them later from Graph authority.

Community semantic names are separate optional clock-neutral records:

```text
8 bytes   "CVACNAM1"
u32       schema = 3
u32       Dream naming contract version; `0` for user-authored names
u8        source: 1=Dream, 2=User
32 bytes  exact-membership CommunityId
32 bytes  baseline CommunityId
u32       representative Memory count
repeated  32-byte MemoryId
u32       UTF-8 name byte length
N bytes   semantic name
```

The current Dream naming contract version is `2`. Dream records carry contract versions `1..=2` and 1–8 sorted unique representative Memory IDs from the named Community. Version `2` uses the four existing Community sub-centroids and targets two representative Memories per sub-centroid. User-authored records carry contract version `0`, source `User`, and zero representative Memory IDs. Directly persisted schema-3 records require `baseline CommunityId == CommunityId`; the baseline marks the exact membership against which the name was authored/generated. Current-name resolution may project that record through deterministic Community lineage without appending another name record. User names follow clear continuation lineage without a material-change cutoff. Dream names follow clear lineage only while the current Community remains at least 0.750 Jaccard-similar to the stored baseline. All records store a trimmed non-empty UTF-8 name no longer than 96 bytes. Latest direct record for one `CommunityId` wins. Historical schema-1 and schema-2 `CVACNAM1` records remain readable and infer their stored Community ID as the baseline. Naming records consume no `CVAVERS1`, Graph version, Community generation, Memory version, or Vector Generation version. Representative IDs record which Memory texts informed Dream; vectors themselves are never persisted in this record or sent as naming evidence.

### Packed vectors
Format marker:
```text
8 bytes   "CVAPVFM1"
u32       schema = 1
```
Matrix object:
```text
8 bytes   "CVAPVEC1"
32 bytes  PackedVectorId
u32       dimensions
u8        scalar tag
3 bytes   reserved = 0
u64       row count
u64       matrix byte length
N bytes   contiguous rows
```
`row_bytes = dimensions * scalar_width`; matrix length must equal `row_count * row_bytes`. Scalar tags: `1=i8`, `2=u8`, `3=i16`, `4=u16`, `5=i32`, `6=u32`, `7=i64`, `8=u64`, `9=f16`, `10=bf16`, `11=f32`, `12=f64`.
Packed-vector ID is SHA-256 over `"CVA-PACKED-VECTOR-V1\0"`, dimensions, scalar tag, and exact matrix bytes.
Freshness P2 changes only transient event scheduling and whether diagnostic counters are collected. P4 adds a fixed 8,192-byte receipt membership bitset and completeness flag as owner-local, rebuildable acceleration state; it is never serialized, and exact positive lookup and canonical replay still read the accepted journal. These phases add no persisted chunk, magic, version, identity encoding, accepted effect, or score-policy change.

### Dream Freshness pass journal (`CVDREAM1`)

Each chunk begins with eight-byte magic, sixteen-byte owner UUID and a u8 kind. Strings are u32-length UTF-8; Memory IDs are 32 bytes; numeric turns/versions are little-endian u64; sets are u32-count sorted unique IDs.

| Kind | Payload and authority |
| --- | --- |
| 0 | Begin: pass ID, source ID, first revision (1), original turn, semantic Graph baseline, eligible pre-existing IDs |
| 1 | Roots: pass ID and eligible root set |
| 2 | Settled: pass ID, closing an accepted begun pass |
| 3 | Legacy acceptance: pass ID and first settlement turn; it lacks current graph evidence |
| 4 | Publication intent: Memory ID, first mutation ID, accepted birth turn, u8 admit-at-birth flag |
| 5 | Intent consumption: Memory ID and matching mutation ID |
| 6 | Reconciliation replay of settled pass ID |
| 7 | Reconciliation replay of publication birth ID |
| 8 | Current acceptance: pass ID, settlement turn, semantic Graph cut, Memory graph version, optional Community generation and matching Memory graph version (one presence tag and two u64s) |

Reopen validates complete framing, owner/source identity, first revision, root membership, immutable acceptance and exact Community proof. Direct birth intent is derived only when the matching first accepted Memory revision exists at its proven Archive cut; orphan intent grants no score. Grouped completion supplies its own atomic birth proof. Recovery uses original accepted graph topology and settlement turn, excludes the newborn cohort and publishes at most one effect event before closing the pass. Current acceptance retains both semantic and Memory Graph cuts because they serve different owners.

Strict copy/repack/reclamation preserve these chunks. Divergent reconciliation replays settled identities, original birth/admission flags and immutable effect/proof provenance at exact rebased Archive cuts. Pending first passes or publication intents remain explicitly unsupported and are rejected before output; a missing historical proof is not substituted by current topology.

### Legacy semantic Freshness watermark (`CVAFSA01`)

`CVAFSA01` is a retained pre-release compatibility record. It begins with eight-byte `CVAFSA01`, followed by UTF-8 JSON with fields `owner_uuid` and `activation`; the historical activation object contains owner, exact Archive boundary/cut and semantic policy identity. Current code does not expose an enable/disable API and does not create or change these records. When encountered on an older development REL, a valid non-null record contributes only a lower-bound watermark so already-accounted fragments are not credited again; a historical cancelled/null record does not disable intrinsic F8. Reopen still validates a retained non-null boundary, reconciliation preserves/rebases it by source identity, and repack/reclamation retain the bytes. New RELs require no activation record and use the ordinary activity/checkpoint floor.

### Memory Freshness journal (`CVAFRS02`)

Freshness is owner-local metadata separate from immutable Memory bodies and semantic revisions. The owner pins a validated policy in `CVAFRP01`: eight-byte magic, sixteen-byte owner UUID, little-endian u16 policy version, u64 turns per decay point and four i16 values for access/linkage principals and local/outside hop costs. Policy becomes immutable before enrollment; inconsistent owner/policy records cause reopen to fail.

Every `CVAFRS02` record begins with eight-byte magic, sixteen-byte owner UUID and a u8 kind. Kind 0 initializes a Memory (32-byte ID, u64 accepted turn, score/admission/accounting record); kind 1 admits it (ID and record). A record contains i16 score and optional admitted/accounted u64 turns, each encoded as a zero/one tag followed by the value when present. Kind 2 stores an event; kind 3 stores an atomic accepted-use batch; kind 4 stores a separate accepted-use receipt for reconciliation replay. Strings use a u32 byte length followed by UTF-8. Counts are u32.

An event carries event ID, destination-local u64 turn, producer kind (1 Dream, 2 accepted use, 3 previous turn-semantic, 4 fragment-semantic), original source ID, sixteen-byte origin owner, original u64 accepted turn and u64 base policy version. Semantic producers encode typed evidence **before** graph proof. For kind 3 the evidence is owner UUID (16 bytes), conversation/node strings (u32 length + UTF-8), accepted turn (u64), base policy digest (32 bytes), semantic version (u16), semantic digest, compatibility-profile ID, source-evidence digest and target-evidence digest (32 bytes each). Kind 4 inserts a 32-byte FragmentId and u32 source count after the evidence owner UUID, followed by that many conversation/node string pairs in strictly increasing source-key order. The source list contains every represented Node, including non-activity context, and includes the pinned availability anchor. Empty/duplicate/noncanonical lists reject. The accepted turn is fragment availability; only activity-bearing Nodes contribute its maximum.

Graph proof follows typed evidence (u64 graph version, optional u64 Community generation and Community graph version), then u32 effect count and ordered (32-byte Memory ID, i16 positive effect) pairs. Semantic graph proof is a nonauthoritative zero/none placeholder. Semantic events may have zero effects; nonempty bonuses are 1..=10. The target digest commits selected body/vector bindings, bytes and bonuses; replay preserves frozen evidence rather than searching current vectors. The source digest pins fragment/represented content, retained generation/profile and row evidence.

Fragment event IDs are `semantic-fragment-v1:` plus lowercase SHA-256 hex over ASCII `reliquary:freshness:semantic-fragment:v1`, one zero byte, the 16 owner UUID bytes and 32 FragmentId bytes. This fixed policy domain does not rotate with profile/generation/configuration evidence. One fragment cannot be awarded again after re-embedding. Kind-3 `semantic-live-v1` receipt decoding remains unchanged for existing records; normal live processing writes kind 4. No migration dispatcher or public migration API is added. Postimages remain derived. A use batch carries use ID, accepted turn, source policy version, sorted unique sources and one event per source. Record kind 4 (distinct from event producer kind 4) remains the separate accepted-use receipt without constituent events. Dream and use identities/principals are unchanged.

Append plus sync precedes acceptance. Validation rejects malformed framing, unknown kinds, truncation/trailing bytes, duplicate recipients, invalid births/admission anchors, principal/provenance/proof violations, and conflicting stable identities. Unsupported `CVAFRS*` prototypes, including `CVAFRS01`, require explicit migration and fail closed. Legacy owners without UUIDs require migration before new enrollment.

Canonical projection orders births and admission before eligible events at their logical cut, and orders events by (accepted destination turn, event ID), independently of disk arrival. Historical reads scan only the accepted physical prefix. A 128-entry/256-KiB recent receipt cache accelerates retries; older exact identities stream from the durable owner. Reopen temporarily indexes validated receipt payloads on disk and replays effects once; this disposable index is removed when rebuild finishes. Large canonical histories use bounded 256-KiB sorted runs and pairwise merges in disposable scratch storage. These projections are rebuildable and never owner authority. There is no general compaction checkpoint or retention policy; history grows with accepted mutations. The ordered historical import settlement is specified below as CVAFRS04. Reconciliation preserves original provenance/effects while rebasing destination turns through exact Archive source identities, without rerunning graph propagation.

See [Architecture](architecture.md), [Rust API](api.md) and [core verification](freshness-core-release-verification-2026-10-03.md).

### Memory Vectors
Format marker:
```text
8 bytes   "CVAMVFM1"
u32       schema = 1
```
Row-binding object:
```text
8 bytes        "CVAMVEC1"
32 bytes       MemoryVectorId
32 bytes       CompatibilityProfileId
32 bytes       PackedVectorId
u64            row count
row_count × 32 ordered MemoryBodyIds
```
Row `N` maps to `memory_body_ids[N]`. Matrix row count must match exactly, the matrix must be `f32` with dimensions equal to the compatibility profile, and every `MemoryBodyId` must exist and be unique within the set. Across all Memory-Vector objects, a `(CompatibilityProfileId, MemoryBodyId)` pair may appear only once. `MemoryVectorId` is SHA-256 over `"CVA-MEMORY-VECTORS-V1\0"`, the profile ID, `PackedVectorId`, and ordered `MemoryBodyId`s.

Memory Vectors are immutable derived bindings and consume no semantic version ticket. Memory revisions may change metadata but cannot change their semantic `MemoryBodyId`; therefore no Memory-vector regeneration/update record exists. A new compatibility profile may bind the same Memory body to another immutable vector.
### Archive Vectors
Format marker:
```text
8 bytes   "CVAAVFM1"
u32       schema = 1
```
Row-binding object:
```text
8 bytes        "CVAAVEC1"
32 bytes       ArchiveVectorId
32 bytes       PackedVectorId
u64            row count
row_count × 32 ordered FragmentIds
```
Row `N` maps to `fragment_ids[N]`. Matrix row count must match exactly; every fragment must exist and be unique within the set. `ArchiveVectorId` is SHA-256 over `"CVA-ARCHIVE-VECTORS-V1\0"`, `PackedVectorId`, and ordered `FragmentId`s. Compatibility and source-Archive metadata are excluded.
### Compatibility profiles
Format marker:
```text
8 bytes   "CVACPFM1"
u32       schema = 1
```
Profile object:
```text
8 bytes   "CVACPRO1"
32 bytes  CompatibilityProfileId
u32       dimensions
u8        normalization: 0=None, 1=L2
3 bytes   reserved = 0
u32       probe-suite version
u32       compatibility-policy version
u32       reference count
repeated references:
    u8    mode: 1=Query, 2=Document
    3     reserved = 0
    u32   vector length
    N×f32 reference values
```
Current probe suite v1 stores two Query and two Document references. Compatibility policy v2 requires corresponding probe vectors from an endpoint to have cosine similarity `>= 0.9998`, with dimensions, normalization, suite version, and policy version equal.
`CompatibilityProfileId` content-addresses the exact stored contract using `"CVA-COMPATIBILITY-PROFILE-V1\0"`, dimensions, normalization, suite/policy versions, and ordered mode+`f32` reference values. Exact ID equality is not the compatibility test; tolerant probe comparison is.
Provider, model, route, and revision metadata are not stored in compatibility profiles. Profiles are immutable and consume no semantic version.
### Vector generations
Format marker:
```text
8 bytes   "CVAVGFM2"
u32       schema = 1
```
Generation payload:
```text
8 bytes   "CVAVGEN2"
32 bytes  VectorGenerationId
32 bytes  CompatibilityProfileId
32 bytes  ArchiveVectorId
u64       source Archive version
```
Generation version metadata:
```text
8 bytes   "CVAVGRC2"
u64       global version
u64       vector version
u64       generation payload chunk offset
u64       generation payload length
```
`VectorGenerationId` is SHA-256 over `"CVA-VECTOR-GENERATION-V2\0"`, compatibility-profile ID, Archive-Vector ID, and source Archive version. Vector versions begin at `1` and are contiguous inside `VectorGenerationStore`. An unversioned generation payload is inert.
The latest published generation for each compatibility profile is current. Source Archive version may not regress for that profile, exceed current Archive state, or predate any fragment in the referenced Archive-Vector set. Current generation publication requires the referenced packed matrix to use `f32`; packed storage remains generic, but alternate searchable scalar representations are not semantic generation formats until their interpretation is defined.
### Insomnia operational records
Format marker:
```text
8 bytes   "CVAINSF1"
```
Legacy/final work-state record:
```text
8 bytes   "CVAINSW1"
32 bytes  EpisodeId
u8        priority
u8        work state
u32       attempt count
i64       updated_at_ns
optional string lease owner
optional 32-byte lease token
optional i64 lease expiry
optional i64 retry-after
optional string last error
```
`CVAINSW1` remains decodable for existing development files and is still used for one final Terminal outcome during the current compatibility slice. New Pending, Processing, renewal, Failed/retry, and lease-expiry transitions are runtime-only and do not emit records. On reopen, persisted transient Work states are ignored; every Episode without a final success/terminal outcome is re-derived as Pending from Archive Episode metadata. Old Complete/Terminal records remain compatible final state.

Current successful Episode completion:
```text
8 bytes   "CVAINSC6"
32 bytes  EpisodeId
u32       attempt number
i64       started_at_ns
i64       completed_at_ns
u32       rejected candidate count
u64       first embedded global version
u32       embedded global-version count
i64       transaction_time_ns
string    extractor model
string    extractor contract/version
u32       local Project MemoryId count
N×32      local resulting MemoryIds
u32       external MemoryRef count
repeated external Memory refs:
    string owner_id
    32     MemoryId
u32       embedded local Memory-body count
repeated embedded Memory bodies:
    32    MemoryBodyId
    u32   Memory-body byte length
    N     exact Memory body bytes: u64 title byte length + UTF-8 title + u64 content byte length + UTF-8 content
u32       newly published local Memory-record count
repeated newly published records:
    u64   global version
    u64   memory version
    u32   encoded Memory-record length
    N     complete "CVAMEMR8" record payload
u32       embedded routing-metadata count
repeated embedded routing metadata:
    u32   encoded routing-metadata length
    N     complete "CVAMRTE1" payload
```
V6 appends `u64 freshness_birth_turn + u64 freshness_policy_version + u32 birth-ID count + N*32 Memory IDs` after routing metadata. IDs equal the exact unique revision-1 records in the completion. This atomic proof initializes extracted births at +100 without admission and reconstructs cohort membership without a repair write. V5 lacks the trailer and never fabricates legacy birth proof. An empty V6 cohort conveys no new enrollment.

The embedded global-version count must equal the newly published local Memory-record count. For a non-empty local publication, record global versions are contiguous beginning at the stored first version. A completion with no new local REL Memories consumes no REL global versions even when it records external Memory references.

`CVAINSC6` is the physical and logical visibility boundary for the REL side of an Insomnia success. Its `transaction_time_ns` is sampled by the Container immediately before publication and applies to every embedded global version in that atomic completion. New local content-addressed Memory bodies, their Memory records, their body-bound routing metadata, their global-version allocation, owner-qualified external Memory references, and the compact successful Episode receipt all live inside this one outer REL chunk. No standalone local Memory-body, Memory-record, Memory-version, routing-metadata, or `CVAVERS2` chunk is emitted before it. Embedded bodies are indexed by `MemoryBodyId` against the outer completion `ChunkRef`; body resolution reads that completion chunk and selects the matching embedded body by ID. Existing local bodies may be referenced without being re-embedded. Embedded routing metadata is validated only after its referenced Memory record/body has been reconstructed, but becomes visible from the same outer completion transaction.

An external Memory reference is `string owner_id + 32-byte MemoryId`. The current routed implementation uses it for User-owned PHY Memories. Those Memories are published and synced in the PHY before the REL completion is appended, and are not REL semantic/version state. If the process fails after PHY publication but before the REL receipt, retry reuses the deterministic Memory mutation ID and accepts the existing PHY Memory only when its routed semantics match.

If a `CVAINSC6` append is interrupted, ordinary trailing-chunk recovery removes the incomplete outer REL chunk, leaving no orphan local REL body, record, routing attachment, or global-version ticket. The Episode therefore reopens as Pending and can be retried safely. Any already-synced external PHY Memory/routing attachment remains durable and is reused by that retry. A valid completed transaction reconstructs all new local Memories, their embedded routing metadata, plus the successful receipt and its external references together.

`CVAINSC4` remains decodable with the same transaction timestamp, owner-qualified external Memory references, and atomic local Memory transaction as V5/V6, but it predates embedded routing metadata and therefore reopens with none from that completion. `CVAINSC3` retains external Memory references but predates explicit transaction time, so its embedded versions reopen with unknown transaction timestamps. `CVAINSC2` has the same embedded local Memory transaction but no external-reference section and likewise has no transaction-time mapping. `CVAINSC1` also remains decodable; in that older format, content-addressed Memory bodies and standalone global-version tickets may precede the completion chunk. V1/V2 completions reopen with an empty external-reference list. Current identified owners write `CVAINSC6`; V5 remains decodable and is emitted when no owner birth proof is available.

The older `CVAINSA1` attempt record remains decodable so existing same-format development files can reopen, but current processing no longer emits it. Retryable failures are runtime-only and add no persistent record. A final Terminal outcome currently persists as one `CVAINSW1` record; a successful outcome persists as one compact `CVAINSC6` REL transaction for identified owners, with any routed external owner publication already durable.

Optional values use a one-byte `0`/`1` presence flag followed by the encoded value when present.

### Strings
```text
u32 byte_length
N bytes UTF-8
```

## Historical semantics

Archive, Memories, Entities, Graph, and Vector Generations have independent local watermarks. Community `generation` and `memory_graph_version` are derived-state bookkeeping, not independent semantic-owner clocks. Global ordering may interleave semantic mutations; integer adjacency is never semantic ancestry. Packed matrices, Memory-Vector bindings, Archive-Vector bindings, and compatibility profiles are immutable backing objects. Memory Vectors have no local clock because their identity is immutable semantic Memory content plus compatibility profile. A published Vector Generation is the semantic association that activates one profile/population.

## Diagnostics and failure behavior

`Cva::open` / `Reliquary::open` requires Reliquary type/scope identity (or the legacy 16-byte Project form) and validates the format markers and records owned by each persisted subsystem. Current 40-byte typed files also carry the durable owner UUID; earlier 16-byte and 24-byte forms remain readable for explicit migration but have no owner ID. Graph and Entity are compatibility exceptions for older files created before those owners existed: absence of their records opens as empty owner state, and new durable writes initialize the required current records. Other unsupported development-format incompatibilities fail closed.

`Phylactery::open` requires exact typed Phylactery identity (`file_kind=2`, scope byte `0`) and rebuilds Memories, Entities, typed Graph state, optional Community snapshots/semantic names, Packed Vectors, Memory Vectors, Compatibility Profiles, Ego state, and Entity-resolution state where present. It validates semantic global-version uniqueness, Graph endpoints, Entity-resolution references, Community/name membership references, vector/profile references, absence of REL-local Memory provenance, and structural validity of any external `MemorySourceRef`. REL, legacy CVA, and invalid file-kind/scope combinations fail closed.

Container validates framing/global tickets. A truncated **final** length-prefixed chunk is treated as an interrupted append: reopen truncates the file to that chunk's starting offset and resumes from the last complete chunk boundary. Truncation of the REL/PHY header still fails closed. Concrete stores validate their own complete records. Cross-store references are validated after reconstruction in dependency order. Composition-level validation rejects a global version claimed by multiple semantic mutations.

## Defaults or precedence

Default fragments use eight turns with two-turn overlap; the exported library constants `DEFAULT_FRAGMENT_TURNS` and `DEFAULT_FRAGMENT_OVERLAP` are the single source for that default policy. Default Episode input ceiling is 32 KiB. Compatibility probe suite v1 and compatibility policy v2 are fixed by the current implementation.

## Gateway owner-operation journal

The gateway mounts `<state-dir>/owner-operations/<rel-uuid>.roj` alongside each REL conversation journal. Its 28-byte header contains `RQOP0001`, little-endian version 1 and the exact 16-byte REL UUID. Each append frame stores a 32-bit payload length, 64-bit sequence, one-byte tag, SHA-256 prefix checksum, JSON payload and SHA-256 checksum over prefix plus payload. Payload framing is bounded to 16 MiB; file size is bounded to 256 MiB. Tags cover Attribution, EpisodePlan, EpisodeReceiptObserved and Held; a leading snapshot preserves its sequence watermark across atomic compaction. Reopen repairs an incomplete final prefix/payload only; complete checksum/sequence/owner corruption fails closed.

Attribution is now used in production User admission and preserves exact original verified identity on retry. Agent source records additionally carry an explicit `initiating_generation_id` naming an earlier admitted User operation. The journal copies that operation's conversation, initiating principal and exact admission/PHY identity; append, replay and snapshot validation reject mismatches or missing initiating operations. The optional field defaults to absent for existing User records. Source identity conflicts across actor classes resolve to Unknown. Attribution also captures an optional `personal` admission containing the UseOwnPhy mount epoch and owner grant generation; a destination binding alone supplies no permission. Frozen personal drafts name their exact authorizing User generation IDs. Missing fields in older operational records remain absent and cannot authorize a write. Episode plans and receipts are consumed by the REL publication coordinator. Grant withdrawal syncs authorization-revoked Held markers for the original attachment's User generations, including offline REL journals; Agent bindings inherit withdrawal from their explicit origin. Compaction preserves withdrawal markers even when another Held reason follows. Grant restoration does not revive those admissions. Compaction retains attribution and unresolved plans, pruning a plan only after its observed receipt is durable. Operational state consumes no semantic clock and changes no REL/PHY/Archive format. Missing attribution disables source routing authority even when Archive retains a legacy principal field. This journal does not claim a distributed transaction or cross-process gateway arbitration.

## Gateway provisional conversation journal

New gateway submissions retain admission authorization facts in the existing request_metadata JSON: owner_id, principal_id, grant=SubmitConversation and policy_revision=1. The existing principal_id and scope_fingerprint bind the accepted turn to the canonical REL, conversation and branch. These facts are immutable accepted-work authority, not live client credentials or persisted attachment tokens. Earlier admissions without this metadata remain readable using their retained verified principal and scope; present metadata must match. Exact retries compare the original request fingerprint with new server-generated admission metadata omitted, without rewriting the accepted record. FinalizeCapture retains the admitted principal, final branch, exact parent and supported policy revision. VisibleAutoBranch captures an optional human-facing automatic branch label in the same JSON record before Archive publication; older FinalizeCapture records omit that field and decode it as absent. No new frame/event/container format is introduced.

Gateway mount identity is admission state, not a storage-format change: the existing Container header parser is reused by a read-only probe before semantic reopen. The canonical REL/PHY header, UUID preservation on copy/repack, and Archive/Memory formats are unchanged. OS physical file identity is retained only in the live gateway execution and is never persisted as portable owner identity.

The gateway's provisional conversation state is stored outside the REL container under `<state-dir>/conversation-queues/<rel-uuid>.rqj`, with one journal per canonical REL owner. Staged attachments are content-addressed under `<state-dir>/conversation-queues/staging/<rel-uuid>/<sha256>.blob`, keeping garbage collection isolated to one REL. The file begins with a checksummed owner header. Append records use bounded length-prefixed JSON frames with a sequence and SHA-256 checksum; a compacted snapshot uses a 64-bit payload length and checksum so a full-REL projection can exceed an ordinary frame limit while remaining within the journal bound. A sibling lock file provides exclusive writer ownership across opens.

Host operation IDs hash the length-prefixed owner/conversation/submission components so separators in a valid component cannot collide with composite boundaries. HostOperationUpdated records retain per-capability stable operation IDs, payload digests, transition revisions, phases, last known phases and sticky cancellation requests in that same journal. Older snapshots default missing host-operation maps and cancellation fields; older readers are not assumed to understand the new event variants. Unknown host outcomes retain staged payloads and prevent native replay. Terminal host records survive compaction with the submission tombstone.

The host payload is distinct from a journal frame: `RQMUXH01`, followed by a little-endian u64 metadata length and JSON `(owner_id, Submission)`, then a little-endian u64 byte length and raw bytes for each attachment in submission order. SHA-256 covers the entire bounded payload. Binary attachment bytes are not expanded into JSON arrays. This is a controlled adapter operation contract, not a host assistant-result format.

Attachment bytes are durably staged in a per-REL content-addressed spool before enqueue can acknowledge them; journal submissions retain the identity and content hash. Compaction keeps canceled-submission tombstones and unresolved finalization/Archive/postcommit receipts while reclaiming eligible terminal payload bytes. The journal is operational state, not `.rel` Archive history. The dated F1–F6 runtime verification and Phase 1 acceptance passed; no journal frame/event/container format version changed during finish execution. Format changes require updating this description alongside the owning journal module.

## Gateway generation journal records (P3.6)

The existing REL operational journal adds GenerationUpdated and optional generation state on submission projections/tombstones; absent legacy fields default to none. Immutable fields are operation/generation IDs, branch ID, parent node ID and start timestamp. Phase changes carry contiguous transition_revision; checkpoints carry contiguous sequence, UTF-8 byte length and SHA-256. Terminal generations reject further mutation. Nonterminal generation payloads survive compaction. Text stays in the existing synchronized interaction-stream owner; no new CVA format or P4 cursor is added. [P3.6 record](multiplexing-implementation-plan.md) specifies the synchronization and restart handoffs.

## Conversation snapshot persistence boundary (P3.7)

The observation builder adds no persisted record, journal event, CVA format or mutation clock. Its durability barrier calls the mounted owner's existing sync. RelJournal::observation_branch_id reads captured targets or the existing retained admission branch metadata even after terminal compaction removes content/attachments. Automatic branch labels/fork parents remain the existing FinalizeWon capture fields. The snapshot publishes only the durable checkpoint sequence, never the checkpoint digest or stream text. [P3.7 evidence](multiplexing-implementation-plan.md).

## Atomic historical import settlements (`CVAFRS04`)

The physical-boundary attribution helpers do not alter this persisted layout. In particular, `input_bytes` is the frozen input prefix, not the physical position of ImportStart or the accepted settlement frame. New-epoch reconstruction cannot publish until a versioned settlement/replay contract authenticates its start prefix and preserves prior same-cut receipts. Existing CVAFRS03/CVAFRS04 remain readable under their original contracts.

Each payload is an authoritative sparse Freshness settlement; ordinary receipts remain `CVAFRS02`. Exact decoded retries are no-ops. Conflicting same-ID evidence and reversed/overlapping cuts reject.

All integers are little endian. Layout: eight-byte magic; 16-byte owner UUID; seven 32-byte digests (import ID, start evidence, source corpus, manifest, frozen input bytes, H1 policy, F8 policy); seven u64 values (base cut, settlement cut, global version, input byte length, Archive version, full Graph version, Memory Graph version); u32 cohort count and sorted unique Memory IDs; u32 sparse replacement count and sorted unique Memory IDs with existing Freshness records; u32 superseded-event count and sorted unique length-prefixed UTF-8 event IDs; SHA-256 over all preceding bytes. Unsupported policies, trailing bytes, missing accepted birth proof and noncanonical replacements reject.

Replay applies ordered settlements at their cuts and preserves untouched records and retained earlier receipt history. Post-settlement accepted use at the same activity cut remains valid through retry/reopen. Later-import replacement construction must start from canonical state at its base cut; production coordination remains pending.

Legacy `CVAFRS03` retains its six-digest/six-u64 layout. Decode creates one synthetic initial settlement with base cut zero and deterministic start-evidence digest, preserving every stored identity, cut, record and proof field. Reopen does not rewrite the file; no new `CVAFRS03` is emitted. Copy/repack/reclaim preservation and conversion restrictions continue to apply to settlement authority.

## Freshness diagnostic records

Opt-in Freshness stage samples are process-local JSON diagnostics. They are not Container chunks and add no journal magic, event version, accepted clock or on-disk authority. Historical scan counters distinguish visited payload bytes from separately read mutation-object bytes; these counts describe read work, not new durable indexes. Existing accepted effects and postimages remain authoritative. [Development](development.md) owns capture commands.

## Related docs

- [Architecture](architecture.md)
- [Rust API](api.md)
- [Architectural invariants](invariants.md)
- [Operator and integration manual](manual/INDEX.md)
- [ADR 0006](decisions/0006-archive-vector-row-bindings.md)
- [ADR 0007](decisions/0007-compatibility-profiles-and-vector-generations.md)
- [ADR 0012](decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md)
- [ADR 0013](decisions/0013-immutable-memory-vector-bindings.md)
- [ADR 0036](decisions/0036-typed-semantic-graph-endpoints.md)

## Notes

These remain development formats and may evolve before a stable external-format commitment. Filenames are indexed only in the disposable in-memory lexical index and add no persistent record. Broader persistent lexical indexing, ANN acceleration, retention-aware semantic-history vacuum, and whole-file historical restore remain future work.

## Conversation recovery validation (P3.8)

P3.8 adds no persistent record, schema, queue, clock or migration. It validates the existing generation checkpoint length/hash against the exact UTF-8 prefix in the interaction-stream owner before reconciling a synchronized extension. Archive-backed completion also reconciles that extension. Mount and quiescence validate captured submission/Archive receipts through the same implementation. Pending order, policy revision, branch identity and Unknown/cancel-intent records retain their existing owners and formats. [P3.8 evidence](multiplexing-implementation-plan.md).

## Historical production review boundary

CVAFRS04 is the current historical settlement publication; CVAFRS03 is read-only legacy compatibility. C2 deletes the duplicate in-memory store and composition API and makes native settlement helpers private to the Cva owner module. Cva::reconstruct_historical_freshness accepts options only and publishes the verified native owner projection through the existing settlement append and sync. The settlement format adds start evidence and a base cut; ordinary receipt identity and automatic activation are unchanged. Supported final credit-excluded replacements may contain only the canonical unadmitted creation record (+100, both activity anchors absent); arbitrary unadmitted scores or partial anchors reject. Full canonical manifest/contribution proof is integrated; C6 records legacy checkpoint recovery; current settlement recovery acceptance is tracked in the remaining execution plan; importer integration and retained-corpus release acceptance remain release blockers. See [completion plan](freshness-historical-completion-execution-plan-2026-10-12.md).
## P5-E native receipt reuse

E5 adds no REL/PHY/container/journal codec. Native tool events use the existing immutable Echo source sequence and correlation fields; P3 validates their accepted generation and ordering before synchronized publication. Generated artifacts are existing embedded files/source attachments atomically ingested with the finalized assistant turn. Exact file identity, ordered source attachment receipt and conflicting retry validation survive ordinary reopen. Resume tokens are caller-held hints, not persisted semantic authority. See [E5 acceptance record](multiplexing-implementation-plan.md).


## External-derived index and Memory records (P5-G core)

The external core adds three concrete formats without reinterpreting native Memory record v8 or `MemorySourceRef` bytes.

`CVAXIDX1` is an append-only complete source snapshot: 8-byte magic, little-endian u64 JSON length, UTF-8 JSON, then SHA-256 of all preceding bytes. JSON is bounded to 8 MiB and contains namespace/contract/profile/dimensions, generation/revision/cursor, sorted descriptors and external chunk-to-packed-vector bindings, plus previous cursor/change digest. No source strings are stored. The snapshot is installed after sync. Invalid logical index metadata is discarded and requires rebuild; missing profile/vector backing also invalidates that source. Physical Container framing/checksum failure retains the existing Container recovery policy.

`CVAXMEM1` atomically publishes an owner-local semantic Memory batch, external evidence bindings, routing attachments and window receipt. Its fixed header is magic (8), JSON length (u64), embedded global version start (u64), record count (u32), and transaction timestamp (i64), followed by bounded JSON and SHA-256. The authenticated fixed range lets Container recover physical global versions without interpreting Memory semantics. MemoryStore validates the embedded native Memory records, body hashes, provenance and versions. Legitimate semantic Memory bodies are stored; canonical transcript bodies are absent. Empty receipts reserve no global versions. Typed REL publication carries its accepted activity cut for freshness reconstruction.

`CVAXPLN1` is a bounded checksummed processing plan in trusted Gateway state at `external-processing/<rel-owner>/<window-hash>.rxp`. It uses the magic/u64-length/JSON/SHA-256 envelope and the existing frozen PreparedMemory codec. It freezes project/private derivations, exact private principal/destination and identifier-only evidence before recipient writes. Publication uses synchronized temporary files and atomic rename (write-through rename on Windows). The plan is not REL/PHY source history; it needs the same access protection and backup treatment as existing owner-operation plans. Per-plan size is 16 MiB; each REL plan directory is limited to 4,096 plans and 256 MiB. There is no automatic plan-pruning policy in this core.


C3–C4 now derive the full H1 manifest and canonical semantic/Dream event batch from exclusively borrowed native Cva owner state. No supplied digest, roots or exclusion arrays enter the operation. Retained source vectors, grouped/singleton provenance, availability, lifecycle and profile proof determine eligible semantic roots before the 64-root limit. Actual first-cycle Dream/duplicate-origin evidence is pinned to accepted graph transactions; verified no-link takes precedence over later adjacency. The private frozen batch carries raw source identities and recipient-only exclusions. Unsupported provenance stays untouched, while final credit-excluded replacement records are unadmitted. The options-only operation now propagates the frozen batch, scores canonical per-recipient contributions through the frozen cut and publishes through native FreshnessStore/CVAFRS04. Exact accepted retries return without rescoring or rewriting; new owner-input drift rejects before publication. C8 implements bounded embedding through the exclusive offline/configured operation.

Previous turn-semantic receipts remain readable as their frozen source model. New fragment receipts cannot be mixed with that previous live history: acceptance rejects before append, preventing an old observation from receiving a second award after the source-model correction. No conversion or maintained pre-release migration path is provided.

## N3 historical proof encoding

Physical CVAFRS03 framing is unchanged. N3 used H1 manifest schema/policy version 2; vector proof pins FragmentId, all immutable represented Node bindings, latest activity availability, selected generation/profile and row/content digests. Requested-fill contract version 2 hashes fragment coverage under `reliquary:historical-embedding-coverage:v2`. The N3 reconstruction policy digest was `e4a40d7711ba768119336577b6276de1aff396d2d9d289606e9d3674615e088b`; previous unreleased checkpoint policy digests reject without a compatibility registry. Archive Fragment/vector encodings and authoritative retained fixtures are unchanged.

## N4 accepted Archive chronology

H1 manifest schema remains 2; reconstruction policy version is 3, timeline `accepted-archive-activity-v1`, policy digest `f72f73b74a30a59de228c3a14a0e9344314c61724116efe3d822f22a25ca1fdb`. CVAFRS03 framing and semantic fragment/Dream identity are unchanged. Reopen verifies the accepted source prefix against Archive positions. Previous unreleased policy digests reject; no maintained conversion support is added. See [H1 policy](freshness-historical-manifest-policy.md).

## Completed retained normalization storage boundary

The one-time normalization added no REL/PHY format, compatibility chain or checkpoint framing. Proved legacy completion births were reencoded with the existing current completion encoder and physical references relocated with the standard storage helper. Existing immutable Memory bodies/revisions and accepted Archive/Graph authority were preserved. Ordinary current-owner processing supplied genuinely missing generation/vector/Dream work. The accepted Freshness checkpoints use existing CVAFRS03 framing; historical approximation does not become actual Dream proof. Temporary normalizer sources, private hooks, credentials and audit scratch are deleted. The [verification record](freshness-retained-normalization-verification-2026-10-13.md) retains before/after hashes, derivation claims and measured acceptance. Fixture metadata retains original marker text separately from verified current hashes/versions.

## Historical production scope

The [C12 production disposition](freshness-historical-production-disposition-2026-10-13.md) marks H1–H8 complete and merge-ready as supported explicit, offline/exclusive single-owner REL reconstruction. Historical reconstruction intentionally remains an offline/backfill operation rather than automatic or mounted migration. Live F8 is intrinsic REL behavior with no activation switch. PHY Freshness remains outside this release.
