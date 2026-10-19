# ADR 0041: Semantic import boundaries, owner-neutral pruning, and explicit history rewrite

Status: Accepted direction, 2026-10-18.

## Context

ADR 0040 introduced terminal tombstones, import epochs and explicit transformed-copy pruning. The implementation subsequently made the physical ImportStart-to-settlement byte range part of durable Historical Freshness authority. CVAFRS05 persists start/freeze prefixes and an immutable-prefix digest; legacy framing similarly authenticates an entire pre-marker file prefix.

Physical ordering solved a real attribution problem during settlement construction: accepted events at the same logical activity cut still need deterministic before-import versus during-import classification. It did not require every byte in that physical range to remain immutable forever.

The current physical-prefix authority has an undesirable consequence: payload that is semantically safe to destroy, including packed embedding matrices for retired compatibility profiles, may become permanently unprunable merely because it was written before an authenticated freeze prefix. Repeated imports can expand that protected region.

Reliquary and Phylactery also need symmetric owner maintenance. REL supports explicit destructive history pruning with retention evidence; PHY has compatible lifecycle/vector primitives but no validated destructive-prune authority.

Finally, history-preserving pruning is not enough for long-lived stores. Users need an explicit operation that can construct a fresh REL/PHY history from selected useful state and intentionally leave years of superseded, duplicate, tombstoned, obsolete or otherwise unwanted history behind.

## Decision

### Import epochs are semantic history boundaries

ImportStart and ImportComplete remain the durable owner-level boundaries of an import epoch.

Physical file offsets may be used transiently while constructing an import settlement, including for same-cut accepted-event attribution. They are not enduring semantic authority after the settlement has been accepted.

The canonical import-settlement representation will persist the exact semantic evidence needed to validate attribution: import/start identity, source/cohort identity, affected Memories, exact accepted event identities and digests, superseded event identities, semantic cuts and resulting Freshness state. It will not require the original physical byte range to remain unchanged.

New code stops emitting physical-prefix-authoritative settlements. Persisted CVAFRS03/CVAFRS04/CVAFRS05 and CVAILEG1 remain readable only at the on-disk compatibility boundary. A verified normalization path may consume their physical evidence once and materialize the canonical semantic settlement.

### Pruning is owner-neutral and history-preserving

Pruning preserves the same logical owner and the same accepted semantic history while explicitly destroying selected historical payload.

Prune authorization belongs to shared owner maintenance, not Archive. REL and PHY both consume the same maintenance manifest/store. Each semantic domain remains responsible for proving that a selected payload may be destroyed.

Pruning may replace eligible payload with authenticated prune markers while retaining the minimum identity, topology, versions, transaction ordering and retention evidence necessary to distinguish intentional destruction from corruption.

REL additionally owns Archive-specific pruning. PHY does not acquire Archive semantics merely to participate in owner maintenance.

### Rewrite is intentional fresh-history construction

A rewrite reads one REL or PHY, applies an explicit selection policy, and creates a new destination history containing only the selected state.

Rewrite is not migration, reconciliation, reclaim or prune. Omitted history is intentionally absent from the destination and does not require prune markers.

Memory selection is a primary rewrite input. The caller may choose explicit Memories and/or deterministic filters over current lifecycle, supersession, known duplicate state, age/source time, Freshness and related canonical metadata. The rewrite planner must make closure expansion explicit.

The caller may select any number of embedding compatibility profiles, including none. Selected profiles carry only vector rows required by retained Memory bodies and retained REL Archive fragments. Unselected profiles and their vector payload do not appear in the rewritten destination.

Source/Archive, Entity, Relationship/Observation, Graph, Ego, Freshness and derived-state policy are independently selectable subject to explicit referential-closure rules. No dangling semantic reference is silently created.

Rewrite has two identity modes:

- new-owner rewrite creates an independent owner, suitable for development fixtures, experiments and forks;
- preserve-owner rewrite is an explicit replacement/rollover operation. It records a new history generation and predecessor/rewrite evidence so old and rewritten generations cannot be mistaken for ordinary divergent replicas.

Reconciliation must fail closed across different history generations unless a future explicit cross-generation operation is defined.

## Consequences

Import settlement validation becomes independent of physical relocation and eligible payload pruning.

Retired embedding payload written during an import can be pruned once legacy physical settlements are normalized to semantic evidence.

PHY can support the same validated destructive Memory/Entity/Relationship/vector pruning model as REL without acquiring Archive ownership.

Long-lived REL/PHY stores gain an explicit, auditable way to discard low-value deep history and begin a compact new history from selected state.

Migration remains the format-preserving operation. Rewrite does not inherit migration's requirement to replay every historical revision.

## Compatibility boundary

Persisted user data is the only reason to retain old settlement readers. CVAFRS03/CVAFRS04/CVAFRS05/CVAILEG1 compatibility is isolated at decode/normalization. No new internal caller may branch on those formats and no new writer emits them.

The migration is otherwise a staged hard cut to one canonical semantic import-settlement representation and one shared owner-maintenance pruning substrate.
