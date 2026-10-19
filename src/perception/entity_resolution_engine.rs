use crate::entity_admission::{is_bare_generic_surface, is_obvious_transient_occurrence};
use crate::entity_resolution_decision::try_decision_fast_path;
use crate::{
    DecisionEndpoint, EntityResolutionEvaluation, EntityResolutionPrepared, EntityResolver,
    EntityResolverError, GeneralEndpoint,
};
use std::sync::Arc;

pub struct EntityResolutionEngine<E> {
    fallback: EntityResolver<E>,
    decision: Option<Arc<dyn DecisionEndpoint>>,
}

impl<E: GeneralEndpoint> EntityResolutionEngine<E> {
    pub fn new(fallback: E) -> Self {
        Self {
            fallback: EntityResolver::new(fallback),
            decision: None,
        }
    }

    pub fn with_decision_endpoint(mut self, decision: Option<Arc<dyn DecisionEndpoint>>) -> Self {
        self.decision = decision;
        self
    }

    pub fn decision_model(&self) -> Option<&str> {
        self.decision.as_ref().map(|endpoint| endpoint.model())
    }

    pub fn fallback_model(&self) -> &str {
        self.fallback.model()
    }

    pub(crate) fn evaluate_prepared(
        &self,
        prepared: &EntityResolutionPrepared,
    ) -> Result<EntityResolutionEvaluation, EntityResolverError> {
        if is_bare_generic_surface(&prepared.candidates.mention.text) {
            return Ok(EntityResolutionEvaluation {
                output: crate::EntityResolverOutput {
                    decision: crate::EntityResolutionDecision::Reject,
                    reason: crate::EntityResolutionReason::GenericRole,
                },
                materialization: None,
            });
        }

        if is_obvious_transient_occurrence(&prepared.memory, &prepared.candidates) {
            return Ok(EntityResolutionEvaluation {
                output: crate::EntityResolverOutput {
                    decision: crate::EntityResolutionDecision::Reject,
                    reason: crate::EntityResolutionReason::TransientValue,
                },
                materialization: None,
            });
        }

        if let Some(decision) = &self.decision {
            if let Ok(Some(evaluation)) = try_decision_fast_path(decision.as_ref(), prepared) {
                return Ok(evaluation);
            }
        }
        self.fallback.evaluate_prepared(prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity_candidate_test_support::{
        entity_draft, install_rel_mention, publish_rel_memory, temp_path,
    };
    use crate::{
        Cva, EntityCandidateConfig, EntityId, EntityResolutionDecision, GraphRelationKind,
        SimulatedDecisionEndpoint, SimulatedGeneralEndpoint,
    };
    use serde_json::json;

    fn rel_with_candidate(name: &str) -> (Cva, crate::MemoryEntityMentionKey, EntityId) {
        let mut rel = Cva::create_project(temp_path(name)).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            name,
            "Editor",
            "Zephyr is the editor used for this project.",
        );
        let key = install_rel_mention(&mut rel, memory, "Zephyr");
        let entity_id = EntityId([7; 32]);
        rel.publish_entity(
            Some(entity_id),
            0,
            entity_draft("Zephyr", &[], "engine-candidate", 1),
        )
        .unwrap();
        (rel, key, entity_id)
    }

    #[test]
    fn high_confidence_decision_bypasses_fallback_model() {
        let (mut rel, key, entity_id) = rel_with_candidate("engine-fast-path.rel");
        let decision: Arc<dyn DecisionEndpoint> = Arc::new(SimulatedDecisionEndpoint::new(
            "jev",
            vec![json!({
                "answers": {
                    "resolution": {
                        "type": "choice",
                        "choice": "candidate_0",
                        "probabilities": {
                            "candidate_0": 0.97,
                            "create_new": 0.01,
                            "unresolved": 0.01,
                            "reject": 0.01
                        }
                    }
                }
            })],
        ));
        let engine =
            EntityResolutionEngine::new(SimulatedGeneralEndpoint::new("fallback", Vec::new()))
                .with_decision_endpoint(Some(decision));

        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 100)
            .unwrap();

        assert_eq!(
            outcome.decision,
            EntityResolutionDecision::ResolveExisting(entity_id)
        );
        assert_eq!(outcome.entity_id, Some(entity_id));
    }

