# Scope-free REL creation acceptance

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Record the Core 2.1 creation cut and the file compatibility checks from November 18, 2026.

## Overview

The [homogeneity specification](../planning/rel-homogeneity-spec-2026-11-18.md) changes the creation input to `Rel::create(path)` and `reliquary rel create PATH`. The current REL writer uses a fresh owner ID and the neutral `Unspecified` header value. Scope remains readable on existing files.

## Fresh-process evidence

Core `36c6383a` created three empty 206-byte REL 2.1 files using the original scope option. Separate processes from the updated build reported the same format, owner IDs and scope in both `info` and `verify`. All three SHA-256 hashes matched the pre-change measurements.

| Original scope | Owner ID | SHA-256 |
| --- | --- | --- |
| Organization | `1b71d2ae6b604d3d863df845701e66c2` | `607c6a6c01a0ed032fda82210dbf8e6f7f49429d422ab6ff2aba56bab6853d3d` |
| Project | `d3049c0eb490417399c4d813f8a11485` | `72147d0e5353574edbf5dc80e8092841b95d6f6c9a02586c87f765ad33b7908b` |
| Connection | `12051e77f6c9494f845074939ebb8f20` | `2a137b30e1200271b7b17f253c349163083bb75b1adb54401e772d5b37782f80` |

The new CLI created a neutral REL and a fresh process verified its owner ID. The CLI suite checked that an occupied path retained the original bytes and that supplying the retired `--scope` option left the requested path uncreated. The maintenance fixture kept the original Organization scope, owner ID and dependency history through same-owner rewrite and storage reclamation.

## Integration checks

| Check | Result |
| --- | --- |
| Workspace compilation and locked offline tests | 688 tests passed. The existing cross-process lease test was filtered because this Windows account lacks symbolic-link privilege. |
| Locked offline CLI and detached Root consumer | 17 CLI and 16 consumer tests passed. The separately invoked G4 runner passed the normally ignored consumer process test. |
| Fresh-process G4 gate | Passed on a deterministically generated synthetic corpus. The exported Lantern and Cascade RELs kept their Project and Connection metadata and their original bytes. |
| Workspace and CLI formatting, package dependency guard, and documentation entrypoint links | Passed. |

## Related docs

- [Homogeneity execution plan](../planning/rel-homogeneity-plan-2026-11-18.md)
- [Public Root API](../public-product-api.md)
- [CLI reference](../reference/cli.md)

## Notes

The synthetic fixtures and fresh-process output are under the ignored `.test-tmp/rel-homogeneity-2026-11-18/` directory. The Hermes repository records its matched native and Python verification with the vendored Core revision.
