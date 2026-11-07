# Reliquary 2.1 public product API

Downstream Rust consumers depend on the root `reliquary-memory` package. Create
or open owners with `Rel` and `Phylactery`; both are opaque read handles.
Writable work is mounted once in `Gateway` and reached only through an
authenticated product `Instance`.

```rust
use reliquary_memory::{
    Gateway, HostCredentialBinding, OwnerGrant, OwnerScope, Rel,
    TrustedHostAdmission,
};

let rel = Rel::create("project.rel", OwnerScope::Project)?;
let gateway = Gateway::new();
let owner = gateway.mount_rel(&rel, true)?;
let token = load_secret_from_host_configuration();
let admission = TrustedHostAdmission::new([
    HostCredentialBinding::new("operator", &token, [OwnerGrant::ManageRel(owner)])
]);
let (requests, revocation) = admission.install(&gateway)?;
let instance = requests.authenticate(&token)?;
let management = instance.workspace_management(owner)?;
let health = management.health()?;
# Ok::<(), reliquary_memory::ProductError>(())
```

Tokens belong in host configuration, never request parameters, logs, or CLI
arguments. Grants are immutable. Revocation is checked again on reads and at
writer handoff/commit.

The separate `reliquary` CLI is REL-only. It emits JSON for successful reports,
uses nonzero exit status with a JSON error on failure, never overwrites an
output, and has no `cva` or implicit `phy` alias.

## Migration from the reference facade

- `Cva` → `Rel` (intentional hard cut; no alias).
- mutable root/store access → opaque owner inspection or authenticated
  `Instance`/`WorkspaceManagement` operation.
- caller-created replay evidence → durable current-owner admission.
- in-place maintenance → explicit plan plus verified, independently reopened
  no-replace copy.
- direct provider/worker CLI commands → configured trusted-host workflows.

Unsupported scenarios include 2.0 readers/conversion, cross-owner or PHY
reconciliation, arbitrary Dream/Insomnia maintenance, mutable public stores,
network daemons, and unrestricted provider administration.
