#!/usr/bin/env python3
"""Source-backed Phase 0B G6 consumer-drift audit.

This does not build either Rust crate or prove cancellation semantics. It pins
the inspected Warlock API boundary so Phase 1 cannot silently inherit the old
single-active-REL/PHY/managed-session contract.
"""
from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parents[1]

# Each API must be migrated or explicitly retained at the new authority when
# the consumer hard cut occurs. Names are current source landmarks, not a new
# public compatibility facade.
WARLOCK_SEAMS: dict[str, dict[str, tuple[str, ...]]] = {
    "composition": {
        "workspace.rs": ("ReliquaryRuntimeHost", "pub(crate) runtime:", "reliquary_host("),
        "workspace_open.rs": ("InteractionRuntime::new", ".mount_rel("),
        "workspace_session.rs": ("restore_workspace_session",),
    },
    "conversation": {
        "conversation.rs": (
            "start_conversation_session(", "reopen_conversation_session(",
            "require_active_session(", "send_user_message_with_attachments(",
        ),
        "conversation_stream.rs": (
            "begin_assistant_stream(", "checkpoint_assistant_stream(",
            "complete_assistant_stream(",
        ),
    },
    "provider": {
        "chat_commands.rs": ("reserve(&workspace_id)", "send_user_message_with_attachments(", "ProviderCancellation"),
        "inference_lifecycle.rs": ("ProviderCancellation", "pub fn quiesce("),
        "chat_stream.rs": ("checkpoint_assistant_stream(", "ChatStreamEvent::TextDelta"),
        "reliquary_routes.rs": ("ReliquaryRuntimeRoutes",),
    },
}

PIN_RE = {
    "reliquary": r'reliquary-memory\s*=\s*\{[^}]*?rev\s*=\s*"([0-9a-f]{40})"',
    "arcana": r'(?m)^arcana\s*=\s*\{[^}]*?rev\s*=\s*"([0-9a-f]{40})"',
}


def pin(content: str, key: str) -> str:
    match = re.search(PIN_RE[key], content)
    if not match:
        raise ValueError(f"cannot establish {key} revision from Cargo manifest")
    return match.group(1)


def run(warlock: pathlib.Path) -> dict[str, object]:
    manifest = warlock / "src-tauri" / "Cargo.toml"
    if not manifest.is_file():
        raise FileNotFoundError(f"Warlock manifest missing: {manifest}")
    warlock_manifest = manifest.read_text(encoding="utf-8")
    reliquary_rev = pin(warlock_manifest, "reliquary")
    direct_arcana = pin(warlock_manifest, "arcana")
    historical = subprocess.run(
        ["git", "-C", str(HERE), "show", f"{reliquary_rev}:Cargo.toml"],
        capture_output=True, text=True, check=True, timeout=15
    ).stdout
    transitive_arcana = pin(historical, "arcana")
    if direct_arcana != transitive_arcana:
        raise AssertionError(
            f"Warlock dependency conflict: direct Arcana {direct_arcana} "
            f"!= Reliquary-pinned Arcana {transitive_arcana}"
        )
    verified: dict[str, list[str]] = {}
    for area, files in WARLOCK_SEAMS.items():
        verified[area] = []
        for filename, tokens in files.items():
            path = warlock / "src-tauri" / "src" / filename
            body = path.read_text(encoding="utf-8")
            absent = [token for token in tokens if token not in body]
            if absent:
                raise AssertionError(
                    f"{path}: missing inspected integration landmarks {absent}; "
                    "review migration impact before updating this manifest"
                )
            verified[area].append(filename)
    return {
        "consumer": "Warlock-v2",
        "status": "source inventory verified; no build or runtime-conformance claim",
        "reliquary_revision": reliquary_rev,
        "direct_arcana_revision": direct_arcana,
        "transitive_arcana_revision": transitive_arcana,
        "source_groups": verified,
        "current_stage_authority": {
            "pending_submission_journal": "Reliquary future native queue",
            "provider_execution_and_active_cancel": "Warlock",
            "conversation_commit_and_stream_checkpoint": "Reliquary",
            "gateway_principal_grants_and_subscription": "Reliquary future gateway",
            "external_durable_pending_host_queue": "not established in inspected Warlock",
        },
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--warlock",
        type=pathlib.Path,
        default=HERE.parent / "Warlock-v2",
        help="The sibling Warlock-v2 checkout; required for a source-backed audit",
    )
    args = ap.parse_args()
    try:
        result = run(args.warlock.resolve())
    except (OSError, ValueError, AssertionError, subprocess.SubprocessError) as exc:
        print(json.dumps({"g6_consumer_audit": "FAILED", "reason": str(exc)}, indent=2))
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
