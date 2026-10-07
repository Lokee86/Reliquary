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
    "published": "set_rel_metadata_for create_knowledge_memory_for replace_knowledge_memory_for rename_knowledge_community_for mutate_knowledge_relation_for validate_project_environment_for checkpoint_manual_project_for ingest_project_attachment_for set_conversation_title_for put_conversation_compaction_for put_echo_event_for",
    "owner_semantic_mutation": "create_knowledge_memory_for_owner replace_knowledge_memory_for_owner mutate_knowledge_relation_for_owner rename_knowledge_community_for_owner",
    "delegated": "set_rel_metadata create_knowledge_memory refresh_principal_projection_for",
    "ephemeral": "set_rel_processing_enabled_for",
    "host": "reconnect_host",
    "epoch": "quiesce_rel quiesce_rel_for reopen_rel reopen_rel_for reconcile_quiesced_rel_for shutdown unmount_rel mount_rel_path reconcile_rel_file",
    "trusted_project": "create_project_environment_rel inspect_project_adoption_source adopt_project_environment_rel validate_project_environment_cva",
    "background": "set_routes wake_rel_background_for set_rel_background_enabled_for wake_phy_background_for set_phy_background_enabled_for",
    "private": "set_phylactery_profile quiesce_phy quiesce_phy_for reopen_phy reopen_phy_for unmount_phy mount_phy_path",
    "conversation": "submit_conversation admit_conversation_writer cancel_conversation_submission process_next_conversation_submission set_conversation_policy resume_conversation_submission cancel_conversation_generation start_conversation_generation checkpoint_conversation_generation complete_conversation_generation",
    "attachment": "select_rel detach_instance select_managed_conversation attach_conversation_viewer detach_conversation_viewer attach_authenticated_instance refresh_authenticated_instance with_admission_adapter",
    "read": "conversation_policy conversation_generation_status new new_with_host state_dir capability_state capability_authority authorize active_rel_id instance_managed_state mounted_rel_ids mounted_phy_ids owner_admission conversation_summaries conversation_summaries_for conversation_transcript conversation_transcript_for search_conversation_branch search_conversation_branch_for echo_events echo_events_for read_echo_source_for files_for_source files_for_source_for project_file_ref project_file_ref_for file_bytes file_bytes_for search_archive search_archive_for read_reliquary_knowledge read_reliquary_knowledge_for knowledge_memory knowledge_memory_for read_memory_provenance read_memory_provenance_for resolve_external_memory_provenance_for rel_metadata rel_metadata_for dependency_closure dependency_closure_for retrieve_reliquary_memories_for background_status viewable_rel_owners conversation_transcript_page conversation_transcript_page_for search_dependency_archive_for search_dependency_archive conversation_summaries_page search_archive_page search_memories_by_text_for search_memories_by_text retrieve_reliquary_memories project_repository_root_for resolved_file_bytes_for visible_semantic_owners semantic_owner knowledge_memory_for_owner read_knowledge_for_owner entity_stats_for_owner entities_for_owner semantic_graph_relations_for_owner memory_routing_metadata_for_owner phylactery_profile search_visible_memories search_visible_archive search_archive_owner conversation_revision_for conversation_branch_start_node_ids_for conversation_compactions_for read_memory_provenance_for_owner resolve_knowledge_provenance_for_owner owner_mount_is_path_scoped background_status_for phy_background_status_for background_worker_state_for"
}
CLASSIFICATION = {name: group for group, names in GROUPS.items() for name in names.split()}
# Conservatively recognize state-changing Cva verbs in production gateway/worker
# modules. Reads are deliberately excluded; new mutating prefixes must be added
# here and covered by a fixture rather than silently bypassing the guard.
WRITE = re.compile(
    r"\.cva\s*\.\s*(?:set_|publish_|commit_|mark_|reconcile_|resolve_|"
    r"correlate_|register_|create_|replace_|rename_|mutate_|validate_|"
    r"checkpoint_|ingest_|accept_|claim_|renew_|freeze_|fail_|finalize_|"
    r"complete_|remove_|upsert_|retire_|append_)[a-z_]*\s*\("
)

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
        route_body = re.sub(r"\s+", "", body)
        category = CLASSIFICATION.get(name)
        if category is None:
            errors.append(f"{name}: unclassified public GatewayRuntime API")
        elif category == "published" and "with_rel_mutation(" not in body:
            errors.append(f"{name}: missing canonical mutation admission")
        elif category == "trusted_project":
            if name == "create_project_environment_rel":
                if "self.lifecycle.lock(" not in route_body or "rel_path.exists()" not in body or "create_project_rel(" not in body:
                    errors.append(f"{name}: missing atomic new-file path admission")
                if "admit_unmounted_path(" in body:
                    errors.append(f"{name}: cannot inspect header before Project REL creation")
            elif name != "validate_project_environment_cva" and "admit_unmounted_path(" not in body:
                errors.append(f"{name}: missing unmounted trusted path admission")
        elif category == "owner_semantic_mutation":
            if "semantic_write_token(" not in body or "OwnerKind::Rel" not in body or "OwnerKind::Phy" not in body:
                errors.append(f"{name}: missing exact-owner REL/PHY write discrimination")
            elif name == "rename_knowledge_community_for_owner":
                if "rename_knowledge_community_for(" not in body or "with_phy(" not in body:
                    errors.append(f"{name}: missing canonical semantic/PHY rename")
            elif "with_rel_mutation(" not in body or "with_phy(" not in body:
                errors.append(f"{name}: missing canonical committed REL / private PHY writes")
        elif category == "ephemeral" and "with_rel_distribution_admission(" not in body:
            errors.append(f"{name}: missing ephemeral sequencing boundary")
        elif category == "conversation" and not any(route in body for route in ("with_conversation_admission(", "with_accepted_work(", "with_operation(", "self.process_next_accepted_conversation_submission(", ".process_reserved_conversation_submission(")):
            errors.append(f"{name}: missing sequenced P3 persistence route")
        elif category == "host" and "publish_host_capability_change(" not in body:
            errors.append(f"{name}: missing host summary publication")
        elif category == "epoch":
            route = {
                "quiesce_rel": ("sequencer.close(",),
                "quiesce_rel_for": ("sequencer.close(",),
                "reopen_rel": ("allocate_mount_epoch(", "reopen_rel_distribution("),
                "reopen_rel_for": ("allocate_mount_epoch(", "reopen_rel_distribution("),
                "reconcile_quiesced_rel_for": (
                    "fence.snapshot_full(",
                    "OwnerAdmission::Closed",
                    "workers_stopped()",
                    "reconcile_rel(",
                ),
                "shutdown": ("sequencer.close(",),
                "unmount_rel": ("sequencer.close(",),
                "mount_rel_path": ("mount_rel_inner(",),
                "reconcile_rel_file": ("admit_unmounted_path(", "reconcile_rel_file_unmounted("),
            }.get(name, ())
            if not route or not all(step in route_body for step in route):
                errors.append(f"{name}: missing classified REL epoch/resync route")
        elif category == "background":
            # Owner-local controls change no durable CVA or REL feed state.
            routes = {
                "set_routes": ("self.lifecycle.lock(", "self.routes.write(", "self.shutting_down.load("),
                "wake_rel_background_for": ("self.authorize(", "OwnerGrant::ManageRel", "self.with_rel(", "notify_work("),
                "set_rel_background_enabled_for": ("self.authorize(", "OwnerGrant::ManageRel", "self.with_rel(", "background_enabled.store(", "notify_work("),
                "wake_phy_background_for": ("self.authorize(", "OwnerGrant::UseOwnPhy", "self.with_phy(", "shared.wake("),
                "set_phy_background_enabled_for": ("self.authorize(", "OwnerGrant::UseOwnPhy", "self.with_phy(", "background_enabled.store(", "shared.wake("),
            }[name]
            if not all(step in route_body for step in routes):
                errors.append(f"{name}: missing exact native background control route")
            if WRITE.search(body):
                errors.append(f"{name}: authoritative write in background control classification")
        elif category == "private":
            route = {
                "set_phylactery_profile": ("with_phy(",),
                "quiesce_phy": ("quiesce_phy_inner(",),
                "quiesce_phy_for": ("quiesce_phy_inner(",),
                "reopen_phy": ("reopen_phy_inner(",),
                "reopen_phy_for": ("reopen_phy_inner(",),
                "unmount_phy": ("fence.close_and_drain(",),
                "mount_phy_path": ("mount_phy_inner(",),
            }.get(name, ())
            if not route or not all(step in route_body for step in route):
                errors.append(f"{name}: missing private-owner maintenance route")
        elif category == "attachment":
            route = {
                "select_rel": ("attachment_entry(", "instance.select_rel("),
                "detach_instance": ("remove_instance(", "detach_instance_viewers("),
                "select_managed_conversation": ("select_rel(", "set_instance_managed_state("),
                "attach_conversation_viewer": ("viewers", "insert("),
                "detach_conversation_viewer": ("viewers", "remove("),
                "attach_authenticated_instance": ("adapter.verify(", "attach_instance("),
                "refresh_authenticated_instance": ("adapter.verify(", "replace_instance_grants("),
                "with_admission_adapter": ("admission_adapter",),
            }.get(name, ())
            if not route or not all(step in route_body for step in route):
                errors.append(f"{name}: missing classified attachment/authority route")
        elif category == "read" and WRITE.search(body):
            errors.append(f"{name}: authoritative write in read classification")
        elif category == "delegated":
            expected = {"set_rel_metadata": "set_rel_metadata_for(", "create_knowledge_memory": "create_knowledge_memory_for(", "refresh_principal_projection_for": "commit_principal_projection("}[name]
            if expected not in body:
                errors.append(f"{name}: missing classified delegate")
    return errors

