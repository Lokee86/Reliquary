# D02 current-only reader probe

This offline assurance binary is outside the shipping package graph. It enables the owner-session d02-observation feature only in this standalone package; the feature is disabled by default for product consumers.

The observer opens one shared read-only Container, constructs the current REL or PHY owner session from that same handle, and retains the shared lease through page export and finalization. It hashes physical bytes through the held handle, checks that the candidate path still names that file before and after observation, and emits the owner identity and clocks observed from the target. The reported global version is the highest allocated physical version, not an accepted-cut inventory.

Each invocation emits at most one page of current Memory IDs: at most 128 IDs and 4 KiB of raw ID payload. An optional --memory-cursor continues pagination. Cursors bind to owner ID, the held-file SHA-256, history generation, Memory version, family, and ascending ID order, and are limited to 512 bytes. The observer does not read revision bodies or histories.

The ordinary current-only owner open replays the complete owner before these bounded page APIs are reached. This probe does not claim that the owner replay or full-file hashing is size-bounded. It does not call historical_proof, so the proof record cap cannot truncate or gate the Memory-ID page.

The probe always returns INCOMPLETE, emits no semantic parity rows, and marks all five PG-C01 fixture families and exhaustive cut inventory incomplete. The Memory-ID page is only a partial identity observation; revisions, bodies, lifecycle, provenance and product-family mappings remain incomplete. The remaining fixture schema families are synthetic and do not define real product fields or source bindings. Owner identity is not principal authorization.

Synthetic Rust tests create disposable current-format REL and PHY owners. They check same-handle hash/path identity behavior and read-only output, then build 129 direct synthetic Memories to verify the 128-item/4-KiB page bound, cursor continuation and context binding.

The exact schema/fixture inputs remain pinned to PG-C01 commit 0f824a09fe01f5ff327ab56f3296f36d8f4d7c87. No converter crate or converter traversal is imported. --output uses exclusive creation and refuses existing destinations; it never writes to the candidate.
