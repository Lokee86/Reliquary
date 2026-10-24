#!/usr/bin/env python3
"""Fast permanent guards for the exact Phase 7 capability cuts."""
import argparse
import sys
from pathlib import Path
import re

from check_owner_boundaries import strip_inactive_cfg
from check_multiplexing_phase4_coverage import masked

ROOT = Path(__file__).resolve().parents[1]
READ_POLICY = {
    "src/runtime_inference/dream_canonical.rs",
    "src/runtime_inference/dream_owner_candidates.rs",
    "src/runtime_inference/dream_owner_duplicate.rs",
    "src/runtime_inference/dream_owner_publisher.rs",
    "src/runtime_inference/dream_owner_lifecycle.rs",
    "src/runtime_inference/insomnia/evidence.rs",
    "src/retrieval/memory_retrieval_build.rs",
}
PROVIDER_POLICY = {"src/external_source/index.rs", "src/external_source/processing.rs"}


def violations(path, source, facts):
    view = masked(strip_inactive_cfg(source, facts))
    errors = []
    if path in READ_POLICY:
        if re.search(r"\b(?:Container|RelExecutionOwner|PhyExecutionOwner|Cva|Phylactery|GatewayRuntime|InstanceRuntime)\b", view):
            errors.append(f"{path}: lower policy receives physical or product composition capability")
    if path == "src/runtime_inference/dream_canonical.rs" and re.search(r"\btrait\s+DreamOwnerRead\b", view):
        errors.append(f"{path}: Dream policy restores an upper owner callback port")
    if path in PROVIDER_POLICY:
        if re.search(r"\b(?:EmbeddingEndpoint|GeneralEndpoint|TemporalInferencer|InsomniaExtractor|EmbeddingMode)\b|\bcomplete_json\s*\(", view):
            errors.append(f"{path}: External Sources invokes provider orchestration")
    if path == "src/imports/reconciliation_interval.rs":
        if re.search(r"\.insomnia\s*\.\s*apply_(?:work|attempt)\s*\(", view):
            errors.append(f"{path}: Import replay mutates InsomniaStore directly")
    if path == "src/owner_lifecycle/insomnia_import_replay.rs":
        if re.search(r"\bfn\s+decode_record\s*\(", view):
            errors.append(f"{path}: obsolete Insomnia replay decode forwarder remains")
    if path.startswith("src/owner_lifecycle/") and path.endswith("reliquary.rs"):
        if re.search(r"\b(?:LexicalIndex|DuplicateIndex|GatewayRuntime|RelExecutionOwner)\b", view):
            errors.append(f"{path}: durable owner contains runtime/service state")
    if path == "src/owner_lifecycle/phylactery.rs":
        if re.search(r"\b(?:LexicalIndex|DuplicateIndex|GatewayRuntime|PhyExecutionOwner)\b", view):
            errors.append(f"{path}: durable owner contains runtime/service state")
    return errors


def main():
    import json
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--files", nargs="*", help="changed-source paths; default is all guarded seams")
    args = parser.parse_args()
    mapping = json.loads((ROOT / "tools/pitlord/production-components.json").read_text())
    facts = set(mapping["cfg_profile"]["rustc_cfg_facts"])
    paths = sorted(READ_POLICY | PROVIDER_POLICY | {
        "src/owner_lifecycle/reliquary.rs",
        "src/owner_lifecycle/phylactery.rs",
        "src/imports/reconciliation_interval.rs",
        "src/owner_lifecycle/insomnia_import_replay.rs",
    })
    if args.files is not None:
        paths = sorted(set(paths) & {p.replace("\\", "/") for p in args.files})
    errors = []
    for path in paths:
        if not (ROOT / path).is_file():
            errors.append(f"{path}: guarded production seam missing")
        else:
            errors.extend(violations(path, (ROOT / path).read_text(encoding="utf-8"), facts))
    print("\n".join(errors) if errors else f"Phase 7 source cuts: {len(paths)} guarded seams passed")
    return int(bool(errors))


if __name__ == "__main__":
    sys.exit(main())