def check_conversation_stage_helpers(source):
    source = masked(source)
    errors = []
    for name in ("with_conversation_admission", "with_accepted_work"):
        match = re.search(rf"\bfn\s+{name}\b", source)
        if match is None:
            errors.append(f"{name}: canonical P3 stage helper is missing")
            continue
        start = source.index("{", match.end())
        body = source[start:closing(source, start) + 1]
        if not re.search(r"\bgateway\s*\.\s*with_conversation_distribution\s*\(", body):
            errors.append(f"{name}: missing mounted publication gate")
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

def checkpoint_accessor(source):
    source = masked(source)
    match = re.search(r"\bfn\s+checkpoint_from_journal\b", source)
    if match is None:
        return None
    start = source.index("{", match.end())
    return source[start:closing(source, start) + 1]

def check_checkpoint_accessor(source):
    body = checkpoint_accessor(source)
    if body is None:
        return ["checkpoint_from_journal: canonical durable-prefix reader is missing"]
    errors = []
    for name in ("interaction_streams_for_session", "generation_stream_for_conversation"):
        if name in body:
            errors.append(f"checkpoint_from_journal: forbidden unbounded accessor {name}")
    read = body.find("interaction_stream_record(")
    length = body.find("let byte_len =")
    slice_at = body.find("get(..byte_len)")
    utf8 = body.find("std::str::from_utf8(prefix)")
    digest = body.find("Sha256::digest(prefix)")
    byte_cap = re.search(r"byte_len\s*>\s*MAX_DURABLE_STREAM_PREFIX_BYTES", body)
    available_len = re.search(r"byte_len\s*>\s*record\s*\.\s*content\s*\.\s*len\s*\(\s*\)", body)
    if min(read, length, slice_at, utf8, digest) < 0 or not (read < length < slice_at < utf8 < digest):
        errors.append("checkpoint_from_journal: must borrow one record and hash the bounded UTF-8 prefix before exposing text")
    if byte_cap is None or available_len is None or max(byte_cap.start(), available_len.start()) > slice_at:
        errors.append("checkpoint_from_journal: enforce durable prefix cap and available content length before slicing")
    if re.search(r"\brecord\s*\.\s*clone\s*\(", body[:slice_at]) or re.search(
        r"interaction_stream_record\s*\([^)]*\)\s*\.\s*clone\s*\(", body[:slice_at]
    ):
        errors.append("checkpoint_from_journal: must not clone the full stream record before prefix slicing")
    if re.search(r"record\s*\.\s*content\s*\.\s*(?:clone|to_owned)\s*\(", body[:slice_at]):
        errors.append("checkpoint_from_journal: must not clone full stream content before prefix slicing")
    if re.search(r"\.\s*cloned\s*\(", body[:slice_at]):
        errors.append("checkpoint_from_journal: must not use cloned() for the stream record before prefix slicing")
    return errors

