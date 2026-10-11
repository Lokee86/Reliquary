# Freshness semantic policy module — 2026-10-11

Parent: [Documentation index](INDEX.md).

## Purpose

Implement the graph-independent numerical and candidate-reduction core frozen by the v1 semantic Freshness policy.

## Overview

**Date:** 2026-10-11. **Status:** Step 5 complete; live F8 remains disabled pending later producer, durability, and release-gate steps.

## Related docs

- [Live semantic Freshness calibration and v1 policy](freshness-semantic-calibration-2026-10-11.md)
- [Live semantic policy/API contract](freshness-live-semantic-policy-contract.md)
- [Implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)

## Scope

Step 5 implements the graph-independent semantic policy frozen in the v1 calibration record. The module is `src/freshness/semantic_policy.rs` and is exposed through `freshness::semantic_policy`.

It owns:

- versioned policy constants and canonical descriptor/digest;
- inclusive cosine thresholding and the bounded linear bonus curve;
- deterministic binary64 cosine calculation and input validation;
- source/target vector profile and generation compatibility checks;
- strongest-match-per-Memory reduction, deterministic tie ordering, candidate limiting, and local effect calculation.

It does not own Archive ingestion, vector storage, lifecycle eligibility, Freshness durability, or graph propagation. Those remain explicit seams for later steps.

## Frozen behavior

The default policy is version 1, numerical revision 1, eligibility revision 1, threshold `0.75`, bonus cap `10`, and candidate limit `64`. A cosine at the threshold receives bonus `1`; values below the threshold receive `0`. The curve is monotone and capped at `10`.

Usable candidates must have finite cosine in `[-1, 1]`, compatible profile evidence, a nonzero generation, and no self-source exclusion. Invalid rows are dropped before dedupe and do not consume the candidate limit. A target keeps its strongest cosine; exact ties use fragment-id bytes and then vector-binding bytes. Ranking occurs by bonus, cosine, and `MemoryId` bytes before the limit is applied. Returned effects are sorted by `MemoryId` bytes.

The canonical descriptor is:

`{"bonus_cap":10,"candidate_limit":64,"curve":"inclusive-linear-v1","eligibility_revision":1,"numerical_revision":1,"threshold":0.75,"version":1}`

Its SHA-256 digest is `5886dc23e386664b3dbef03a116dbfab5567ad2998be0e21e07c4895ab1d261f`.

## Verification

Focused unit tests cover threshold boundaries, monotonicity, cap behavior, strongest-match and tie reduction, candidate ranking and limiting, incompatible profile/generation evidence, invalid inputs, cosine validation, and descriptor stability.

Documentation impact: this module adds the implementation record for Step 5 and does not change user-facing Freshness limits or CLI behavior.

Architecture impact: semantic numerical policy remains under the existing Freshness owner; no Archive, Graph, vector-storage, or durability ownership moves.

## Notes

This module provides a deterministic pure-policy seam for later accepted-turn production and bounded search. It does not activate semantic reinforcement or change existing Freshness scoring principals.
