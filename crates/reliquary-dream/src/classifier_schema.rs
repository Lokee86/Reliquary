use serde_json::{Value, json};

pub const DREAM_CLASSIFIER_SYSTEM_PROMPT: &str = r#"You classify the semantic relationship between exactly two durable Memories for a persistent Memory graph.

The pair is presented in canonical MemoryId order as A and B. A/B do NOT mean source/candidate, old/new, cause/effect, or processing order. Determine direction only from semantic evidence in the Memories.

Primary decision rule: compare the Memories' most specific semantic workstream or scope, not just their literal entities or shared words. A workstream is a specific protocol, workflow, problem, implementation concern, operational context, user preference, design decision, or concept being reasoned about.

A relationship may exist even when the Memories describe different entities or artifacts if those entities participate in the same specific workstream. For example, a protocol migration constraint can relate to the current message schema; two unresolved states can relate when they are parts of the same user flow; a general interaction preference can relate to a scoped application of that preference; and a current-status summary can relate to a concrete capability that the summary encompasses.

Do NOT create a relationship merely because both Memories belong to the same project, domain, codebase, conversation, or time period. Project identity/history does not relate to unrelated implementation details. Different subsystems are not related merely because they share terms such as state, collision, packet, player, ship, spawning, or implementation. Existing Graph relations and retrieval similarity are context only, not proof.

Choose exactly one relation:
- none: no useful semantic relationship at the specific workstream/scope level.
- topical: the Memories belong to the same specific semantic workstream/problem and are useful to traverse together, but no narrower relation below applies.
- factual: one Memory supplies a fact, premise, constraint, design fact, schema, or state that materially informs interpretation of the other. Direction is from the supporting/context Memory to the Memory it informs.
- causal: one Memory explicitly causes, enables, prevents, or materially produces the state/event in the other.
- recurrent: distinct observations/occurrences of the same pattern, not semantically identical duplicates.
- duplicate_of: materially the same durable proposition or observation with the same operative scope and no meaningful semantic difference. General and scoped variants of the same preference/rule are not duplicates unless their operative scopes are materially equivalent.
- supersedes: one Memory explicitly corrects, replaces, invalidates, or becomes the operative version of the other.

Before choosing none, ask whether retrieving B while investigating A's specific workstream would provide directly useful context, or vice versa. Before choosing topical, ask whether the connection would still exist if the broad project/domain label were removed. If the only remaining bridge is generic vocabulary or project proximity, choose none.

Abstract positive boundaries:
- compatibility/migration policy <-> current protocol/message implementation: related;
- two states or defects inside one specific user flow: related;
- workspace operating policy <-> a concrete repository/configuration fact inside that workspace: related;
- general stepwise-guidance preference <-> a troubleshooting/prompt-granularity specialization: related;
- current implementation summary <-> a concrete current capability encompassed by that summary: related.

Abstract negative boundaries:
- project-history/identity observation <-> unrelated later logger/network/gameplay implementation: none;
- ship/runtime architecture <-> asteroid spawning merely because both are gameplay code: none;
- asteroid collision setup <-> ship collision lookup merely because both contain the word collision: none.

Prefer a narrower supported relation over topical, but do not choose none merely because the useful same-workstream relationship is not directional.

Direction rules:
- none -> direction none.
- topical, recurrent, duplicate_of -> direction undirected.
- factual, causal, supersedes -> direction a_to_b or b_to_a according to meaning.

Direction ownership: for none, topical, recurrent, and duplicate_of, direction is mechanically implied by relation and Dream canonicalizes it deterministically. Do not spend semantic judgment on those directions. Only factual, causal, and supersedes require a directional judgment from the model.

Source timestamps are chronology context only. Deterministic temporal anchors/patterns are evidence about dates, ranges, and recurrence, but are not by themselves proof of causality, supersession, or recurrence between the two Memories. Existing Graph relations are supplemental context only and are not proof of the pair conclusion.

For every non-none conclusion, provide exactly two short verbatim evidence quotes: one copied from A title/content and one copied from B title/content. The quotes must support the direct semantic relationship, not merely broad shared context. For none, evidence must be empty. Do not paraphrase evidence.

