# ADR 0020: Reliquary and Phylactery file kinds

Status: Accepted; typed Reliquary and Phylactery file identities/lifecycles implemented; Reliquary project-only semantics amended by ADR 0021; explicit legacy migration implemented by ADR 0022
Date: 2026-08-24
Owners: container identity, Reliquary persistence boundary, Phylactery persistence boundary, migration compatibility
Supersedes: CVA as the long-term user-facing file identity
Superseded by: none

## Context

The original `.cva` file identity predates the separation between Reliquary state and user-global Phylactery state. Once both persistence domains exist, treating both as generic CVAs would make file identity and storage semantics ambiguous.

Reliquary and Phylactery are expected to reuse most of the same low-level storage machinery. That implementation reuse does not mean they are the same semantic container. ADR 0021 subsequently amends Reliquary from a project/workspace-only container into the shared family for typed non-user durable scopes such as Organization, Project, and Connection. Phylactery remains user-global Identity state whose validity must not depend on retaining project source turns.

A different extension alone is insufficient because callers must be able to validate the semantic kind from the file itself rather than trusting a filename.

## Decision

The long-term product file identities are:

- **Reliquary:** `.rel`
- **Phylactery:** `.phy`

They are distinct semantic file kinds backed, where practical, by shared container/storage primitives.

Conceptually:

```text
shared container/storage engine
├── framing / checksums / recovery
├── versioning primitives
├── object records
├── memory machinery
├── vector machinery
├── graph machinery
└── compaction / migration support

        ↓ semantic file kind

Reliquary (.rel)          Phylactery (.phy)
typed non-user scopes     user-global Identity state
```

Reliquary and Phylactery file kinds are now encoded in the mandatory typed physical header. Reliquary additionally carries Organization, Project, or Connection scope identity; Phylactery carries no Reliquary scope. The extension is a user-facing identifier, not the authority for container type.

ADR 0021 adds a second internal discriminator for the Reliquary scope kind and adopts the preferred human-facing filename shape `<name>.<scope-type>.rel`, currently including `.org.rel`, `.prj.rel`, and `.con.rel`. The internal type remains authoritative.

## Semantic distinction

### Reliquary (`.rel`)

Reliquary is the shared persistence family for typed non-user durable scopes. ADR 0021 currently establishes Organization, Project, and Connection scope kinds.

Project Reliquaries retain project/workspace identity, source Archive/history, Episodes, Memories, provenance/evidence, vectors, graph state, files/attachments, and later Dream/Ego-derived project state as those owners are implemented. Organization and Connection Reliquaries may use different semantic owner sets and validity rules over the same low-level container machinery.

Project source/provenance remains a first-class part of the Project Reliquary model.

### Phylactery (`.phy`)

Phylactery is the user-global persistence domain. Its deliberately bounded owner set now includes Memories, Graph, Packed Vectors, Memory Vectors, Compatibility Profiles, and purpose-built Ego Identity/Personality/Anchor/web-synthesis records. Ego state is not represented as Memory records: it has its own append-only owner-local persistence so explicit persona/context conditioning does not become Memory-Web authority. Shared low-level mechanics are reused where their semantics are genuinely shared.

A Phylactery Memory does not require source turns or a live originating Reliquary. Direct `.phy` Memory publication requires all REL-local Episode/node/conversation provenance fields to be absent, so PHY never stores another owner's source records. Routed User Memories may instead retain `MemorySourceRef`, an identifier-only owner-qualified reference to the originating REL, Episode, source node, and optional authority/grounding node identities. That reference may be unresolved without invalidating the PHY.

Phylactery must not become "a Reliquary with nullable provenance." Archive/history, Episodes, embedded Files/attachments, Insomnia work/completion state, Archive Vectors, Vector Generations, and interaction-stream checkpoints are not Phylactery owners in the current implementation. The existing lexical index is also REL Archive-specific and is not reused as a fake user-Memory index.

