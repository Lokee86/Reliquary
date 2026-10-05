#!/usr/bin/env python3
"""Fail closed when a public gateway API or REL worker write lacks classification.
This source guard supplements Rust behavioral tests; it is not a Rust compiler.
"""
from pathlib import Path
import argparse
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
GROUPS = {
    "published": "set_rel_metadata_for create_knowledge_memory_for replace_knowledge_memory_for rename_knowledge_community_for mutate_knowledge_relation_for validate_project_environment_for checkpoint_manual_project_for ingest_project_attachment_for",
    "delegated": "set_rel_metadata create_knowledge_memory refresh_principal_projection_for",
    "ephemeral": "set_rel_processing_enabled_for",
    "host": "reconnect_host",
    "epoch": "quiesce_rel quiesce_rel_for reopen_rel reopen_rel_for reconcile_quiesced_rel_for shutdown unmount_rel mount_rel_path",
    "private": "set_phylactery_profile quiesce_phy quiesce_phy_for reopen_phy reopen_phy_for unmount_phy mount_phy_path",
    "convergence": "submit_conversation admit_conversation_writer cancel_conversation_submission process_next_conversation_submission set_conversation_policy resume_conversation_submission cancel_conversation_generation start_conversation_generation checkpoint_conversation_generation complete_conversation_generation",
    "attachment": "select_rel detach_instance select_managed_conversation attach_conversation_viewer detach_conversation_viewer attach_authenticated_instance refresh_authenticated_instance with_admission_adapter",
    "read": "conversation_policy conversation_generation_status new new_with_host state_dir capability_state capability_authority authorize active_rel_id instance_managed_state mounted_rel_ids mounted_phy_ids owner_admission conversation_summaries conversation_summaries_for conversation_transcript conversation_transcript_for search_conversation_branch search_conversation_branch_for echo_events echo_events_for read_echo_source_for files_for_source files_for_source_for project_file_ref project_file_ref_for file_bytes file_bytes_for search_archive search_archive_for read_reliquary_knowledge read_reliquary_knowledge_for knowledge_memory knowledge_memory_for read_memory_provenance read_memory_provenance_for resolve_external_memory_provenance_for rel_metadata rel_metadata_for dependency_closure dependency_closure_for retrieve_reliquary_memories_for background_status viewable_rel_owners conversation_transcript_page conversation_transcript_page_for search_dependency_archive_for search_dependency_archive conversation_summaries_page search_archive_page search_memories_by_text_for search_memories_by_text retrieve_reliquary_memories project_repository_root_for resolved_file_bytes_for visible_semantic_owners semantic_owner knowledge_memory_for_owner read_knowledge_for_owner entity_stats_for_owner entities_for_owner semantic_graph_relations_for_owner memory_routing_metadata_for_owner phylactery_profile search_visible_memories owner_mount_is_path_scoped"
}
CLASSIFICATION = {name: group for group, names in GROUPS.items() for name in names.split()}
WRITE = re.compile(r"\.cva\s*\.\s*(?:set_|publish_|commit_|mark_|reconcile_dream_|resolve_principal_|correlate_|register_)[a-z_]*\s*\(")

def masked(source):
    # Preserve offsets, handling nested comments, ordinary and raw strings.
    result = list(source)
    i = 0
    while i < len(source):
        start = i
        if source.startswith("//", i):
            end = source.find("\n", i)
            i = len(source) if end < 0 else end
        elif source.startswith("/*", i):
            i += 2
            depth = 1
            while i < len(source) and depth:
                if source.startswith("/*", i):
                    depth += 1
                    i += 2
                elif source.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
        else:
            raw = re.match(r'(?:br|r)(#*)"', source[i:])
            if raw:
                terminator = '"' + raw[1]
                end = source.find(terminator, i + len(raw[0]))
                i = len(source) if end < 0 else end + len(terminator)
            elif source[i] == '"':
                i += 1
                while i < len(source):
                    if source[i] == "\\":
                        i += 2
                    elif source[i] == '"':
                        i += 1
                        break
                    else:
                        i += 1
            else:
                i += 1
                continue
        result[start:i] = " " * (i - start)
    return "".join(result)

def closing(source, start):
    depth = 0
    for i in range(start, len(source)):
        if source[i] == "{":
            depth += 1
        elif source[i] == "}":
            depth -= 1
            if depth == 0:
                return i
    raise ValueError("unclosed Rust block")

