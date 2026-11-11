# Reliquary architecture

Parent index: [Documentation](../INDEX.md)

## Purpose

Navigate the current Reliquary architecture by responsibility. These pages describe the implementation at the repository revision under review; the package DAG file is the exact production dependency contract.

## System and ownership

- [System architecture](system.md) — package groups, composition, dependency direction, and external boundaries.
- [Storage and durable state](storage.md) — Container framing and ownership of persistent data.
- [Owner sessions](owner-session.md) — authorization-scoped reads, single-writer authority, grouped commits, and replay.
- [Application workflows](application.md) — cross-owner product workflows and publication coordination.
- [Semantic processing](semantic-processing.md) — bounded semantic proposals and derived decisions.
- [Gateway and host lifecycle](gateway.md) — mounted owners, principal-scoped Instances, queues, host contracts, and recovery.
- [Switchboard](switchboard.md) — runtime configuration, protected credentials, and provider routing.

## Related implementation contracts

- [Approved package dependency DAG](../../architecture/target-crate-dag-2026-11-03.json)
- [Architecture guard](../../scripts/check_architecture.py)
- [Root public API facade](../../src/lib.rs)



## Direct Files

<!-- doc-ledger:files:start -->

- [application.md](application.md) - Application documentation.
- [gateway.md](gateway.md) - Gateway documentation.
- [maintainer-map.md](maintainer-map.md) - Maintainer Map documentation.
- [owner-session.md](owner-session.md) - Owner Session documentation.
- [semantic-processing.md](semantic-processing.md) - Semantic Processing documentation.
- [storage.md](storage.md) - Storage documentation.
- [switchboard.md](switchboard.md) - Switchboard documentation.
- [system.md](system.md) - System documentation.
<!-- doc-ledger:files:end -->

## Stub Files

<!-- doc-ledger:stubs:start -->
<!-- doc-ledger:stubs:end -->

## Direct Folders

<!-- doc-ledger:folders:start -->
<!-- doc-ledger:folders:end -->