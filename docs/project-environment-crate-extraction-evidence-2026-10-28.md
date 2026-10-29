# Project Environment crate extraction evidence

Baseline: specification commit `8f78c3c122a2c087abf1030a757e40fb0ba67ee3`; the pre-migration focused suite was reported as 18/18. The unrelated Graph evidence modification was present before extraction and remains outside this change.

The compiled `reliquary-project-environment` crate owns Git, Lore, repository preparation, file reads and upload decisions. Root retains creation/adoption, durable correlation and validation; Gateway retains authorization and publication. Archive remains the owner of reference and correlation types. No mutable Root or Gateway handle enters the crate. The Root shim re-exports the consumed operations and preserves the public mode type.

## Validation

- Crate check passed with `--locked --offline`; external consumer tests passed 2/2, including real Git preparation and mode/checkpoint gating.
- Focused Root project-environment tests passed 18/18; Gateway ambient authorization tests passed 17/17.
- Workspace all-target check passed; locked offline CLI tests passed 32/32. Formatting and documentation checks passed.
- The complete Root suite passed 1,457 tests with 5 ignored using local networking and a short repository-local temporary path. A restricted-network attempt failed local HTTP tests. A subsequent run with a longer temporary path completed 1,454 passed, 3 failed and 5 ignored; the three external Gateway failures passed in a 7/7 rerun and the full suite passed using the shorter path.
- `cargo tree` shows Archive as the only direct internal dependency. No backend references a Root or Gateway handle. Displaced backend files were removed.
- The architecture guard's policy and other self-checks passed; the semantic ownership map now includes the crate source and consumer tests, but Pitlord reported owner errors against six `src/context_engine` paths absent from the checkout. This graph result cannot establish current ownership; fresh graph verification remains an independent limitation. The specification records the prior scanner timeout.
