# Operational verification for OC01–OC09

## Final live closure — 2026-11-08

**OC09 live acceptance passed.** The final canonical run is preserved at
`C:\!bin\workspace-data\reliquary-oc09-live-2026-11-08`. It used the
ordinary rebuilt CLI, the existing protected `reliquary.cfg`, and the
same two-day normalized input hash recorded below. Its import ID is
`f7cc33500ec1bebe3d1f3b3be3fc5d529c14a7704ecbc90f50de2bbe1722169c`.

The saved Codex credentials were explicitly refreshed. Live provider
verification then exposed and closed two transport/probe regressions:
Responses-Lite requests again send `reasoning.context = "all_turns"`, and
the direct decision readiness probe uses a valid typed `choice` question.
All seven originally configured provider routes subsequently passed live
synthetic probes.

The real import completed all 46 Episodes after two explicit held-work
retries: 46 complete, 0 failed, 0 held and 0 pending. Insomnia published
13 REL Memories and 4 PHY Memories while rejecting 81 candidates through
normal validation. Entity processing completed, the optional relationship
stage remained not configured, vector coverage completed, all 13 imported
REL Memories completed first-cycle Dream, historical Freshness completed,
and the import reached `Ready`.

A Dream retry exposed a second recovery defect: the resumed coordinator
derived a new embedding compatibility-profile identity instead of reusing
the profile already pinned to the import. The corrected path now reuses
that accepted profile and verifies the current endpoint against it before
continuing. The preserved import resumed successfully through Dream and
Freshness after this correction.

Passive hybrid search returned the WebSocket handshake and movement-packet
Memories with complete vector coverage. Stateful retrieval initially
exposed two configuration/provider-subset gaps. The existing config had no
Memory Retrieval, shared decision, or General fallback, so `models.general`
was explicitly configured to the already-working Dream Codex route
(`gpt-5.6-luna`, `codex-alt`, low reasoning), leaving every dedicated
route unchanged. The live Codex structured-output API then proved that it
rejects JSON-Schema `const` while accepting the equivalent typed one-value
`enum`; the General decision adapter now normalizes that schema form.

The predeclared stateful query then selected the three relevant Memories:
`GDScript input packet fields`, `Client sending input packets`, and
`WebSocket hello received`. Refinement rewrote the query to:

> What does the user explicitly say the server still needs to do for movement packets after the WebSocket client handshake?

Traversal from the packet-fields Memory inspected persisted Dream Factual
and Topical edges. The model declined the one unseen neighbor for the
refined query, so traversal returned no additional Memory while retaining
the exact edge evidence. `show`, `source`, and `neighbors` all
succeeded in the same interaction session. Source inspection recovered
user turn `20baaa7f-1f8a-4caa-be23-946fa7b3b928` verbatim enough to prove
the intended fact: the client had connected, its `hello` reached the
server, and the next task was to get the server to handle movement.

A final independent process reopened both owners: REL verified with 13
Memories, PHY verified with 4, and a new stateful query returned the same
three WebSocket Memories. CLI interaction remained inspection-only, so it
did not manufacture a confirmed-consumption Freshness receipt; confirmed
host delivery/receipt semantics remain covered by the accepted trusted-host
tests.

The sections below retain the earlier blocked snapshot as historical
evidence. Its 401/400/pending statements are superseded by this closure.

## Real two-day Archive import

The ordinary CLI created separate project REL and user PHY owners, staged
the normalized source, verified both owner files, and reopened them in
separate processes. The input SHA-256 matched
`d6c261b544a1c62d20ede3a77fb21bcb0a9fcfbc682c041ee2bf5eb8c65e8f37`.
The recorded UTC selection used nanoseconds
`[1778860679950216000, 1779033479950216000)`.

| Measurement | Result |
| --- | --- |
| Conversations | 8 |
| Branches | 21 |
| Visible turns | 778 |
| Episodes staged | 46 |
| Remapped parent links | 41 |
| Asset references omitted by the text-only import | 35 |

An independent CLI read compared the retained parents, roles, timestamps,
and content of all 778 visible turns. Both owner file hashes stayed the
same during inspection. The import ID is
`ee8199e89934803df74d168812856da2bdd039f61925211a126ee9d4c42b1c4d`.

