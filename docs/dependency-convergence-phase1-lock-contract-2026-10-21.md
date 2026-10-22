# Phase 1 file lease contract

This contract records the implemented Phase 1 D3 file authority and publication rules.

## Lease primitive and lifetime

On Unix, Container uses `std::fs::File::try_lock` for an exclusive writer lease
and `try_lock_shared` for a stable read lease. On Windows, it uses nonblocking
`LockFileEx` on a one-byte range at offset `i64::MAX`, immediately beyond the
largest possible Windows file byte. The reserved range does not lock payload
bytes, so ordinary readers and diagnostics can still inspect container data
while an owner is mounted. Both primitives lock the same opened file object,
not a separate pathname. The `Lease` retains a duplicate of the locked handle
in an `Arc`; a `PhysicalReadSource` shares that Arc so dropping the original
Container cannot release the lease before the source. Closing the last handle
or crashing out of the process releases the OS lock. Contention maps to the
typed `WriterConflict` error. A platform/filesystem that reports locking as
unsupported fails closed with `LockingUnsupported`.

An internal `PhysicalReadSource` duplicates the already leased handle and keeps
it behind an Arc-owned read-only facade. It does not acquire a second OS lock
that would conflict with the mounted writer. The source and writer share an
atomic in-process gate: cursor reads hold a shared count, and every raw physical
mutation takes the exclusive sentinel with a nonblocking compare-and-swap. A
conflicting local operation returns `UnstableSnapshot`; no partial read or
write proceeds.

On Unix, the primitive locks the opened inode, so ordinary hard links and
symlink-resolved opens of that inode contend. On Windows, the byte-range lock
is attached to the file object; hard links and symlink-resolved opens address
the same object and contend. The lock therefore follows the object across rename,
but does not by itself reserve the old pathname against replacement. Unix
revalidation compares the path's device/inode with the held handle. Windows
revalidation compares the volume serial and file index from
`GetFileInformationByHandle` on the current path's handle with the leased
handle's values. Callers must keep the handle alive through publication and
validate source identity at the publication boundary. Read snapshots hold a shared lock for their lifetime;
writers cannot mutate while that lease is held. Lock ordering is one container
at a time; multi-container callers acquire in stable path order and release in
reverse order. A writable runtime mount already owns the writer lease, so a
second writable mount fails with `WriterConflict` and a read snapshot must
either wait/retry or return the same typed conflict.

New destination files use exclusive create (`create_new`) and immediately take
the exclusive lease before writing the header. Existing files take the lease
before writable inspection or replay. In-place repair requires the writer
lease and a separately authorized operation; the lease alone does not authorize
repair. Ordinary open reports an incomplete tail with its complete-prefix end.

## Current limits and follow-up

The descriptor lock prevents concurrent access through another cooperating
Container open, including aliases that resolve to the same object. It cannot
prevent a hostile process that ignores advisory locks from writing directly,
and Unix cannot prevent an unrelated process from replacing a pathname while
an already-open descriptor remains valid. Mutating and publication call sites
must revalidate that the pathname still names the leased object immediately
before acting and refuse stale/replaced sources. Filesystems without reliable
OS lock support are refused. Destination publication across rename remains the
responsibility of the rewrite layer, which must hold the destination lease and
check the target atomically; this primitive does not claim to solve a check-
then-rename race by itself.

## Purpose

Record the concrete OS lock and access contract for the physical storage boundary.

## Overview

The preceding contract defines scope, ownership and verification requirements.

## Related docs

[Phase 1 specification](dependency-convergence-phase1-storage-foundation-spec-2026-10-21.md), [Storage format](storage-format.md).

## Notes

The V2 encoding remains unchanged. Current behavior and unfinished acceptance are recorded separately.

## Owner-local read evidence

Owner stores that already hold a Container receive a concrete, non-writable
`PhysicalReadSource` derived from that descriptor. It does not acquire a second
OS lock or expose the File. Duplicate handles retain the existing OS lease.
Positional reads keep independent cursor offsets. A shared atomic gate pins each
active cursor against all raw mutations of the originating session; concurrent
mutation refuses with `UnstableSnapshot`. This permits bounded Freshness and
historical replay inside a held owner without reopening its path as a second
writer. Independent inspection still takes its own shared OS lease.

## Destination publication

Fresh-history rewrite creates and validates a unique staging file in the output
directory, retains its exclusive writer lease through semantic verification, and publishes
with atomic hard-link creation. Existing destination entries (including aliases
and symlinks) cannot be overwritten. Unsupported hard-link filesystems refuse.
Unix synchronizes the destination directory after publication. Reconciliation
promotion revalidates the canonical digest under its writer lease and retains
candidate and backup leases across replacement; relocated handles verify the
published identity before continuing. All cooperating publication paths must
retain their source and destination evidence through the publication boundary.

On Windows, reconciliation replacement uses the standard library's `fs::rename`
while both file leases remain held. A direct MoveFileExW call with either
MOVEFILE_REPLACE_EXISTING alone or together with MOVEFILE_WRITE_THROUGH
refuses replacement of open files on the tested Windows filesystem; the standard
library's rename succeeds under the same handles and locks. The candidate is
synced before and after replacement. Windows directory synchronization is not
claimed; unlike Unix directory sync_all, this does not promise directory-entry
durability against sudden power loss. Process-level atomic replacement and
exclusion remain required and tested.
