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
OFFLINE_FACADE_POLICY = {
    "src/facade/cva_lifecycle.rs", "src/facade/cva_repack.rs",
    "src/facade/cva_principal_repack.rs", "src/facade/historical_import_transform.rs",
    "src/facade/cva_reconcile_promote.rs",
}
OWNER_SERVICE_FACADE_POLICY = {
    "src/facade/cva_memory_retrieval.rs", "src/facade/phylactery_memory_retrieval.rs",
    "src/facade/phylactery_memory_source.rs", "src/facade/external_source_extensions.rs",
    "src/facade/dream_product_extensions.rs", "src/facade/dream_pair_admission.rs",
    "src/facade/phylactery_dream_candidates.rs", "src/facade/cva_dream_population.rs",
}


def violations(path, source, facts):
    view = masked(strip_inactive_cfg(source, facts))
    errors = []
    if path in OWNER_SERVICE_FACADE_POLICY:
        if re.search(r"\bself\s*\.\s*(?:owner|memories|graph|communities|container|dream_pairs|processing_epochs|memory_vectors|packed_vectors)\b|\bcrate\s*::\s*(?:memory_store|memory_vector_store|dream_owner_candidates|dream_cooldown)\b", view):
            errors.append(f"{path}: public owner/service binding bypasses native runtime operation")
    if path in OFFLINE_FACADE_POLICY:
        if re.search(r"\bContainer\s*::|\.\s*container\s*\.|\bcrate\s*::\s*(?:archive_codec|turn_ingest_codec)\b", view):
            errors.append(f"{path}: public offline binding performs physical or codec implementation work")
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
    paths = sorted(READ_POLICY | PROVIDER_POLICY | OFFLINE_FACADE_POLICY | OWNER_SERVICE_FACADE_POLICY | {
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