def self_test():
    fixtures = 0
    def expect(value):
        nonlocal fixtures
        fixtures += 1
        assert value

    expect(check_gateway("impl GatewayRuntime { pub fn new_mutator(&self) {} }"))
    expect(check_gateway("impl GatewayRuntime { pub async fn new_mutator(&self) {} }"))
    expect(check_gateway("impl GatewayRuntime { pub fn create_knowledge_memory_for(&self) { self.with_rel(); } }"))
    expect(check_gateway("impl GatewayRuntime { pub fn create_knowledge_memory_for_owner(&self) { self.semantic_write_token(); self.with_rel_mutation(); } }"))
    expect(not check_gateway("impl GatewayRuntime { pub fn create_knowledge_memory_for_owner(&self) { self.semantic_write_token(); match token.kind { OwnerKind::Rel => self.with_rel_mutation(), OwnerKind::Phy => self.with_phy() } } }"))
    expect(check_gateway("impl GatewayRuntime { pub fn rel_metadata_for(&self) { runtime.cva.set_rel_metadata(); } }"))
    expect(not check_gateway('impl GatewayRuntime { pub fn rel_metadata_for(&self) { let text = "runtime.cva.set_metadata()"; } }'))
    expect(check_gateway("impl GatewayRuntime { pub fn reopen_rel(&self) {} }"))
    expect(not check_gateway("impl GatewayRuntime { pub fn reopen_rel(&self) { self.allocate_mount_epoch(); self.reopen_rel_distribution(); } }"))
    expect(check_gateway("impl GatewayRuntime { pub fn create_project_environment_rel(&self) { self.admit_unmounted_path(rel_path); } }"))
    expect(not check_gateway("impl GatewayRuntime { pub fn create_project_environment_rel(&self) { self.lifecycle.lock(); if rel_path.exists() { return Err(()); } create_project_rel(rel_path); } }"))
    expect(check_gateway("impl GatewayRuntime { pub fn create_project_environment_rel(&self) { self.lifecycle.lock(); if rel_path.exists() { return Err(()); } self.admit_unmounted_path(rel_path); create_project_rel(rel_path); } }"))
    expect(check_gateway("impl GatewayRuntime { pub fn set_phylactery_profile(&self) {} }"))
    expect(not check_gateway("impl GatewayRuntime { pub fn set_phylactery_profile(&self) { self.with_phy(|_| {}); } }"))
    expect(check_worker("fn commit() { runtime.cva.publish_memory(); }"))
    expect(check_worker("fn commit() { runtime.cva.create_knowledge_memory(); }"))
    expect(check_worker("fn sweep() { runtime.finalize_inactive_session(); }"))
    expect(not check_worker("fn commit() { shared.mutate(Family, |runtime| { runtime.cva.publish_memory(); }); }"))
    expect(not check_worker("fn commit() { shared.mutate(Family, |runtime| { runtime.cva.create_knowledge_memory(); }); }"))
    expect(not check_worker("fn sweep() { shared.mutate(Family, |runtime| { runtime.finalize_inactive_session(); }); }"))
    expect(check_gateway("impl GatewayRuntime { pub fn submit_conversation(&self) { with_admission(); } }"))
    expect(check_conversation_stage_helpers("fn with_conversation_admission<T>() { f(); } fn with_accepted_work<T>() { f(); }"))
    expect(check_gateway("impl GatewayRuntime { pub fn reconcile_quiesced_rel_for(&self) { self.reconcile_rel(owner_id); } }"))
    expect(not check_gateway("impl GatewayRuntime { pub fn reconcile_quiesced_rel_for(&self) { self.fence.snapshot_full(); OwnerAdmission::Closed; self.workers_stopped(); self.reconcile_rel(owner_id); } }"))
    for name in GROUPS["background"].split():
        expect(check_gateway(f"impl GatewayRuntime {{ pub fn {name}(&self) {{}} }}"))
    background = "impl GatewayRuntime { pub fn set_rel_background_enabled_for(&self) { self.authorize(OwnerGrant::ManageRel); self.with_rel(); shared.background_enabled.store(); notify_work(); } }"
    expect(not check_gateway(background))
    expect(check_gateway(background.replace("OwnerGrant::ManageRel", "OwnerGrant::ViewRel")))
    expect(check_gateway(background.replace("notify_work();", "runtime.cva.publish_memory(); notify_work();")))
    expect(check_gateway("impl GatewayRuntime { pub fn phy_background_status_for(&self) { runtime.cva.commit_memory(); } }"))
    checkpoint = """fn checkpoint_from_journal() {
        let record = runtime.cva().interaction_stream_record(&generation.generation_id);
        let byte_len = usize::try_from(checkpoint.utf8_bytes);
        if byte_len > MAX_DURABLE_STREAM_PREFIX_BYTES || byte_len > record.content.len() { return Err(()); }
        let prefix = record.content.as_bytes().get(..byte_len);
        let text = std::str::from_utf8(prefix).to_owned();
        let digest = Sha256::digest(prefix);
    }"""
    expect(not check_checkpoint_accessor(checkpoint))
    expect(check_checkpoint_accessor("fn checkpoint_from_journal() { let records = interaction_streams_for_session(); }"))
    expect(check_checkpoint_accessor("fn checkpoint_from_journal() { let record = generation_stream_for_conversation(); }"))

    def expect_checkpoint_error(mutated, expected_error):
        errors = check_checkpoint_accessor(mutated)
        expect(any(expected_error in error for error in errors))

    bounded_check = "if byte_len > MAX_DURABLE_STREAM_PREFIX_BYTES || byte_len > record.content.len()"
    expect_checkpoint_error(
        checkpoint.replace(bounded_check, "if byte_len > record.content.len()"),
        "enforce durable prefix cap and available content length",
    )
    expect_checkpoint_error(
        checkpoint.replace(bounded_check, "if byte_len > MAX_DURABLE_STREAM_PREFIX_BYTES"),
        "enforce durable prefix cap and available content length",
    )
    expect_checkpoint_error(
        checkpoint.replace(
            "interaction_stream_record(&generation.generation_id);",
            "interaction_stream_record(&generation.generation_id).clone();",
        ),
        "must not clone the full stream record before prefix slicing",
    )
    expect_checkpoint_error(
        checkpoint.replace(
            "interaction_stream_record(&generation.generation_id);",
            "interaction_stream_record(&generation.generation_id).cloned();",
        ),
        "must not use cloned() for the stream record before prefix slicing",
    )
    expect_checkpoint_error(
        checkpoint.replace(
            "let prefix = record.content.as_bytes().get(..byte_len);",
            "let content = record.content.clone();\n        let prefix = content.as_bytes().get(..byte_len);",
        ),
        "must not clone full stream content before prefix slicing",
    )
    expect_checkpoint_error(
        checkpoint.replace(
            "let prefix = record.content.as_bytes().get(..byte_len);",
            "let content = record.content.to_owned();\n        let prefix = content.as_bytes().get(..byte_len);",
        ),
        "must not clone full stream content before prefix slicing",
    )
    print(f"P4 coverage guard: {fixtures} negative/positive fixtures passed")

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
    conversation_path = directory / "runtime_host_gateway_conversation.rs"
    errors += [f"{conversation_path.name}: {e}" for e in check_conversation_stage_helpers(conversation_path.read_text(encoding="utf-8"))]
    checkpoint_path = directory / "runtime_host_gateway_conversation_observation.rs"
    errors += [f"{checkpoint_path.name}: {e}" for e in check_checkpoint_accessor(checkpoint_path.read_text(encoding="utf-8"))]
    # Scan every production REL worker module, not a hand-maintained subset. This
    # makes newly added worker files part of the canonical-admission check by default.
    for path in sorted(directory.glob("runtime_host_rel_*.rs")):
        if "tests" in path.stem:
            continue
        errors += [f"{path.name}: {e}" for e in check_worker(path.read_text(encoding="utf-8"))]
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"P4.8 coverage guard: {count} public APIs classified; REL worker writes admitted")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())