    #[test]
    fn bare_generic_guard_precedes_candidate_resolution() {
        let mut rel = Cva::create_project(temp_path("engine-generic-role.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "generic-role",
            "Preference",
            "The user prefers concise workshop notes.",
        );
        let key = install_rel_mention(&mut rel, memory, "The user");
        let entity_id = publish_typed_entity(&mut rel, "Workshop Notes", "domain_entity", "notes");
        rel.set_entity_association(memory, entity_id, true, rel.graph_version())
            .unwrap();

        let engine =
            EntityResolutionEngine::new(SimulatedGeneralEndpoint::new("fallback", Vec::new()));
        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 102)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Reject);
        assert_eq!(outcome.reason, crate::EntityResolutionReason::GenericRole);
    }

    #[test]
    fn transient_gate_precedes_high_confidence_jev_match() {
        let mut rel = Cva::create_project(temp_path("engine-transient.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "transient",
            "Rollback",
            "Rollback to commit 6ed8e7a is final.",
        );
        let key = install_rel_mention(&mut rel, memory, "commit 6ed8e7a");
        let entity_id = EntityId([9; 32]);
        rel.publish_entity(
            Some(entity_id),
            0,
            entity_draft("commit 6ed8e7a", &[], "bad-commit", 1),
        )
        .unwrap();

        let decision: Arc<dyn DecisionEndpoint> = Arc::new(SimulatedDecisionEndpoint::new(
            "jev",
            vec![json!({
                "answers": {
                    "resolution": {
                        "type": "choice",
                        "choice": "candidate_0",
                        "probabilities": {
                            "candidate_0": 0.99,
                            "create_new": 0.0,
                            "unresolved": 0.0,
                            "reject": 0.01
                        }
                    }
                }
            })],
        ));
        let engine =
            EntityResolutionEngine::new(SimulatedGeneralEndpoint::new("fallback", Vec::new()))
                .with_decision_endpoint(Some(decision));

        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 103)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Reject);
        assert_eq!(outcome.entity_id, None);
        assert!(rel.entity_associations_for_memory(memory).is_empty());
    }

    #[test]
    fn high_confidence_jev_does_not_terminally_merge_lexical_only_candidate() {
        let mut rel = Cva::create_project(temp_path("engine-lexical-only.rel")).unwrap();
        let source = publish_rel_memory(
            &mut rel,
            "source",
            "Music binding",
            "background_music_player stores the scene music player reference.",
        );
        let key = install_rel_mention(&mut rel, source, "background_music_player");
        let evidence = publish_rel_memory(
            &mut rel,
            "evidence",
            "Background music",
            "BackgroundMusic is the scene node used for music playback.",
        );
        let entity_id = rel
            .publish_entity(
                None,
                0,
                entity_draft("BackgroundMusic", &[], "music-node", 1),
            )
            .unwrap()
            .0
            .id;
        rel.set_memory_relation(
            source,
            evidence,
            GraphRelationKind::Topical,
            true,
            rel.graph_version(),
        )
        .unwrap();
        rel.set_entity_association(evidence, entity_id, true, rel.graph_version())
            .unwrap();

        let decision: Arc<dyn DecisionEndpoint> = Arc::new(SimulatedDecisionEndpoint::new(
            "jev",
            vec![json!({
                "answers": {
                    "resolution": {
                        "type": "choice",
                        "choice": "candidate_0",
                        "probabilities": {
                            "candidate_0": 0.99,
                            "create_new": 0.0,
                            "unresolved": 0.01,
                            "reject": 0.0
                        }
                    }
                }
            })],
        ));
        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![json!({
                "decision": "unresolved",
                "reason": "insufficient_evidence",
                "target_candidate_index": -1
            })],
        );
        let engine = EntityResolutionEngine::new(fallback).with_decision_endpoint(Some(decision));

        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 104)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
        assert_eq!(outcome.entity_id, None);
    }

    #[test]
    fn high_confidence_jev_can_terminally_resolve_normalized_surface_candidate() {
        let mut rel = Cva::create_project(temp_path("engine-normalized.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "normalized",
            "Presenter",
            "Player Hue Presenter owns player hue presentation.",
        );
        let key = install_rel_mention(&mut rel, memory, "Player Hue Presenter");
        let entity_id = rel
            .publish_entity(
                None,
                0,
                entity_draft("PlayerHuePresenter", &[], "presenter", 1),
            )
            .unwrap()
            .0
            .id;

        let decision: Arc<dyn DecisionEndpoint> = Arc::new(SimulatedDecisionEndpoint::new(
            "jev",
            vec![json!({
                "answers": {
                    "resolution": {
                        "type": "choice",
                        "choice": "candidate_0",
                        "probabilities": {
                            "candidate_0": 0.99,
                            "create_new": 0.0,
                            "unresolved": 0.01,
                            "reject": 0.0
                        }
                    }
                }
            })],
        ));
        let engine =
            EntityResolutionEngine::new(SimulatedGeneralEndpoint::new("fallback", Vec::new()))
                .with_decision_endpoint(Some(decision));

        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 105)
            .unwrap();

        assert_eq!(
            outcome.decision,
            EntityResolutionDecision::ResolveExisting(entity_id)
        );
        assert_eq!(outcome.entity_id, Some(entity_id));
    }

    #[test]
    fn low_confidence_decision_falls_through_to_fallback_model() {
        let (mut rel, key, entity_id) = rel_with_candidate("engine-fallback.rel");
        let decision: Arc<dyn DecisionEndpoint> = Arc::new(SimulatedDecisionEndpoint::new(
            "jev",
            vec![json!({
                "answers": {
                    "resolution": {
                        "type": "choice",
                        "choice": "candidate_0",
                        "probabilities": {
                            "candidate_0": 0.80,
                            "create_new": 0.10,
                            "unresolved": 0.05,
                            "reject": 0.05
                        }
                    }
                }
            })],
        ));
        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![json!({
                "decision": "resolve_existing",
                "reason": "context_match",
                "target_candidate_index": 0
            })],
        );
        let engine = EntityResolutionEngine::new(fallback).with_decision_endpoint(Some(decision));

        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 101)
            .unwrap();

        assert_eq!(
            outcome.decision,
            EntityResolutionDecision::ResolveExisting(entity_id)
        );
        assert_eq!(outcome.entity_id, Some(entity_id));
    }

    #[test]
    fn general_model_choice_without_calibrated_confidence_uses_existing_resolver() {
        let (mut rel, key, entity_id) =
            rel_with_candidate("engine-general-decision-no-confidence.rel");
        let general: Arc<dyn GeneralEndpoint> = Arc::new(SimulatedGeneralEndpoint::new(
            "general-decision",
            vec![json!({
                "answers": {"resolution": {"type": "choice", "choice": "candidate_0"}}
            })],
        ));
        let decision: Arc<dyn DecisionEndpoint> = Arc::new(
            crate::general_decision::GeneralDecisionEndpoint::new(general),
        );
        let fallback = SimulatedGeneralEndpoint::new(
            "entity-resolver",
            vec![json!({
                "decision": "unresolved",
                "reason": "insufficient_evidence",
                "target_candidate_index": -1
            })],
        );
        let engine = EntityResolutionEngine::new(fallback).with_decision_endpoint(Some(decision));

        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 118)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
        assert_eq!(outcome.entity_id, None);
        assert_eq!(rel.entities().len(), 1);
        assert_eq!(rel.entities()[0].id, entity_id);
    }

    fn publish_typed_entity(rel: &mut Cva, name: &str, kind: &str, mutation: &str) -> EntityId {
        let mut draft = entity_draft(name, &[], mutation, 1);
        draft.kind = kind.into();
        rel.publish_entity(None, 0, draft).unwrap().0.id
    }

    #[test]
    fn fallback_related_distinct_cannot_merge_variable_into_runtime_entity() {
        let mut rel = Cva::create_project(temp_path("engine-related-distinct.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "music-binding",
            "Music binding",
            "background_music_player stores the BackgroundMusic node reference.",
        );
        let key = install_rel_mention(&mut rel, memory, "background_music_player");
        let entity_id =
            publish_typed_entity(&mut rel, "BackgroundMusic", "runtime_entity", "music-node");
        rel.set_entity_association(memory, entity_id, true, rel.graph_version())
            .unwrap();

        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![
                json!({
                    "decision": "resolve_existing",
                    "reason": "context_match",
                    "target_candidate_index": 0,
                    "identity_relation": "related_distinct",
                    "mention_kind": "code_symbol"
                }),
                json!({
                    "decision": "create_new",
                    "reason": "first_seen_identity",
                    "promotion_policy": "requires_recurrence",
                    "entity_kind": "code_symbol",
                    "entity_summary": "The background_music_player code symbol."
                }),
            ],
        );
        let engine = EntityResolutionEngine::new(fallback);

        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 106)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
        assert_eq!(outcome.entity_id, None);
        assert_eq!(rel.entities().len(), 1);
        assert!(matches!(
            rel.entity_resolution(key).unwrap().status,
            crate::MemoryEntityResolutionStatus::Pending(ref value)
                if value.reason == crate::EntityResolutionReason::RecurrenceRequired
        ));
    }

    #[test]
    fn fallback_kind_conflict_cannot_merge_file_into_code_type() {
        let mut rel = Cva::create_project(temp_path("engine-kind-conflict.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "connection-service-file",
            "ClientConnectionService",
            "client_connection_service.gd defines the ClientConnectionService type.",
        );
        let key = install_rel_mention(&mut rel, memory, "client_connection_service.gd");
        let entity_id = publish_typed_entity(
            &mut rel,
            "ClientConnectionService",
            "code_type",
            "connection-service-type",
        );
        rel.set_entity_association(memory, entity_id, true, rel.graph_version())
            .unwrap();

        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![
                json!({
                    "decision": "resolve_existing",
                    "reason": "context_match",
                    "target_candidate_index": 0,
                    "identity_relation": "same_identity",
                    "mention_kind": "file"
                }),
                json!({
                    "decision": "create_new",
                    "reason": "first_seen_identity",
                    "promotion_policy": "requires_recurrence",
                    "entity_kind": "file",
                    "entity_summary": "The client_connection_service.gd source file."
                }),
            ],
        );
        let engine = EntityResolutionEngine::new(fallback);

        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 107)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
        assert_eq!(outcome.entity_id, None);
        assert_eq!(rel.entities().len(), 1);
    }

    #[test]
    fn fallback_uncertain_identity_stays_unresolved() {
        let (mut rel, key, _) = rel_with_candidate("engine-uncertain.rel");
        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![json!({
                "decision": "resolve_existing",
                "reason": "context_match",
                "target_candidate_index": 0,
                "identity_relation": "uncertain",
                "mention_kind": "unknown"
            })],
        );
        let engine = EntityResolutionEngine::new(fallback);

        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 108)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
        assert_eq!(outcome.entity_id, None);
        assert_eq!(
            outcome.reason,
            crate::EntityResolutionReason::InsufficientEvidence
        );
    }

    #[test]
    fn v6_kind_guard_blocks_structural_relation_merges() {
        for (case, mention, mention_kind, candidate_name, candidate_kind) in [
            (
                "repo-app",
                "Space Rocks repository",
                "repository",
                "Space Rocks",
                "application",
            ),
            (
                "asset-effect",
                "torpedo_explosion.png",
                "file",
                "Torpedo explosion",
                "code_component",
            ),
            ("ui-app", "V2 UI", "ui_component", "V2", "application"),
            (
                "model-subsystem",
                "targeting read model",
                "code_type",
                "Targeting",
                "subsystem",
            ),
            (
                "method-type",
                "current_camera()",
                "code_symbol",
                "GameplayShellFlow",
                "code_type",
            ),
            (
                "directory-component",
                "networking/packets",
                "directory",
                "PacketCodec",
                "code_component",
            ),
        ] {
            let mut rel = Cva::create_project(temp_path(&format!("engine-{case}.rel"))).unwrap();
            let content =
                format!("{mention} is related to {candidate_name} but is a distinct referent.");
            let memory = publish_rel_memory(&mut rel, case, "Distinct referents", &content);
            let key = install_rel_mention(&mut rel, memory, mention);
            let entity_id = publish_typed_entity(&mut rel, candidate_name, candidate_kind, case);
            rel.set_entity_association(memory, entity_id, true, rel.graph_version())
                .unwrap();

            let fallback = SimulatedGeneralEndpoint::new(
                "fallback",
                vec![
                    json!({
                        "decision": "resolve_existing",
                        "reason": "context_match",
                        "target_candidate_index": 0,
                        "identity_relation": "same_identity",
                        "mention_kind": mention_kind
                    }),
                    json!({
                        "decision": "create_new",
                        "reason": "first_seen_identity",
                        "promotion_policy": "requires_recurrence",
                        "entity_kind": mention_kind,
                        "entity_summary": format!("The distinct {mention} referent.")
                    }),
                ],
            );
            let engine = EntityResolutionEngine::new(fallback);
            let outcome = rel
                .resolve_entity_mention_with_engine(
                    &engine,
                    key,
                    EntityCandidateConfig::default(),
                    109,
                )
                .unwrap();

            assert_ne!(
                outcome.decision,
                EntityResolutionDecision::ResolveExisting(entity_id),
                "{case} must not merge structurally distinct identities"
            );
            assert_eq!(outcome.entity_id, None, "{case}");
            assert_eq!(rel.entities().len(), 1, "{case}");
        }
    }

    #[test]
    fn fallback_consolidates_positive_alias_surface_candidates() {
        for (case, mention, candidate_name, kind) in [
            (
                "repo-url-alias",
                "https://github.com/Lokee86/space-rocks",
                "@SpaceRocks repository",
                "repository",
            ),
            ("uuid-plural-alias", "UUIDs", "UUID", "data_format"),
            (
                "gdscript-shorthand",
                "GDS",
                "GDScript",
                "programming_language",
            ),
        ] {
            let mut rel = Cva::create_project(temp_path(&format!("engine-{case}.rel"))).unwrap();
            let content =
                format!("{mention} denotes the same continuing identity as {candidate_name}.");
            let memory = publish_rel_memory(&mut rel, case, "Alias", &content);
            let key = install_rel_mention(&mut rel, memory, mention);
            let entity_id = publish_typed_entity(&mut rel, candidate_name, kind, case);

            let fallback = SimulatedGeneralEndpoint::new(
                "fallback",
                vec![json!({
                    "decision": "resolve_existing",
                    "reason": "context_match",
                    "target_candidate_index": 0,
                    "identity_relation": "same_identity",
                    "mention_kind": kind
                })],
            );
            let engine = EntityResolutionEngine::new(fallback);
            let outcome = rel
                .resolve_entity_mention_with_engine(
                    &engine,
                    key,
                    EntityCandidateConfig {
                        lexical_memory_limit: 0,
                        graph_neighbor_limit: 0,
                        ..EntityCandidateConfig::default()
                    },
                    110,
                )
                .unwrap();

            assert_eq!(
                outcome.decision,
                EntityResolutionDecision::ResolveExisting(entity_id),
                "{case}"
            );
            assert_eq!(outcome.entity_id, Some(entity_id), "{case}");
            assert_eq!(rel.entities().len(), 1, "{case}");
        }
    }

    #[test]
    fn related_distinct_exact_surface_same_kind_stays_unresolved() {
        let mut rel = Cva::create_project(temp_path("engine-python-exact-duplicate.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "python-language-duplicate",
            "Python",
            "Python is the programming language used by the data-sync tool.",
        );
        let key = install_rel_mention(&mut rel, memory, "Python");
        let entity_id = publish_typed_entity(
            &mut rel,
            "Python",
            "programming_language",
            "python-existing",
        );

        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![json!({
                "decision": "resolve_existing",
                "reason": "context_match",
                "target_candidate_index": 0,
                "identity_relation": "related_distinct",
                "mention_kind": "programming_language"
            })],
        );
        let engine = EntityResolutionEngine::new(fallback);
        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 115)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
        assert_eq!(outcome.reason, crate::EntityResolutionReason::Ambiguous);
        assert_eq!(outcome.entity_id, None);
        assert_eq!(rel.entities().len(), 1);
        assert_eq!(rel.entities()[0].id, entity_id);
    }

    #[test]
    fn create_new_related_distinct_exact_surface_same_kind_stays_unresolved() {
        let mut rel =
            Cva::create_project(temp_path("engine-python-create-new-duplicate.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "python-language-create-new",
            "Python",
            "The environment uses Python tooling.",
        );
        let key = install_rel_mention(&mut rel, memory, "Python");
        let entity_id = publish_typed_entity(
            &mut rel,
            "Python",
            "programming_language",
            "python-existing",
        );

        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![json!({
                "decision": "create_new",
                "reason": "context_conflict_new_identity",
                "target_candidate_index": 0,
                "identity_relation": "related_distinct",
                "mention_kind": "programming_language"
            })],
        );
        let engine = EntityResolutionEngine::new(fallback);
        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 116)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
        assert_eq!(outcome.reason, crate::EntityResolutionReason::Ambiguous);
        assert_eq!(outcome.entity_id, None);
        assert_eq!(rel.entities().len(), 1);
        assert_eq!(rel.entities()[0].id, entity_id);
    }

    #[test]
    fn post_admission_same_surface_same_final_kind_blocks_duplicate() {
        let mut rel =
            Cva::create_project(temp_path("engine-python-post-admission-duplicate.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "python-post-admission",
            "Python",
            "The environment uses Python tooling.",
        );
        let key = install_rel_mention(&mut rel, memory, "Python");
        let entity_id = publish_typed_entity(
            &mut rel,
            "Python",
            "programming_language",
            "python-existing",
        );

        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![
                json!({
                    "decision": "create_new",
                    "reason": "context_conflict_new_identity",
                    "target_candidate_index": 0,
                    "identity_relation": "related_distinct",
                    "mention_kind": "tool"
                }),
                json!({
                    "decision": "create_new",
                    "reason": "named_referent",
                    "promotion_policy": "immediate",
                    "entity_kind": "programming_language",
                    "entity_summary": "Python, the programming language used by project tooling and scripts."
                }),
            ],
        );
        let engine = EntityResolutionEngine::new(fallback);
        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 117)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
        assert_eq!(outcome.reason, crate::EntityResolutionReason::Ambiguous);
        assert_eq!(outcome.entity_id, None);
        assert_eq!(rel.entities().len(), 1);
        assert_eq!(rel.entities()[0].id, entity_id);
    }

    #[test]
    fn same_identity_kind_conflict_stays_unresolved_instead_of_creating_duplicate() {
        let mut rel = Cva::create_project(temp_path("engine-kind-disagreement.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "python-language",
            "Python",
            "Python is the programming language used by the data-sync tool.",
        );
        let key = install_rel_mention(&mut rel, memory, "Python");
        let entity_id = publish_typed_entity(
            &mut rel,
            "Python",
            "programming_language",
            "python-existing",
        );

        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![json!({
                "decision": "resolve_existing",
                "reason": "context_match",
                "target_candidate_index": 0,
                "identity_relation": "same_identity",
                "mention_kind": "tool"
            })],
        );
        let engine = EntityResolutionEngine::new(fallback);
        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 111)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
        assert_eq!(outcome.entity_id, None);
        assert_eq!(
            outcome.reason,
            crate::EntityResolutionReason::InsufficientEvidence
        );
        assert_eq!(rel.entities().len(), 1);
        assert_eq!(rel.entities()[0].id, entity_id);
    }

    #[test]
    fn admission_metadata_cannot_materialize_transient_branch_identity() {
        let mut rel = Cva::create_project(temp_path("engine-admission-transient.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "branch-ambiguous",
            "Client work",
            "clientv2 is used for the current development work.",
        );
        let key = install_rel_mention(&mut rel, memory, "clientv2");

        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![json!({
                "decision": "create_new",
                "reason": "first_seen_identity",
                "promotion_policy": "immediate",
                "entity_kind": "other",
                "entity_summary": "The source-control branch named clientv2."
            })],
        );
        let engine = EntityResolutionEngine::new(fallback);
        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 112)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Reject);
        assert_eq!(
            outcome.reason,
            crate::EntityResolutionReason::TransientValue
        );
        assert_eq!(outcome.entity_id, None);
        assert!(rel.entities().is_empty());
    }

    #[test]
    fn recurrence_required_reason_always_stays_pending() {
        let mut rel = Cva::create_project(temp_path("engine-recurrence-reason.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "player-data-service",
            "Player-data",
            "player-data service may become independently deployed later.",
        );
        let key = install_rel_mention(&mut rel, memory, "player-data service");
        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![json!({
                "decision": "reject",
                "reason": "recurrence_required",
                "promotion_policy": "none",
                "entity_kind": "unknown",
                "entity_summary": ""
            })],
        );
        let engine = EntityResolutionEngine::new(fallback);
        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 113)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
        assert_eq!(
            outcome.reason,
            crate::EntityResolutionReason::RecurrenceRequired
        );
        assert!(matches!(
            rel.entity_resolution(key).unwrap().status,
            crate::MemoryEntityResolutionStatus::Pending(ref value)
                if value.reason == crate::EntityResolutionReason::RecurrenceRequired
        ));
    }

    #[test]
    fn admission_rejects_general_category_materialization() {
        let mut rel = Cva::create_project(temp_path("engine-wrapper-category.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "generative-ai",
            "Generative AI",
            "generative-AI is a category of artificial-intelligence systems.",
        );
        let key = install_rel_mention(&mut rel, memory, "generative-AI");
        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![json!({
                "decision": "create_new",
                "reason": "first_seen_identity",
                "promotion_policy": "immediate",
                "entity_kind": "domain_entity",
                "entity_summary": "Generative AI, the category of artificial-intelligence systems used to generate content."
            })],
        );
        let engine = EntityResolutionEngine::new(fallback);
        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 114)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Reject);
        assert_eq!(
            outcome.reason,
            crate::EntityResolutionReason::WrapperCategory
        );
        assert!(rel.entities().is_empty());
    }

    #[test]
    fn recurrence_required_reason_is_always_pending_even_if_model_rejects() {
        let mut rel = Cva::create_project(temp_path("engine-recurrence-invariant.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "player-data-service",
            "Player data",
            "The player-data service may become independently deployable.",
        );
        let key = install_rel_mention(&mut rel, memory, "player-data service");
        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![json!({
                "decision": "reject",
                "reason": "recurrence_required",
                "promotion_policy": "none",
                "entity_kind": "unknown",
                "entity_summary": ""
            })],
        );
        let engine = EntityResolutionEngine::new(fallback);

        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 113)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
        assert_eq!(
            outcome.reason,
            crate::EntityResolutionReason::RecurrenceRequired
        );
        assert!(matches!(
            rel.entity_resolution(key).unwrap().status,
            crate::MemoryEntityResolutionStatus::Pending(ref value)
                if value.reason == crate::EntityResolutionReason::RecurrenceRequired
        ));
    }

    #[test]
    fn admission_metadata_cannot_materialize_wrapper_category() {
        let mut rel = Cva::create_project(temp_path("engine-wrapper-category.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "category",
            "Generative AI",
            "Generative AI is a category of systems used to generate content.",
        );
        let key = install_rel_mention(&mut rel, memory, "Generative AI");
        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![json!({
                "decision": "create_new",
                "reason": "first_seen_identity",
                "promotion_policy": "immediate",
                "entity_kind": "domain_entity",
                "entity_summary": "Generative AI, the general category of systems used to generate content."
            })],
        );
        let engine = EntityResolutionEngine::new(fallback);

        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 114)
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Reject);
        assert_eq!(
            outcome.reason,
            crate::EntityResolutionReason::WrapperCategory
        );
        assert!(rel.entities().is_empty());
    }

    #[test]
    fn decision_endpoint_failure_falls_through_to_fallback_model() {
        let (mut rel, key, entity_id) = rel_with_candidate("engine-decision-error.rel");
        let decision: Arc<dyn DecisionEndpoint> =
            Arc::new(SimulatedDecisionEndpoint::new("jev", Vec::new()));
        let fallback = SimulatedGeneralEndpoint::new(
            "fallback",
            vec![json!({
                "decision": "resolve_existing",
                "reason": "context_match",
                "target_candidate_index": 0
            })],
        );
        let engine = EntityResolutionEngine::new(fallback).with_decision_endpoint(Some(decision));

        let outcome = rel
            .resolve_entity_mention_with_engine(&engine, key, EntityCandidateConfig::default(), 102)
            .unwrap();

        assert_eq!(
            outcome.decision,
            EntityResolutionDecision::ResolveExisting(entity_id)
        );
        assert_eq!(outcome.entity_id, Some(entity_id));
    }
}
