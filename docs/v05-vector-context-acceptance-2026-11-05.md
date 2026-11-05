# V05 vector publication and bounded context

V01, S07, and S08 are accepted prerequisites.

| Requirement | Evidence |
| --- | --- |
| Source-bound REL and PHY vectors | Gateway checks the owner grant before inference and again before publication. Owner publication validates active profile, exact Memory revision and body, dimensions, normalization, and existing row bytes. Tests cover REL and PHY isolation, stale and invalid inputs, orphan packed payload recovery, and retry after reopen without repeat inference. |
| Archive generation provenance | Archive retains fragment definitions and first-acceptance source versions. Provider requests use bounded batches and publication requires complete fragment coverage. Owner reopen validates each generation's fragment definitions, captured cut, mapping watermark, and profile-qualified rows. The Gateway fixture publishes and reuses a generation after reopen. |
| Inference outside owner locks | Gateway captures immutable source inputs, runs inference, and reacquires the writer to validate and commit. Vector and compaction fixtures reenter owner access from inference callbacks. |
| Repository evidence and bounded context | The context workflow uses the host's native transcript, separate REL and PHY Memory evidence, and exact VCS attachment bytes. It checks request budgets and checkpoint lineage. The Gateway fixtures cover context with an empty REL transcript, read-only reopen, foreign Instance and PHY denial, and project grant revocation after preparation. |

Archive clock and typed record replay changes are shared foundations for later import work. V06 completion still requires durable import settlement and verified-copy maintenance. G2 remains open.

The isolated V05 candidate passed 491 tests across 47 suites. Locked offline workspace compilation and metadata, formatting, whitespace, and the architecture guard passed with 23 production packages and 76 edges. The Archive reopen test also rejects a generation with a forged future source cut.
