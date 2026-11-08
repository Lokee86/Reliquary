# Operational verification for OC01–OC09

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
The embedding probe passed. Automatic approval review separately rejected
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

The final locked offline run passed 654 workspace tests and 14 CLI tests.
The Root-only consumer passed seven tests, including exact held-checkpoint
resume and independent owner verification. Its pre-existing authored G4
reopen test retains the G4 runner prerequisite. Workspace check, all three
format checks, local Markdown links, staged whitespace, and the architecture
guard passed. Cargo builds used the repository's serialized
`target-codex-check` directory with debug information disabled for dev and
test profiles.

## Remaining implementation scope

The imported coordinator declares typed relationship proposals optional.
An owner-scoped record binds the configured routes and ordered stage policy
before imported extraction. The vector stage binds the verified profile.
The Root-only fixture rejects changed Dream configuration before held
Episode retry and rejects a changed embedding profile after reopen.

Lexicon is uninitialized in this worktree, so there is no current Arcana
snapshot. The repository's executable dependency guard checks all 23
packages and 76 production edges. Local Markdown links are checked
directly because the shared documentation policy file is absent.