## Predeclared retrieval checks

The query is “What has the user established about the WebSocket client
handshake and the server's next step for movement packets?” The refinement
is “Focus on what the user explicitly says the server still needs to do.”
The raw user turns `20baaa7f-1f8a-4caa-be23-946fa7b3b928` and
`d011afa7-3b64-409c-a926-7cd024218e0e` in the same conversation
provide the expected handshake and packet-decoding evidence. Verify the
original role, exact text, timestamps, revision, and Archive ancestry when
inspecting a selection. The movement math question
`f3a3d327-0423-4c7d-92b5-d58612a302ed` does not establish a settled
requirement. Use a persisted active edge for traversal if an eligible
edge emerges from processing.

## Provider and processing evidence

The protected configuration loaded offline. It contains dedicated
Insomnia, Insomnia ownership, Dream, Entity Resolution, Entity Resolution
decision, and embedding routes. It supplies none of the accepted retrieval
decision choices `models.memory_retrieval_decision`,
`models.general_decision`, or `models.general`. The original Switchboard
uses the same retrieval selection order. The implementation keeps
each configured account and route scoped to its declared operation.

Independent readiness probes sent synthetic input. Insomnia, Insomnia
ownership, Dream, and Entity Resolution each received HTTP 401. The
configured direct Entity Resolution decision route received HTTP 400.
The embedding route passed both query and document compatibility probes
against an empty owner. Automatic approval review separately rejected
a live retrieval command because it contained corpus-derived text destined
for external providers. The real import remains staged and pending.

| Acceptance area | Real-corpus result |
| --- | --- |
| Insomnia extraction | 46 pending, 0 complete, 0 held, 0 failed |
| Accepted and rejected candidates | 0 accepted, 0 rejected, no extraction attempted |
| REL and PHY Memory publication | 0 REL, 0 PHY |
| First-cycle Dream, Entity and relationships | Pending extraction |
| Vector coverage and historical Freshness | Pending extraction |
| Retrieval, refinement and traversal | Live gate blocked |
| Source and fresh-process owner verification | Passed for the Archive |
| Model quality and confirmed use | No live observation |

The separate-process loopback fixture reached Ready, reopened one Memory
in each owner, checked hybrid search, and exercised query, refinement and
traversal from an anchor with no active neighbors in a JSONL session.
It also checked exact and restricted
provenance through the REL and PHY commands. Confirmed use is covered by
trusted-host delivery and receipt recovery tests. Fixture results are
separate from live acceptance.

The final locked offline run passed 656 workspace tests and 16 CLI tests.
The Root-only consumer passed eight tests, including exact held-checkpoint
resume, paired live recovery, and independent owner verification. Its
pre-existing authored G4 reopen test retains the G4 runner prerequisite.
Workspace check, all three
format checks, local Markdown links, staged whitespace, and the architecture
guard passed. Cargo builds used the repository's serialized
`target-codex-check` directory with debug information disabled for dev and
test profiles.

## Processing scope and recovery

The imported coordinator declares typed relationship proposals optional.
An owner-scoped record binds the configured routes and ordered stage policy
before imported extraction. The vector stage binds the verified profile.
The Root-only fixture rejects changed Dream configuration before held
Episode retry and rejects a changed embedding profile after reopen.

The live coordinator reads accepted Insomnia checkpoints, resolves the
recorded Entity mentions, and runs first-cycle Dream with a bound route,
profile, and recorded frontier. The Root-only fixture resumes paired REL
and PHY Entity work after a provider failure and independent reopen. The
separate-process CLI fixture verifies a persisted Dream edge and observes
no further inference on replay.

Older live extractions that lack an exact checkpoint report unsupported
Entity work. Automatic relationship proposals have no configured live
source. The coordinator builds vectors for selected Dream sources, while
the named vector commands can cover additional eligible Memories.

Lexicon is uninitialized in this worktree, so there is no current Arcana
snapshot. The repository's executable dependency guard checks all 23
packages and 76 production edges. Local Markdown links are checked
directly because the shared documentation policy file is absent.
