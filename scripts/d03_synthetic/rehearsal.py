from __future__ import annotations

from pathlib import Path
from typing import Any

from .engine import (
    _read_selection,
    _read_state,
    _validate_candidate,
    _validate_previous,
    _write_selection,
    _write_state,
    inspect_state,
)
from .files import D03Error, checked_root


def rehearse(root_path: str | Path, failpoint: str | None = None) -> dict[str, Any]:
    """Simulate routing inside the fake root; no product opener reads its pointer."""
    if failpoint not in {None, "before_selection_replace", "after_selection_replace", "health_check"}:
        raise D03Error("unsupported rehearsal failpoint")
    root = checked_root(root_path)
    state = _read_state(root)
    if state["phase"] != "StagedReadOnly":
        raise D03Error("only a staged synthetic candidate can be rehearsed")
    if state["writer_gate"] != "closed" or state["writers_stopped"] is not True:
        raise D03Error("writer stop is not proven in the synthetic state")
    if state["pending_host_outcome"] not in {"held", "drained"}:
        raise D03Error("unknown host outcome is not safely held")
    if state["write_boundary_crossed"] is not False or state["accepted_new_writes"] != "0":
        raise D03Error("rehearsal refuses a crossed new-write boundary")
    if _read_selection(root) != state["previous_generation_id"]:
        raise D03Error("rehearsal expects the previous synthetic generation to remain selected")
    _validate_candidate(root, state)
    _validate_previous(root, state)

    state["phase"] = "ReleaseStopped"
    _write_state(root, state)
    if failpoint == "before_selection_replace":
        state["phase"] = "RehearsalFailedOldActive"
        state["health_check"] = "not-run"
        _write_state(root, state)
        raise D03Error("injected failure before simulated selection replacement; old generation remains selected")

    _write_selection(root, state["candidate_generation_id"])
    state["selected_generation_id"] = state["candidate_generation_id"]
    if failpoint == "after_selection_replace":
        state["phase"] = "CandidateFenced"
        state["health_check"] = "not-run"
        _write_state(root, state)
        raise D03Error("injected failure after simulated selection replacement; candidate remains fenced")
    try:
        _validate_candidate(root, state)
        if failpoint == "health_check":
            raise D03Error("injected synthetic health-check failure")
    except D03Error:
        state["phase"] = "CandidateFenced"
        state["health_check"] = "failed"
        _write_state(root, state)
        raise
    state["phase"] = "Rehearsed"
    state["health_check"] = "passed"
    _write_state(root, state)
    return inspect_state(root)


def rollback_prewrite(root_path: str | Path) -> dict[str, Any]:
    root = checked_root(root_path)
    state = _read_state(root)
    if state["write_boundary_crossed"] is not False or state["accepted_new_writes"] != "0":
        raise D03Error("blind rollback after an accepted new-generation write is forbidden")
    if state["writer_gate"] != "closed" or state["writers_stopped"] is not True:
        raise D03Error("prewrite rollback requires the synthetic writer gate to stay closed")
    if state["phase"] not in {
        "StagedReadOnly", "Rehearsed", "RehearsalFailedOldActive", "CandidateFenced", "PrewriteRolledBack"
    }:
        raise D03Error("prewrite rollback is unavailable from this synthetic phase")
    if _read_selection(root) not in {state["candidate_generation_id"], state["previous_generation_id"]}:
        raise D03Error("simulated selection is outside the recorded cohort")
    _validate_previous(root, state)
    _write_selection(root, state["previous_generation_id"])
    state["selected_generation_id"] = state["previous_generation_id"]
    state["phase"] = "PrewriteRolledBack"
    state["health_check"] = "previous-generation-verified"
    _write_state(root, state)
    return inspect_state(root)