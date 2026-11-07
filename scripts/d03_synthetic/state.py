from __future__ import annotations

from pathlib import Path
from typing import Any

from .files import D03Error, atomic_write, canonical_bytes, parse_json, read_bytes, reject_reparse
from .manifest import CANDIDATE, PREVIOUS

STATE_FILE = ".d03-rehearsal-state.json"
SELECTION_FILE = ".d03-simulated-selection.json"
_PHASES = {
    "Unverified", "StagedReadOnly", "SelectionPending", "CandidateFenced",
    "Rehearsed", "PrewriteRolledBack",
}


def _state_path(root: Path) -> Path:
    return root / STATE_FILE


def _selection_path(root: Path) -> Path:
    return root / SELECTION_FILE


def _valid_digest(value: object) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def _write_state(root: Path, state: dict[str, Any]) -> None:
    try:
        data = canonical_bytes(state) + b"\n"
    except (TypeError, ValueError, RecursionError) as error:
        raise D03Error(f"rehearsal state is not canonical JSON: {error}") from error
    atomic_write(_state_path(root), data)


def _read_state(root: Path) -> dict[str, Any]:
    state_path = _state_path(root)
    reject_reparse(state_path, "rehearsal state")
    value = parse_json(read_bytes(state_path, "rehearsal state"))
    fields = {
        "simulation_only", "phase", "candidate_generation_id", "previous_generation_id",
        "manifest_sha256", "previous_manifest_sha256", "selected_generation_id",
        "writer_gate", "writers_stopped", "pending_host_outcome",
        "write_boundary_crossed", "accepted_new_writes", "activation_authorized",
        "health_check", "pending_transition",
    }
    if not isinstance(value, dict) or set(value) != fields:
        raise D03Error("rehearsal state has missing or unknown fields")
    if value["simulation_only"] is not True or value["activation_authorized"] is not False:
        raise D03Error("state is not a non-authorizing synthetic rehearsal")
    if not isinstance(value["phase"], str) or value["phase"] not in _PHASES:
        raise D03Error("unknown synthetic rehearsal phase")
    string_fields = (
        "candidate_generation_id", "previous_generation_id", "selected_generation_id",
        "writer_gate", "pending_host_outcome", "accepted_new_writes", "health_check",
    )
    for key in string_fields:
        if not isinstance(value[key], str):
            raise D03Error(f"state field {key} must be a string")
    if value["accepted_new_writes"] not in {"0", "1"}:
        raise D03Error("accepted write count must be a canonical synthetic count")
    if value["candidate_generation_id"] != CANDIDATE or value["previous_generation_id"] != PREVIOUS:
        raise D03Error("synthetic generation identifiers differ from the fixed fixture catalog")
    if value["selected_generation_id"] not in {PREVIOUS, CANDIDATE}:
        raise D03Error("selected generation is outside the fixed synthetic cohort")
    if not _valid_digest(value["previous_manifest_sha256"]):
        raise D03Error("recorded previous manifest digest is missing or invalid")
    if value["phase"] == "Unverified":
        if value["manifest_sha256"] is not None and not _valid_digest(value["manifest_sha256"]):
            raise D03Error("recorded candidate manifest digest is invalid")
    elif not _valid_digest(value["manifest_sha256"]):
        raise D03Error("staged candidate manifest digest is missing or invalid")

    if not isinstance(value["writers_stopped"], bool) or not isinstance(value["write_boundary_crossed"], bool):
        raise D03Error("writer boundary state must be boolean")
    if value["writer_gate"] != "closed":
        raise D03Error("synthetic harness state must keep the writer gate closed")
    if value["write_boundary_crossed"] != (value["accepted_new_writes"] != "0"):
        raise D03Error("write boundary flag and accepted write count disagree")

    pending = value["pending_transition"]
    if value["phase"] != "SelectionPending":
        if pending is not None:
            raise D03Error("pending selector intent exists outside SelectionPending phase")
        expected_selected = {
            "Unverified": PREVIOUS,
            "StagedReadOnly": PREVIOUS,
            "CandidateFenced": CANDIDATE,
            "Rehearsed": CANDIDATE,
            "PrewriteRolledBack": PREVIOUS,
        }[value["phase"]]
        if value["selected_generation_id"] != expected_selected:
            raise D03Error("phase and selected synthetic generation disagree")
    else:
        if not isinstance(pending, dict) or set(pending) != {
            "operation", "from_generation_id", "to_generation_id"
        }:
            raise D03Error("pending selector intent is malformed")
        operation, source, target = (
            pending["operation"], pending["from_generation_id"], pending["to_generation_id"]
        )
        if not all(isinstance(item, str) for item in (operation, source, target)):
            raise D03Error("pending selector intent fields must be strings")
        if (
            operation != "rehearsal" or (source, target) != (PREVIOUS, CANDIDATE)
        ) and (operation != "rollback" or (source, target) != (CANDIDATE, PREVIOUS)):
            raise D03Error("pending selector intent is outside the safe synthetic transitions")
        if value["selected_generation_id"] != source:
            raise D03Error("pending selector intent source differs from recorded selection")
    return value


def _write_selection(
    root: Path, generation_id: str, *, simulate_unavailable: bool = False
) -> None:
    if not isinstance(generation_id, str) or generation_id not in {PREVIOUS, CANDIDATE}:
        raise D03Error("unknown synthetic generation ID")
    if simulate_unavailable:
        raise D03Error("injected selector replacement unavailability; pending intent is retained")
    atomic_write(
        _selection_path(root),
        canonical_bytes({"simulation_only": True, "generation_id": generation_id}) + b"\n",
    )


def _read_selection(root: Path) -> str:
    selection_path = _selection_path(root)
    reject_reparse(selection_path, "simulated selection record")
    value = parse_json(read_bytes(selection_path, "simulated selection record"))
    if (
        not isinstance(value, dict)
        or set(value) != {"simulation_only", "generation_id"}
        or value["simulation_only"] is not True
        or not isinstance(value["generation_id"], str)
        or value["generation_id"] not in {PREVIOUS, CANDIDATE}
    ):
        raise D03Error("invalid simulated selection record")
    return value["generation_id"]


def _generation_path(root: Path, generation_id: str) -> Path:
    if not isinstance(generation_id, str) or generation_id not in {PREVIOUS, CANDIDATE}:
        raise D03Error("unknown synthetic generation ID")
    parent = root / "generations"
    reject_reparse(parent, "synthetic generations directory")
    if not parent.is_dir():
        raise D03Error("synthetic generations directory is missing")
    path = parent / generation_id
    reject_reparse(path, "synthetic generation directory")
    return path
