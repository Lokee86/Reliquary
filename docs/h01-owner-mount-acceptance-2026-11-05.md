# H01 owner mount and Instance isolation acceptance (2026-11-05)

H01 is accepted at rebuild commit `HEAD` after G2 acceptance. This checkpoint is limited to owner mounting and authenticated Instance isolation; it does not claim H02 queue, H03 host streaming, H04 worker lifecycle, or G3 acceptance.

## Accepted behavior

- `Gateway` owns the one canonical mounted-owner registry. A second mount for the same owner is rejected without replacing the first session, and a writable mount holds the Container writer lease so another mutable owner session cannot open concurrently.
- REL and PHY mounts validate the durable owner identity against the supplied graph admission. Grants cannot name an unmounted owner or a mounted owner of the wrong kind.
- Trusted host configuration binds bearer credentials to a fixed principal and immutable REL/PHY grant sets. Request-facing `Instance` values have no API for adding or changing grants.
- Owner authorization and credential revocation are checked before registry lookup and again immediately before mutable owner work. Revocation invalidates both existing Instances and future authentication.
- Mutable Container/store handles do not escape Gateway. Gateway keeps the owner-session writer behind its owner-local mutex and exposes only bounded operations.

## Focused evidence

The `reliquary-runtime-gateway` tests cover:

- duplicate owner mounting preserves the canonical session;
- a writable mount excludes a second mutable owner session;
- ungranted access is denied before owner lookup;
- grants cannot name unmounted owners;
- invalid principals, malformed credentials, and duplicate credentials fail closed;
- host credential binding fixes principal and owner grants;
- revocation fences existing and future Instances;
- ungranted Instances cannot submit queue work, claim worker input, or read generation state.

The workspace architecture guard remains the executable proof that Gateway depends on owner-session rather than physical stores and that no reverse edge creates another owner authority.

## Verification

Run from `C:\!bin\workspace\Reliquary-rebuild` with the repository-local `target-codex-check` build directory because the historical `target` link is not writable in this environment:

```powershell
python scripts/check_architecture.py
cargo fmt --all --check
cargo check --workspace --locked --offline -j 1
cargo test --workspace --locked --offline -j 1 -- --test-threads=1
```

All commands passed at acceptance. Existing non-fatal compiler/linker warnings remain outside H01 and do not weaken its behavior checks.
