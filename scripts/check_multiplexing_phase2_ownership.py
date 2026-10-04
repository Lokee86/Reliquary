"""Inventory legacy personal ownership; enforce the canonical Phase 2 path."""
import argparse
import json
import re
from pathlib import Path

NEW_PATH = ("runtime_host_gateway", "runtime_host_phy_", "runtime_host_rel_")
RULES = {
    "mutable_phy_slot": r"phylactery\s*:\s*Arc<Mutex<Option<Phylactery>>>",
    "phy_enabled": r"\bphylactery_enabled\b",
    "mixed_user_profile": r"\b(?:profiles|memory_profiles|next_profiles)\.user\b",
    "rel_user_queue": r"\bPerceptionOwner::User\b",
    "ambient_execution": r"\bactive_execution\s*\(",
    "ambient_rel": r"\bactive_rel_id\s*\(\s*\)",
    "ambient_phy": r"\bphylactery_owner_id\s*\(",
    "legacy_authority": r"\b(?:ReliquaryRuntimeHost|OwnerExecution|RuntimeMemoryProfiles|PerceptionOwner)\b",
}
BASELINE = {
    "runtime_host_owner_execution.rs", "runtime_host_vectors.rs",
    "runtime_host_dream.rs", "runtime_host_dream_owner.rs",
    "runtime_host_perception.rs", "runtime_host_perception_owner.rs",
    "runtime_host_perception_flow.rs", "runtime_host_perception_reconciliation.rs",
    "runtime_host_insomnia.rs", "runtime_host_semantic_access.rs",
    "runtime_host_memory_search.rs", "runtime_host_phylactery.rs",
}


def inventory(root):
    rows = []
    for path in sorted((root / "src/runtime_inference").glob("*.rs")):
        if "tests" in path.name or "test_support" in path.name:
            continue
        if path.name not in BASELINE and not path.name.startswith(NEW_PATH):
            continue
        for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for rule, pattern in RULES.items():
                if re.search(pattern, line):
                    rows.append({"file": path.relative_to(root).as_posix(),
                                 "line": line_number, "rule": rule})
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--enforce", action="store_true",
                        help="Reject legacy personal ownership in gateway, REL and PHY production modules.")
    args = parser.parse_args()
    rows = inventory(args.repo.resolve())
    violations = [row for row in rows if Path(row["file"]).name.startswith(NEW_PATH)]
    print(json.dumps({"mode": "enforce" if args.enforce else "inventory",
                      "references": rows, "gateway_violations": violations}, indent=2))
    return 1 if args.enforce and violations else 0


if __name__ == "__main__":
    raise SystemExit(main())
