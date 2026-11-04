# Reliquary clean rebuild

Independent REL 2.1 implementation on `rebuild/clean-architecture`. See [AGENTS.md](AGENTS.md) for the authoritative external execution plan, capability tracker, architecture boundaries, and mandatory verification commands. The selected package graph is [architecture/target-crate-dag-2026-11-03.json](architecture/target-crate-dag-2026-11-03.json); run `python scripts/check_architecture.py` before accepting an increment.

A successful workspace test run is a compilation/behavior checkpoint, **not** completion of product gates G1–G5. The pinned reference and old-format corpus remain separate.