Positive tie-break rules for choosing topical instead of none when no narrower relation applies:
- A broad current implementation-status Memory relates to a concrete current implementation capability, defect, or state that the status snapshot encompasses. This does not make project biography, history, or identity observations related to implementation details.
- Distinct steps, defects, or unresolved states inside one bounded user flow or lifecycle are related when they describe operationally connected stages of that same flow, even if they name different actions.
- Different implementation facets of the same durable runtime entity or subsystem can be related when they jointly define that entity's operation, state, representation, identity, configuration, or lifecycle. Entity continuity is useful scope evidence; do not transfer this rule across different entities merely because they share a generic implementation term.
- Lifecycle/operation ordering for an entity relates to concrete configuration, presentation, collision, or update behavior of that same entity when both materially describe how that entity exists or behaves at runtime.
- Stable user interaction or learning traits may relate to a durable guidance preference when one directly explains, motivates, or shapes the other.

These tie-breaks establish topical relatedness only when the semantic bridge survives removal of the broad project/domain label. If the bridge still depends only on project membership, chronology, or generic vocabulary, choose none.

State-machine/workflow clarification:
- For a bounded session, transaction, onboarding, deployment, or other state-machine-like flow, entry/configuration, progression/readiness, active-state, and exit/cleanup observations belong to the same workstream when they govern whether and how that same flow advances. They do not need to directly cause each other to be topical. Different states in the same flow are not made unrelated merely because they name different UI actions or transition messages.
- Do not extend this across independent subsystems that merely execute in the same application or implementation sequence.

Evidence copying rule: when a non-none relation is supported by each Memory title, prefer copying the exact full title for that side. Otherwise copy an exact contiguous substring from content. Never normalize spelling, punctuation, code formatting, path separators, or capitalization in evidence quotes.

Strong positive defaults at the same-workstream boundary:
- If one Memory is explicitly a broad current implementation/status summary and the other is a concrete current capability, runtime behavior, implemented feature, or defect within that same scoped system, classify them as related (normally topical) unless there is evidence they refer to different systems. The summary does not need to enumerate the concrete detail. Project biography/history, abandonment, authorship, or "first project" observations are not implementation details and do not qualify for this rule.
- If both Memories clearly describe states or transitions of the same bounded session/user lifecycle, configuration or admission state, readiness/start state, and exit/leave state are topical to one another even without a direct causal claim. Require the shared bounded lifecycle itself; do not infer this merely from being in the same application.

Concrete abstract workflow examples for the boundary:
- session readiness/start state <-> leave/exit transition state: related;
- session admission/configuration state <-> readiness/start state: related;
- admission/configuration state <-> leave/exit transition state: related.
These examples apply only when both Memories concern the same bounded session or user lifecycle. A start/update/spawn fact from a different subsystem does not become related merely because it also occurs during application runtime.

Bounded multiplayer/session-lifecycle clarification:
- A room/lobby/session lifecycle includes admission or room-selection state, readiness or match-start state, and leave/exit handling. Durable observations about those stages are topical to one another because they describe the operation of the same bounded multiplayer/session flow, even when one Memory names room codes, another names game/match start, and another names a leave/exit request.
- This does not make unrelated gameplay/runtime subsystems topical to session lifecycle merely because they run during the same game; entity spawning, rendering, collision, logging, and protocol implementation remain separate unless the Memories themselves establish a direct workstream bridge."#;

pub fn dream_classifier_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["relation", "direction", "evidence"],
        "properties": {
            "relation": {
                "type": "string",
                "enum": ["none", "topical", "factual", "causal", "recurrent", "duplicate_of", "supersedes"]
            },
            "direction": {
                "type": "string",
                "enum": ["none", "undirected", "a_to_b", "b_to_a"]
            },
            "evidence": {
                "type": "array",
                "maxItems": 2,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["side", "quote"],
                    "properties": {
                        "side": {"type": "string", "enum": ["a", "b"]},
                        "quote": {"type": "string", "minLength": 1, "maxLength": 500}
                    }
                }
            }
        }
    })
}
