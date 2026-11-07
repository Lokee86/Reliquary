from __future__ import annotations

import os
from pathlib import Path
from typing import Any

from .files import (
    ROOT_MARKER,
    ROOT_MARKER_BYTES,
    D03Error,
    atomic_write,
    canonical_bytes,
    checked_root,
    parse_json,
    write_new,
)
from .manifest import CANDIDATE, PREVIOUS, create_generation
from .validation import validate_generation

STATE_FILE = ".d03-rehearsal-state.json"
SELECTION_FILE = ".d03-simulated-selection.json"


def _state_path(root: Path) -> Path:
    return root / STATE_FILE


def _selection_path(root: Path) -> Path:
    return root / SELECTION_FILE


def _write_state(root: Path, state: dict[str, Any]) -> None:
    atomic_write(_state_path(root), canonical_bytes(state) + b"\n")


def _read_state(root: Path) -> dict[str, Any]:
    state_path = _state_path(root)
    if state_path.is_symlink():
        raise D03Error("rehearsal state must not be a symlink")
    try:
        value = parse_json(state_path.read_bytes())
    except OSError as error:
        raise D03Error("rehearsal state is missing") from error
    fields = {
        "simulation_only", "phase", "candidate_generation_id", "previous_generation_id",
        "manifest_sha256", "previous_manifest_sha256", "selected_generation_id",
        "writer_gate", "writers_stopped", "pending_host_outcome",
        "write_boundary_crossed", "accepted_new_writes", "activation_authorized",
        "health_check",
    }
    if not isinstance(value, dict) or set(value) != fields:
        raise D03Error("rehearsal state has missing or unknown fields")
    if value["simulation_only"] is not True or value["activation_authorized"] is not False:
        raise D03Error("state is not a non-authorizing synthetic rehearsal")
    for key in ("phase", "candidate_generation_id", "previous_generation_id", "selected_generation_id", "writer_gate", "pending_host_outcome", "accepted_new_writes", "health_check"):
        if value[key] is not None and not isinstance(value[key], str):
            raise D03Error(f"state field {key} must be a string")
    if value["accepted_new_writes"] not in {"0", "1"}:
        raise D03Error("accepted write count must be a canonical synthetic count")
    if value["candidate_generation_id"] != CANDIDATE or value["previous_generation_id"] != PREVIOUS:
        raise D03Error("synthetic generation identifiers differ from the fixed fixture catalog")
    if not isinstance(value["writers_stopped"], bool) or not isinstance(value["write_boundary_crossed"], bool):
        raise D03Error("writer boundary state must be boolean")
    return value


def _write_selection(root: Path, generation_id: str) -> None:
    atomic_write(
        _selection_path(root),
        canonical_bytes({"simulation_only": True, "generation_id": generation_id}) + b"\n",
    )


def _read_selection(root: Path) -> str:
    selection_path = _selection_path(root)
    if selection_path.is_symlink():
        raise D03Error("simulated selection record must not be a symlink")
    try:
        value = parse_json(selection_path.read_bytes())
    except OSError as error:
        raise D03Error("simulated selection record is missing") from error
    if (
        not isinstance(value, dict)
        or set(value) != {"simulation_only", "generation_id"}
        or value["simulation_only"] is not True
        or not isinstance(value["generation_id"], str)
    ):
        raise D03Error("invalid simulated selection record")
    return value["generation_id"]


def _generation_path(root: Path, generation_id: str) -> Path:
    if not isinstance(generation_id, str) or generation_id not in {PREVIOUS, CANDIDATE}:
        raise D03Error("unknown synthetic generation ID")
    parent = root / "generations"
    if parent.is_symlink() or not parent.is_dir():
        raise D03Error("synthetic generations directory is missing or aliased")
    path = parent / generation_id
    if path.is_symlink():
        raise D03Error("synthetic generation path must not be a symlink")
    return path


def _validate_candidate(root: Path, state: dict[str, Any]) -> dict[str, Any]:
    return validate_generation(
        _generation_path(root, state["candidate_generation_id"]), state["manifest_sha256"]
    )


