from __future__ import annotations

import os
from pathlib import Path
from typing import Any

from .files import (
    ROOT_MARKER,
    ROOT_MARKER_BYTES,
    D03Error,
    checked_root,
    durable_replace,
    is_reparse_point,
    make_directory,
    sync_directory,
    write_new,
)
from .manifest import CANDIDATE, PREVIOUS, create_generation
from .state import (
    STATE_FILE,
    _generation_path,
    _read_selection,
    _read_state,
    _write_selection,
    _write_state,
)
from .validation import validate_generation

def _validate_candidate(root: Path, state: dict[str, Any]) -> dict[str, Any]:
    return validate_generation(
        _generation_path(root, state["candidate_generation_id"]), state["manifest_sha256"]
    )


def _validate_previous(root: Path, state: dict[str, Any]) -> dict[str, Any]:
    return validate_generation(
        _generation_path(root, state["previous_generation_id"]), state["previous_manifest_sha256"]
    )


def _sync_tree(path: Path) -> None:
    try:
        items = list(path.rglob("*"))
        for item in items:
            if is_reparse_point(item):
                raise D03Error("reparse path appeared while syncing synthetic generation")
            if item.is_file():
                with item.open("r+b") as stream:
                    os.fsync(stream.fileno())
        for directory in sorted(
            [path, *(item for item in items if item.is_dir())],
            key=lambda item: len(item.parts),
            reverse=True,
        ):
            sync_directory(directory)
    except OSError as error:
        raise D03Error(f"synthetic generation durability sync failed: {error}") from error


def stage(root_path: str | Path, failpoint: str | None = None) -> dict[str, Any]:
    """Create a synthetic-only candidate generation; no caller-provided files are accepted."""
    if failpoint not in {None, "before_sync", "after_sync"}:
        raise D03Error("unsupported stage failpoint")
    root = checked_root(root_path, create=True)
    make_directory(root)
    write_new(root / ROOT_MARKER, ROOT_MARKER_BYTES)
    generations = root / "generations"
    make_directory(generations)
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
        "pending_transition": None,
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
    durable_replace(temporary, final)
    state["manifest_sha256"] = candidate_digest
    state["phase"] = "StagedReadOnly"
    _write_state(root, state)
    return inspect_state(root)


def validate(root_path: str | Path) -> dict[str, Any]:
    root = checked_root(root_path)
    from .recovery import reconcile

    state = reconcile(root)
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
    from .recovery import reconcile

    state = reconcile(root)
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
