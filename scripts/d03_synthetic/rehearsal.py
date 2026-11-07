from __future__ import annotations

import os
from pathlib import Path
from typing import Any

from .engine import _read_selection, _read_state, _validate_candidate, _validate_previous, _write_selection, _write_state, inspect_state
from .files import D03Error, checked_root
from .recovery import reconcile

_TRANSITION_FAILPOINTS = {
    "crash_before_selection_replace", "crash_after_selection_replace",
    "simulate_selector_replace_unavailable",
}


def _crash(code: int) -> None:
    os._exit(code)


def _start_transition(state: dict[str, Any], operation: str, source: str, target: str) -> None:
    state["phase"] = "SelectionPending"
    state["pending_transition"] = {
        "operation": operation,
        "from_generation_id": source,
        "to_generation_id": target,
    }


def _finish_rehearsal_transition(root: Path, state: dict[str, Any]) -> None:
    state["selected_generation_id"] = state["candidate_generation_id"]
    state["pending_transition"] = None
    state["phase"] = "CandidateFenced"
    state["health_check"] = "not-run"
    _write_state(root, state)


def rehearse(root_path: str | Path, failpoint: str | None = None) -> dict[str, Any]:
    """Simulate selector replacement inside the fake root; no product opener reads it."""
    allowed = _TRANSITION_FAILPOINTS | {None, "health_check"}
    if failpoint not in allowed:
        raise D03Error("unsupported rehearsal failpoint")
    root = checked_root(root_path)
    state = reconcile(root)
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

    _start_transition(
        state, "rehearsal", state["previous_generation_id"], state["candidate_generation_id"]
    )
    _write_state(root, state)
    if failpoint == "crash_before_selection_replace":
        _crash(81)
    if failpoint == "simulate_selector_replace_unavailable":
        _write_selection(root, state["candidate_generation_id"], simulate_unavailable=True)
    _write_selection(root, state["candidate_generation_id"])
    if failpoint == "crash_after_selection_replace":
        _crash(82)
    _finish_rehearsal_transition(root, state)

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


def rollback_prewrite(
    root_path: str | Path, failpoint: str | None = None
) -> dict[str, Any]:
    if failpoint not in _TRANSITION_FAILPOINTS | {None}:
        raise D03Error("unsupported rollback failpoint")
    root = checked_root(root_path)
    state = reconcile(root)
    if state["write_boundary_crossed"] is not False or state["accepted_new_writes"] != "0":
        raise D03Error("blind rollback after an accepted new-generation write is forbidden")
    if state["writer_gate"] != "closed" or state["writers_stopped"] is not True:
        raise D03Error("prewrite rollback requires the synthetic writer gate to stay closed")
    if state["phase"] not in {
        "StagedReadOnly", "CandidateFenced", "Rehearsed", "PrewriteRolledBack"
    }:
        raise D03Error("prewrite rollback is unavailable from this synthetic phase")
    selected = _read_selection(root)
    if selected not in {state["candidate_generation_id"], state["previous_generation_id"]}:
        raise D03Error("simulated selection is outside the recorded cohort")
    _validate_previous(root, state)
    if selected == state["previous_generation_id"]:
        state["selected_generation_id"] = selected
        state["phase"] = "PrewriteRolledBack"
        state["health_check"] = "previous-generation-verified"
        _write_state(root, state)
        return inspect_state(root)

    _start_transition(
        state, "rollback", state["candidate_generation_id"], state["previous_generation_id"]
    )
    _write_state(root, state)
    if failpoint == "crash_before_selection_replace":
        _crash(83)
    if failpoint == "simulate_selector_replace_unavailable":
        _write_selection(root, state["previous_generation_id"], simulate_unavailable=True)
    _write_selection(root, state["previous_generation_id"])
    if failpoint == "crash_after_selection_replace":
        _crash(84)
    state["selected_generation_id"] = state["previous_generation_id"]
    state["pending_transition"] = None
    state["phase"] = "PrewriteRolledBack"
    state["health_check"] = "previous-generation-verified"
    _write_state(root, state)
    return inspect_state(root)