Graph and Memory-vector/profile state are valid Phylactery-owned durable/derived state. Same-owner Dream processing is now implemented by ADR 0024 using persisted source chronology. Dream does not federate candidates across owners; cross-scope context composition belongs to higher-level retrieval/Ego. Export policy and a purpose-built user-Memory lexical index remain later work.

## Shared implementation boundary

The semantic split does **not** justify duplicating the physical storage implementation.

Low-level framing, checksums, append/recovery mechanics, version primitives, immutable backing objects, vectors, compaction, and other genuinely common mechanics should remain shared. Reliquary and Phylactery add type-specific validation and owner composition above those primitives.

Typed Reliquary scopes should likewise share the common Reliquary/container machinery while allowing Organization, Project, and Connection to enforce distinct scope-specific validation and owner composition.

A strong semantic boundary can therefore exist without a second repository or a forked storage engine.

## CVA compatibility and migration

Existing `.cva` data remains valid as the legacy 16-byte physical form. New creation uses typed Reliquary headers and `.rel` product identity.

Existing `.cva` files are conceptually **legacy Project Reliquary files**, not ambiguous future containers. Migration should move them to the typed Project Reliquary form without gratuitously invalidating stored semantic identities.

The transition should preserve existing record payloads, deterministic IDs, hash/domain-separation constants, and other stable format material wherever possible. A product/file rename alone is not sufficient reason to regenerate Memory IDs, vector IDs, compatibility profiles, or semantic history.

Legacy detection remains explicit and without automatic rewrite. ADR 0022 resolves migration as a semantic repack into a separate current 40-byte identified file rather than an in-place header shift, because persisted semantic records contain absolute chunk offsets. Merely renaming `.cva` to `.rel` is still not migration.

## CVA terminology

`CVA` should stop being the long-term user-facing name for a product file once `.rel` and `.phy` are implemented.

Whether `CVA` survives as a private implementation term for the shared generic container engine is intentionally undecided. If it creates more confusion than value, the implementation abstraction should receive a neutral name later. This naming question must not block the semantic `.rel`/`.phy` split.

Existing `CVA*` record markers and `CVCFG` framing are not changed by this ADR alone. Their compatibility or replacement belongs to the concrete format-migration implementation.

## Consequences

- Warlock can distinguish typed non-user Reliquary state (`.<scope>.rel`) from user-global Identity state (`.phy`) before retrieval begins.
- Phylactery remains part of the Reliquary storage/runtime codebase rather than becoming another repository solely because it has a distinct file type.
- Shared container machinery remains reusable without collapsing the semantic domains into one omni-store.
- New product-facing creation and documentation use typed `.rel`; `.cva` now means the explicitly supported legacy Project Reliquary physical form.
- Migration from `.cva` must be explicit and compatibility-aware.

## Open implementation decisions

- whether later Phylactery capabilities justify additional purpose-built owners beyond the implemented Memory/Graph/vector/profile core;
- exact required, optional, and forbidden owner sets for Organization/Project/Connection Reliquaries as their policy surfaces diverge;
- richer source-resolution/export policy beyond the implemented identifier-only `MemorySourceRef`;
- whether the internal/back-compat `Cva` type/name should eventually be removed; the public product-facing alias is now `Reliquary`;
- file-association and shell UX for `.rel`, typed `.<scope>.rel`, and `.phy` in Warlock;
- whether any low-level record markers need a future neutral naming/version transition.

## Related docs

- [ADR 0018 — Reliquary and Phylactery naming](0018-reliquary-and-phylactery-naming.md)
- [ADR 0021 — Typed Reliquary scopes and Connection state](0021-typed-reliquary-scopes-and-connections.md)
- [Reliquary and Phylactery scope design record](../subsystems/scope-ownership.md)
- [Storage format](../storage-format.md)
- [Roadmap](../roadmap.md)
