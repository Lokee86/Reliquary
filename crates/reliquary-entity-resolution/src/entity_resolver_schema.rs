use serde_json::{Value, json};

pub const ENTITY_RESOLVER_CONTRACT_VERSION: &str = "v9";
pub const ENTITY_ADMISSION_CONTRACT_VERSION: &str = "v6";
pub const ENTITY_MATERIALIZATION_CONTRACT_VERSION: &str = "v3";

pub const ENTITY_RESOLVER_SYSTEM_PROMPT: &str = r#"Resolve only the supplied Entity mention. Candidate generation is permissive: exact, alias, normalized-surface, lexical, or graph evidence NEVER proves identity.

First classify identity_relation between the mention and the selected candidate:
- same_identity: both expressions identify the SAME referent and are interchangeable identifiers for that referent.
- related_distinct: they are semantically related but identify DIFFERENT referents.
- uncertain: evidence does not safely establish either conclusion.

Being owned by, implemented by, stored in, declared in, pointing to, wrapping, controlling, representing, containing, or belonging to a candidate is related_distinct, NOT same_identity. In particular, file vs type, variable/reference vs runtime node/object, repository vs application/project, asset/file vs effect/component, UI vs application, model/read-model vs subsystem, method vs owning type, and directory vs contained component are distinct identities unless the Memory explicitly establishes that the two expressions are names for the same referent.

Also classify mention_kind using the Entity kind vocabulary. Entity-kind conflicts are strong negative evidence. A false merge is worse than leaving the mention unresolved. Conversely, do not split a clearly shared identity merely because wording, qualification, URL form, pluralization, command spelling, or surrounding role differs. Repository names/handles/URLs can denote the same repository; singular/plural format names can denote the same standard; qualified and common project names can denote the same project. For an exact-surface candidate with a compatible kind, choose related_distinct only when the Memory provides positive evidence that two different same-named referents exist.

Choose resolve_existing only when identity_relation=same_identity. If identity_relation=related_distinct and the mention itself is a continuing durable identity not represented by the candidates, choose create_new. If uncertain, choose unresolved. Reject when the mention is not a continuing durable Entity candidate.

A durable Entity must be a continuing identity that can reasonably recur across Memories and accumulate state, observations, or relationships over time. Immutable historical occurrences and one-off instances such as individual commits/revisions, builds, benchmark/test runs, snapshots, requests/responses, transactions, deployment instances, session instances, temporary task branches/worktrees, and similar execution/history records are not durable Entities merely because they have stable identifiers; reject them. Do not add, rewrite, merge, or alter the mention span."#;

pub const ENTITY_ADMISSION_SYSTEM_PROMPT: &str = r#"Validate first admission of one exact Entity mention when no existing Entity candidate was retrieved. Upstream extraction proposed the span, but it may still be generic, incomplete, over-broad, over-split, historical, transient, or too fine-grained to deserve immediate Entity promotion.

The Entity graph represents CONTINUING IDENTITIES, not every uniquely referable thing. A durable Entity should earn semantic graph ownership by being useful across Memories: it can accumulate changing state, observations, relationships, disambiguation value, or retrieval/routing value over time. Merely having a stable name, hash, ID, path, symbol, or unique reference is NOT sufficient.

Classify promotion_policy:
- immediate: intrinsically durable semantic identities that are useful graph anchors even on first mention. Typical examples are people, organizations, projects, products/applications, repositories, tools, services, major subsystems, named protocols/formats, major domain identities, and clearly architectural persistent components.
- requires_recurrence: fine-grained implementation artifacts whose usefulness as graph nodes should be demonstrated by compatible recurrence across distinct Memories. Typical examples are functions/methods, fields, constants, enum members, config keys, input actions, minor files/directories, individual UI controls/nodes/effects, variables, small helper components, and similar implementation details.
- none: use only when the mention is rejected or unresolved for a reason unrelated to recurrence.

A requires_recurrence referent MUST NOT be created from a single Memory. If there is no compatible context_evidence showing the SAME referent in another Memory, choose unresolved with reason=recurrence_required and return no Entity materialization. If compatible recurrence is present, create_new is allowed and the earlier pending mention can later resolve to the created Entity. Surface equality alone does not prove recurrence identity; reject incompatible or clearly different same-surface context.

Code symbols, enum values, and input actions are recurrence-gated implementation artifacts by default. Ordinary classes/structs/code types, files/directories, UI components, runtime nodes, and code components also normally require recurrence unless the current Memory clearly establishes that they are major architectural identities. Do not promote a minor artifact merely because it has a precise path, type name, or symbol name.

Reject immutable historical occurrences and one-off instances that are better kept in Memory/provenance than promoted into the durable Entity graph. This includes individual source-control commits or revisions, individual builds, benchmark/test runs, dated snapshots, one request/response, one transaction, one deployment instance, one session instance, temporary worktrees/branches created for a task, and similar execution/history records. A repository is a durable Entity; one commit of that repository normally is not. A continuing file can be an Entity when it is sufficiently important or recurrent; one immutable revision/snapshot of that file is not.

