from __future__ import annotations

from pathlib import Path
from typing import Any

from .files import D03Error
from .manifest import CANDIDATE, PREVIOUS
from .state import _generation_path, _read_selection, _read_state, _write_state
from .validation import validate_generation


def reconcile(root: Path) -> dict[str, Any]:
    """Resolve a persisted fake selector intent after an interrupted process."""
    state = _read_state(root)
    selected = _read_selection(root)
    if state["phase"] != "SelectionPending":
        if selected != state["selected_generation_id"]:
            raise D03Error("simulated selection and rehearsal state disagree")
        return state

    pending = state["pending_transition"]
    source = pending["from_generation_id"]
    target = pending["to_generation_id"]
    if selected not in {source, target}:
        raise D03Error("actual simulated selection is outside the pending transition")
    operation = pending["operation"]
    if operation == "rehearsal":
        if selected == source:
            state["phase"] = "StagedReadOnly"
            state["health_check"] = "not-run"
        else:
            state["phase"] = "CandidateFenced"
            state["health_check"] = "not-run"
    elif operation == "rollback":
        if selected == target:
            validate_generation(
                _generation_path(root, target), state["previous_manifest_sha256"]
            )
            state["phase"] = "PrewriteRolledBack"
            state["health_check"] = "previous-generation-verified"
        else:
            state["phase"] = "CandidateFenced"
            state["health_check"] = "not-run"
    else:
        raise D03Error("unknown pending synthetic selector operation")
    state["selected_generation_id"] = selected
    state["pending_transition"] = None
    _write_state(root, state)
    return state