def gateway_methods(source):
    source = masked(source)
    for block in re.finditer(r"impl\s+GatewayRuntime\s*\{", source):
        start = source.index("{", block.start())
        end = closing(source, start)
        for method in re.finditer(r"\bpub\s+(?:(?:async|const|unsafe)\s+)*fn\s+(\w+)\b", source[start:end]):
            pos = start + method.end()
            body = source.index("{", pos)
            yield method[1], source[body:closing(source, body) + 1]

def check_gateway(source):
    errors = []
    for name, body in gateway_methods(source):
        category = CLASSIFICATION.get(name)
        if category is None:
            errors.append(f"{name}: unclassified public GatewayRuntime API")
        elif category == "published" and "with_rel_mutation(" not in body:
            errors.append(f"{name}: missing canonical mutation admission")
        elif category == "ephemeral" and "with_rel_distribution_admission(" not in body:
            errors.append(f"{name}: missing ephemeral sequencing boundary")
        elif category == "host" and "publish_host_capability_change(" not in body:
            errors.append(f"{name}: missing host summary publication")
        elif category == "read" and WRITE.search(body):
            errors.append(f"{name}: authoritative write in read classification")
        elif category == "delegated":
            expected = {"set_rel_metadata": "set_rel_metadata_for(", "create_knowledge_memory": "create_knowledge_memory_for(", "refresh_principal_projection_for": "commit_principal_projection("}[name]
            if expected not in body:
                errors.append(f"{name}: missing classified delegate")
    return errors

def check_worker(source):
    source = masked(source)
    regions = []
    for call in re.finditer(r"\bshared\.mutate\s*\(", source):
        start = source.index("{", call.end())
        regions.append((start, closing(source, start)))
    errors = []
    for call in WRITE.finditer(source):
        if not any(start <= call.start() <= end for start, end in regions):
            errors.append(f"worker owner write outside canonical admission at offset {call.start()}")
    # Insomnia uses interaction helpers rather than direct CVA writes.
    helpers = r"\b(?:claim_runtime_insomnia|renew_runtime_insomnia|freeze_runtime_insomnia|commit_runtime_insomnia_verified|fail_runtime_insomnia|finalize_inactive_runtime_conversations|prepare_runtime_insomnia_application|accept_runtime_compatibility_profile|fail_insomnia_episode|finalize_inactive_session)\s*\("
    for call in re.finditer(helpers, source):
        if not any(start <= call.start() <= end for start, end in regions):
            errors.append(f"worker helper outside canonical admission: {call[0]}")
    return errors

def self_test():
    assert check_gateway("impl GatewayRuntime { pub fn new_mutator(&self) {} }")
    assert check_gateway("impl GatewayRuntime { pub async fn new_mutator(&self) {} }")
    assert check_gateway("impl GatewayRuntime { pub fn create_knowledge_memory_for(&self) { self.with_rel(); } }")
    assert check_gateway("impl GatewayRuntime { pub fn rel_metadata_for(&self) { runtime.cva.set_rel_metadata(); } }")
    assert not check_gateway('impl GatewayRuntime { pub fn rel_metadata_for(&self) { let text = "runtime.cva.set_metadata()"; } }')
    assert check_worker("fn commit() { runtime.cva.publish_memory(); }")
    assert not check_worker("fn commit() { shared.mutate(Family, |runtime| { runtime.cva.publish_memory(); }); }")
    print("P4.4 coverage guard: 7 negative/positive fixtures passed")

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    errors = []
    directory = ROOT / "src" / "runtime_inference"
    count = 0
    for path in sorted(directory.glob("runtime_host_gateway*.rs")):
        if "tests" in path.stem:
            continue
        source = path.read_text(encoding="utf-8")
        count += len(list(gateway_methods(source)))
        errors += [f"{path.name}: {e}" for e in check_gateway(source)]
    for family in ("dream", "perception", "vectors", "insomnia"):
        path = directory / f"runtime_host_rel_{family}.rs"
        errors += [f"{path.name}: {e}" for e in check_worker(path.read_text(encoding="utf-8"))]
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"P4.4 coverage guard: {count} public APIs classified; REL worker writes admitted")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
