# P5-C project contract

The canonical `GatewayRuntime` now owns the trusted, pre-mount project lifecycle seam:

- `create_project_environment_rel`
- `inspect_project_adoption_source`
- `adopt_project_environment_rel`
- `validate_project_environment_cva`
- `reconcile_rel_file`

The lifecycle methods use the gateway lifecycle lock, exact mounted-file identity, and
the existing physical `MountClaim`; a path already mounted by this gateway is refused.
Creation and adoption continue to use the existing project repository preparation and
correlation code, so durable owner identity, project revision association, and storage
format are unchanged. Reconciliation is the unmounted path operation and is distinct
from mounted/quiesced reconciliation.

The APIs are trusted embedding-service controls. They are not instance/user endpoints,
do not expose mutable CVA handles, and do not construct or forward through
`ReliquaryRuntimeHost`. Host integration must perform its own path admission and
authorization before calling them.

Verification: the Phase 4 coverage guard and `git diff --check` pass. `cargo fmt --check`
and `cargo check` were started but exceeded the short third-party-cache time budget;
focused Rust tests were not started for the same reason. Existing mounted project tests
continue to cover exact-owner and grant enforcement; the new unmounted path gate fails
closed for mounted/locked files.
