# Migration and Reconciliation

Parent index: [Reliquary operator manual](INDEX.md)

## Purpose

Move older REL/PHY files into the current format and merge divergent copies without bypassing semantic-owner invariants.

## Overview

Migration and reconciliation solve different problems:

- **Migration** converts one old-format file into one current-format file.
- **Reconciliation** combines two copies of the same logical REL owner.

Neither operation is an in-place overwrite of the source.

## Migrate an older file

CLI:

```text
cargo run --manifest-path cli/Cargo.toml -- migrate <source> <output>
```

Library:

```text
migrate_file(source, output)
```

Current migration accepts supported legacy 16-byte Project CVA and earlier 24-byte typed REL/PHY formats and writes a separate current identified file.

After migration:

1. open/verify the output normally.
2. keep the source until the output has been validated.
3. update the application reference to the new file intentionally.

Migration semantically replays owner state; it does not copy obsolete physical layout blindly.

## Reconcile divergent REL copies

Library:

```text
Cva::reconcile(left, right, output)
```

Use reconciliation only when both sides represent the same logical durable owner.

Possible broad outcomes are:

- identical state.
- strict extension.
- true divergence requiring semantic replay.
- typed conflict that fails closed.

True divergence writes a fresh output and replays semantic owners through their normal APIs with fresh clocks.

Current replay ordering ensures dependencies exist before references:

```text
source/archive state
→ Memories
→ Entities
→ typed Graph transactions
→ dependent operational/derived owners
```

Entity associations therefore cannot replay before their Entity endpoint exists.

## Project repository history

Project-file history is not the REL semantic timeline. Lore/Git repository revision correlations have separate compatibility rules. Do not treat reconciliation as a Git/Lore merge operation.

## Conflicts

A conflict means Reliquary cannot preserve both histories under the current owner contract without choosing semantics on the caller's behalf.

Do not “resolve” a typed conflict by copying whichever physical file is newer unless the application explicitly intends to discard the other semantic history.

## After reconciliation

Open the output normally. Derived acceleration state may need rebuilding according to the returned reconciliation result and normal stale-state rules.

## Related docs

- [Rust API](../api.md)
- [Storage format](../storage-format.md)
- [ADR 0019](../decisions/0019-cloud-backed-cva-reconciliation.md)
- [Current limitations](../current-limitations.md)

## Notes

Current reconciliation is a REL operation. Do not generalize it into cross-owner Graph composition; cross-owner composition is a separate higher-level problem.


## Repair a retained REL copy

`reliquary rel repair SOURCE OUTPUT --keep-profile PROFILE_ID` creates a new disposable copy, explicitly frames legacy import evidence where present, retires other profiles, and verifies unchanged semantic/version/profile identity state after reopen. `--prune-retired` explicitly removes eligible retired numeric payload through the owner prune operation; shared references and authenticated prefixes reject. `--reclaim` performs conservative maintenance on a further temporary copy. It does not re-embed data. The selected profile must have complete current Archive and Memory coverage. Existing outputs are rejected; authoritative fixtures are promoted only after separate retrieval and provenance verification.