Reject implementation plans, task scopes, milestones, one-off upgrade phases, and similar work-state concepts that belong in Memory/Observation rather than the Entity graph. Reject grouped/plural wrapper mentions that denote multiple independently identifiable things (for example "info and write servers") rather than one identity; do not create one composite Entity merely because the phrase is contiguous. General categories, classes, and concepts are also wrapper categories rather than continuing identities; for example a category of AI systems is not itself one durable Entity merely because the category has a conventional name.

Reject context-relative descriptions that cannot be independently re-identified from future Memories, such as "the status label", "the Windows-side project", "the app entry", or comparable phrases whose identity depends on omitted surrounding context. If the durable identity requires an omitted modifier outside the supplied span, reject the supplied span rather than reconstructing it.

Do NOT classify a multiword span as generic_role merely because its head noun is generic (for example server, service, tool, component, or repository). An explicitly existing/planned owner-local component with a stable continuing responsibility may be admitted when it is an intrinsically useful semantic identity. Exact compound/symbol surfaces may identify durable referents, but fine-grained implementation identity still requires recurrence.

The Memory may explain what the exact span denotes, but NEVER borrow words outside the mention to repair or expand it. Reject bare generic category/activity/role nouns, abstract processes, transient values, sentence-local descriptors, historical occurrences, and incomplete fragments.

Context_evidence contains bounded owner-local Memories using the same surface when available. It is supporting evidence only. Determine whether those Memories clearly refer to the SAME identity before treating them as recurrence evidence. The surface does not need to be globally unique.

Choose unresolved with reason=ambiguous or insufficient_evidence when the current Memory itself does not identify which durable continuing referent is meant. In particular, a bare filename/basename such as main.go without a concrete namespace/path or other distinguishing identity is unresolved when the current Memory does not establish which file it denotes.

If and only if decision=create_new, choose the narrowest supported entity_kind and write one compact source-grounded entity_summary identifying this specific continuing referent. Use the current Memory as the identity anchor. You MAY use compatible context_evidence to infer stable kind or identity semantics. Exclude context that could describe a second/new/other/different instance; never merge distinct same-surface identities into the metadata.

The summary must describe the stable identity of the referent—what it is—not the transient task, change, operation, event, revision, run, or snapshot currently involving it. For unresolved or reject, return entity_kind=unknown and an empty entity_summary."#;

pub const ENTITY_MATERIALIZATION_SYSTEM_PROMPT: &str = "Materialize metadata for one newly accepted continuing durable Entity. The identity decision is already made; do not reject, merge, rename, add aliases, or change the mention. Choose the narrowest supported entity_kind and write one compact source-grounded entity_summary identifying this specific continuing referent using only the supplied Memory. Describe the stable identity of the referent—what it is—not the transient task, change, operation, event, revision, run, or snapshot currently involving it. Omit operation-specific detail unless it is necessary to distinguish this identity from another durable referent. Do not invent facts not present in the Memory.";

pub fn entity_resolver_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["decision", "reason", "target_candidate_index", "identity_relation", "mention_kind"],
        "properties": {
            "decision": {
                "type": "string",
                "enum": ["resolve_existing", "create_new", "unresolved", "reject"]
            },
            "reason": {"type": "string", "enum": resolution_reasons()},
            "target_candidate_index": {"type": "integer"},
            "identity_relation": {"type": "string", "enum": ["same_identity", "related_distinct", "uncertain"]},
            "mention_kind": {"type": "string", "enum": entity_kinds_with_unknown()}
        }
    })
}

pub fn entity_admission_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["decision", "reason", "promotion_policy", "entity_kind", "entity_summary"],
        "properties": {
            "decision": {
                "type": "string",
                "enum": ["create_new", "unresolved", "reject"]
            },
            "reason": {"type": "string", "enum": resolution_reasons()},
            "promotion_policy": {
                "type": "string",
                "enum": ["immediate", "requires_recurrence", "none"]
            },
            "entity_kind": {"type": "string", "enum": entity_kinds_with_unknown()},
            "entity_summary": {"type": "string", "maxLength": 1024}
        }
    })
}

pub fn entity_materialization_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["entity_kind", "entity_summary"],
        "properties": {
            "entity_kind": {"type": "string", "enum": entity_kinds()},
            "entity_summary": {"type": "string", "minLength": 1, "maxLength": 1024}
        }
    })
}

fn resolution_reasons() -> Vec<&'static str> {
    vec![
        "context_match",
        "context_conflict_new_identity",
        "named_referent",
        "persistent_artifact",
        "descriptive_identity",
        "first_seen_identity",
        "insufficient_evidence",
        "generic_role",
        "abstract_process",
        "transient_value",
        "sentence_local",
        "wrapper_category",
        "ambiguous",
        "recurrence_required",
    ]
}

fn entity_kinds() -> Vec<&'static str> {
    vec![
        "application",
        "code_component",
        "code_symbol",
        "code_type",
        "data_format",
        "directory",
        "domain_entity",
        "enum_value",
        "file",
        "input_action",
        "programming_language",
        "repository",
        "runtime_entity",
        "service",
        "subsystem",
        "tool",
        "tool_service",
        "ui_component",
        "other",
    ]
}

fn entity_kinds_with_unknown() -> Vec<&'static str> {
    let mut values = entity_kinds();
    values.push("unknown");
    values
}