def _validate_previous(root: Path, state: dict[str, Any]) -> dict[str, Any]:
    return validate_generation(
        _generation_path(root, state["previous_generation_id"]), state["previous_manifest_sha256"]
    )


def _sync_tree(path: Path) -> None:
    for item in path.rglob("*"):
        if item.is_symlink():
            raise D03Error("symlink appeared while syncing synthetic generation")
        if item.is_file():
            with item.open("r+b") as stream:
                os.fsync(stream.fileno())


def stage(root_path: str | Path, failpoint: str | None = None) -> dict[str, Any]:
    """Create a synthetic-only candidate generation; no caller-provided files are accepted."""
    if failpoint not in {None, "before_sync", "after_sync"}:
        raise D03Error("unsupported stage failpoint")
    root = checked_root(root_path, create=True)
    root.mkdir()
    write_new(root / ROOT_MARKER, ROOT_MARKER_BYTES)
    generations = root / "generations"
    generations.mkdir()
    previous_digest = create_generation(generations / PREVIOUS, PREVIOUS, None)
    _sync_tree(generations / PREVIOUS)
    _write_selection(root, PREVIOUS)
    state: dict[str, Any] = {
        "simulation_only": True,
        "phase": "Unverified",
        "candidate_generation_id": CANDIDATE,
        "previous_generation_id": PREVIOUS,
        "manifest_sha256": None,
        "previous_manifest_sha256": previous_digest,
        "selected_generation_id": PREVIOUS,
        "writer_gate": "closed",
        "writers_stopped": True,
        "pending_host_outcome": "held",
        "write_boundary_crossed": False,
        "accepted_new_writes": "0",
        "activation_authorized": False,
        "health_check": "not-run",
    }
    _write_state(root, state)

    temporary = generations / ".d03-synthetic-candidate.tmp"
    candidate_digest = create_generation(temporary, CANDIDATE, PREVIOUS)
    if failpoint == "before_sync":
        raise D03Error("injected failure before candidate sync; old synthetic selection remains")
    _sync_tree(temporary)
    if failpoint == "after_sync":
        raise D03Error("injected failure after candidate sync; old synthetic selection remains")
    final = generations / CANDIDATE
    if final.exists():
        raise D03Error("candidate generation collision")
    os.replace(temporary, final)
    state["manifest_sha256"] = candidate_digest
    state["phase"] = "StagedReadOnly"
    _write_state(root, state)
    return inspect_state(root)


def validate(root_path: str | Path) -> dict[str, Any]:
    root = checked_root(root_path)
    state = _read_state(root)
    candidate = _validate_candidate(root, state)
    previous = _validate_previous(root, state)
    if _read_selection(root) != state["selected_generation_id"]:
        raise D03Error("simulated selection and rehearsal state disagree")
    return {
        "status": "INCOMPLETE",
        "evidence_scope": "synthetic_fixture",
        "simulation_only": True,
        "candidate_generation_id": state["candidate_generation_id"],
        "manifest_sha256": candidate["manifest_sha256"],
        "previous_manifest_sha256": previous["manifest_sha256"],
        "owner_count": "6",
        "real_owner_open_performed": False,
    }


def inspect_state(root_path: str | Path) -> dict[str, Any]:
    root = checked_root(root_path)
    state = _read_state(root)
    selected = _read_selection(root)
    if selected != state["selected_generation_id"]:
        raise D03Error("simulated selection and rehearsal state disagree")
    return {
        "status": "INCOMPLETE",
        "evidence_scope": "synthetic_fixture",
        "simulation_only": True,
        "phase": state["phase"],
        "selected_generation_id": selected,
        "candidate_generation_id": state["candidate_generation_id"],
        "previous_generation_id": state["previous_generation_id"],
        "manifest_sha256": state["manifest_sha256"],
        "writer_gate": state["writer_gate"],
        "writers_stopped": state["writers_stopped"],
        "pending_host_outcome": state["pending_host_outcome"],
        "write_boundary_crossed": state["write_boundary_crossed"],
        "accepted_new_writes": state["accepted_new_writes"],
        "activation_authorized": False,
        "health_check": state["health_check"],
    }