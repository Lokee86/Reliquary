# V06 acceptance

V06 covers owner-cut Memory history, exclusive native import, and verified-copy maintenance for REL 2.1 and PHY. The implemented rewrite preserves the owner identity, history generation, clocks, tombstones and retained provenance.

- **History.** Owner-qualified reads distinguish missing revisions from visible revisions and tombstones at the requested cut. Future cuts fail. The history workflow tests cover owner authorization and pre-tombstone cuts. The promotion regression reopens the promoted PHY and verifies retained revision metadata with absent payload bytes.
- **Native import.** A durable exclusive epoch fences ordinary writes and maintenance. The application integration test restarts after staging, resumes scoped work, refuses premature completion, restarts after Freshness settlement and verifies exact retry and independent reopen.
- **Maintenance.** Terminal Memory rewrites stage and sync a separate copy and independently reopen it before publication or offline replacement. Active writers and destination collisions block publication. Replacement retains the original bytes at the backup path. Tests exercise REL and PHY reopen and post-promotion writes.
- **Reconciliation.** Exact physical prefixes produce an independently admitted copy of the extension. Divergence is refused and both owners are preserved. An invalid owner never publishes a destination.

This scope supports terminal Memory payload removal, version-preserving rewrite and exact-prefix reconciliation. The reference implementation also has fresh-history repacking, divergent semantic replay and additional pruning domains. Those operations have no admitted implementation here. G2 has its own semantic parity requirements.

The locked offline workspace run passed 504 tests across 47 suites. Formatting, whitespace and the 23-package, 76-edge production architecture guard passed.
