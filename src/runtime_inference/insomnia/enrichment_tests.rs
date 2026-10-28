use super::extraction::{InsomniaCandidate, InsomniaRoutingMetadata};
use crate::{MemoryTextField, SimulatedGeneralEndpoint};
use reliquary_insomnia::InsomniaOwnership;
use reliquary_insomnia::enrich;
use serde_json::json;

#[test]
fn enrichment_maps_verbatim_mentions_to_durable_text_spans() {
    let mut candidates = vec![candidate()];
    let endpoint = SimulatedGeneralEndpoint::new(
        "metadata",
        vec![json!({
            "memories": {
                "claim-1": {
                    "entity_mentions": [
                        {"field":"content","text":"Sarah","occurrence":-1},
                        {"field":"content","text":"the Vancouver office","occurrence":-1},
                        {"field":"content","text":"Warlock","occurrence":-1}
                    ]
                }
            }
        })],
    );

    enrich(&endpoint, &mut candidates).unwrap();
    let routing = candidates[0].routing_metadata.as_ref().unwrap();
    assert_eq!(routing.entity_mentions.len(), 3);
    assert_eq!(routing.entity_mentions[0].field, MemoryTextField::Content);
    for mention in &routing.entity_mentions {
        let start = mention.start_byte as usize;
        let end = mention.end_byte as usize;
        assert_eq!(&candidates[0].content[start..end], mention.text);
    }
}

#[test]
fn enrichment_rejects_expansion_beyond_durable_mention_limit() {
    let mut candidate = candidate();
    candidate.content = std::iter::repeat_n("Sarah", 65)
        .collect::<Vec<_>>()
        .join(" ");
    let mut candidates = vec![candidate];
    let endpoint = SimulatedGeneralEndpoint::new(
        "metadata",
        vec![json!({
            "memories": {
                "claim-1": {
                    "entity_mentions": [
                        {"field":"content","text":"Sarah","occurrence":-1}
                    ]
                }
            }
        })],
    );

    let error = enrich(&endpoint, &mut candidates).unwrap_err();
    assert!(error.to_string().contains("bounded item limit"));
}

#[test]
fn enrichment_does_not_expand_word_mentions_inside_longer_identifiers() {
    let mut candidate = candidate();
    candidate.content = "Use `wgit` instead of `git`; damageResult wraps damage.".into();
    let mut candidates = vec![candidate];
    let endpoint = SimulatedGeneralEndpoint::new(
        "metadata",
        vec![json!({
            "memories": {
                "claim-1": {
                    "entity_mentions": [
                        {"field":"content","text":"git","occurrence":-1},
                        {"field":"content","text":"damage","occurrence":-1}
                    ]
                }
            }
        })],
    );

    enrich(&endpoint, &mut candidates).unwrap();
    let routing = candidates[0].routing_metadata.as_ref().unwrap();
    let mentions = routing
        .entity_mentions
        .iter()
        .map(|mention| (mention.text.as_str(), mention.start_byte, mention.end_byte))
        .collect::<Vec<_>>();
    assert_eq!(mentions, vec![("git", 23, 26), ("damage", 48, 54)]);
}

#[test]
fn enrichment_selects_one_semantic_occurrence_when_surface_text_is_ambiguous() {
    let mut candidate = candidate();
    candidate.content = "local damage result shadows imported `damage` package.".into();
    let expected_start = candidate.content.rfind("damage").unwrap() as u32;
    let mut candidates = vec![candidate];
    let endpoint = SimulatedGeneralEndpoint::new(
        "metadata",
        vec![json!({
            "memories": {
                "claim-1": {
                    "entity_mentions": [
                        {"field":"content","text":"damage","occurrence":1}
                    ]
                }
            }
        })],
    );

    enrich(&endpoint, &mut candidates).unwrap();
    let routing = candidates[0].routing_metadata.as_ref().unwrap();
    assert_eq!(routing.entity_mentions.len(), 1);
    assert_eq!(routing.entity_mentions[0].text, "damage");
    assert_eq!(routing.entity_mentions[0].start_byte, expected_start);
}

#[test]
fn enrichment_drops_only_embedded_word_occurrences() {
    let mut candidate = candidate();
    candidate.content = "damageResult is populated.".into();
    let mut candidates = vec![candidate];
    let endpoint = SimulatedGeneralEndpoint::new(
        "metadata",
        vec![json!({
            "memories": {
                "claim-1": {
                    "entity_mentions": [
                        {"field":"content","text":"damage","occurrence":-1}
                    ]
                }
            }
        })],
    );

    enrich(&endpoint, &mut candidates).unwrap();
    assert!(
        candidates[0]
            .routing_metadata
            .as_ref()
            .unwrap()
            .entity_mentions
            .is_empty()
    );
}

#[test]
fn enrichment_normalizes_ascii_case_only_drift_to_source_text() {
    let mut candidate = candidate();
    candidate.title = "Diagnostic Backout".into();
    candidate.content = "The user prefers Git.".into();
    let mut candidates = vec![candidate];
    let endpoint = SimulatedGeneralEndpoint::new(
        "metadata",
        vec![json!({
            "memories": {
                "claim-1": {
                    "entity_mentions": [
                        {"field":"content","text":"the user","occurrence":-1}
                    ]
                }
            }
        })],
    );

    enrich(&endpoint, &mut candidates).unwrap();
    let routing = candidates[0].routing_metadata.as_ref().unwrap();
    assert_eq!(routing.entity_mentions[0].text, "The user");
}

#[test]
fn enrichment_keeps_most_specific_overlapping_mentions() {
    let mut candidate = candidate();
    candidate.content =
        "Windows Git and Git; Go uses Go-game-server; State wraps Game.State.".into();
    let mut candidates = vec![candidate];
    let endpoint = SimulatedGeneralEndpoint::new(
        "metadata",
        vec![json!({
            "memories": {
                "claim-1": {
                    "entity_mentions": [
                        {"field":"content","text":"Windows","occurrence":-1},
                        {"field":"content","text":"Git","occurrence":-1},
                        {"field":"content","text":"Windows Git","occurrence":-1},
                        {"field":"content","text":"Go","occurrence":-1},
                        {"field":"content","text":"Go-game-server","occurrence":-1},
                        {"field":"content","text":"State","occurrence":-1},
                        {"field":"content","text":"Game.State","occurrence":-1}
                    ]
                }
            }
        })],
    );

    enrich(&endpoint, &mut candidates).unwrap();
    let mentions = candidates[0]
        .routing_metadata
        .as_ref()
        .unwrap()
        .entity_mentions
        .iter()
        .map(|mention| mention.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        mentions,
        vec![
            "Windows Git",
            "Git",
            "Go",
            "Go-game-server",
            "State",
            "Game.State"
        ]
    );
}

#[test]
fn enrichment_filters_generic_referents_but_keeps_reusable_identity() {
    let mut candidate = candidate();
    candidate.content = "The server uses Rails API server at the Vancouver office; the file is game.gd, the user approved it, Codec seam is only an architectural seam, and the Leave request crossed the pause flow.".into();
    let mut candidates = vec![candidate];
    let endpoint = SimulatedGeneralEndpoint::new(
        "metadata",
        vec![json!({
            "memories": {
                "claim-1": {
                    "entity_mentions": [
                        {"field":"content","text":"The server","occurrence":-1},
                        {"field":"content","text":"Rails API server","occurrence":-1},
                        {"field":"content","text":"the Vancouver office","occurrence":-1},
                        {"field":"content","text":"the file","occurrence":-1},
                        {"field":"content","text":"game.gd","occurrence":-1},
                        {"field":"content","text":"the user","occurrence":-1},
                        {"field":"content","text":"Codec seam","occurrence":-1},
                        {"field":"content","text":"architectural seam","occurrence":-1},
                        {"field":"content","text":"Leave request","occurrence":-1},
                        {"field":"content","text":"pause flow","occurrence":-1}
                    ]
                }
            }
        })],
    );

    enrich(&endpoint, &mut candidates).unwrap();
    let mentions = candidates[0]
        .routing_metadata
        .as_ref()
        .unwrap()
        .entity_mentions
        .iter()
        .map(|mention| mention.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        mentions,
        vec![
            "Rails API server",
            "the Vancouver office",
            "game.gd",
            "the user"
        ]
    );
}

#[test]
fn enrichment_strips_inline_code_delimiters_from_entity_mentions() {
    let mut candidate = candidate();
    candidate.content = "Move `game.gd` into `networking/packets`.".into();
    let mut candidates = vec![candidate];
    let endpoint = SimulatedGeneralEndpoint::new(
        "metadata",
        vec![json!({
            "memories": {
                "claim-1": {
                    "entity_mentions": [
                        {"field":"content","text":"`game.gd`","occurrence":-1},
                        {"field":"content","text":"`networking/packets`","occurrence":-1}
                    ]
                }
            }
        })],
    );

    enrich(&endpoint, &mut candidates).unwrap();
    let mentions = candidates[0]
        .routing_metadata
        .as_ref()
        .unwrap()
        .entity_mentions
        .iter()
        .map(|mention| mention.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(mentions, vec!["game.gd", "networking/packets"]);
}

#[test]
fn enrichment_drops_invented_entity_names() {
    let mut candidates = vec![candidate()];
    let endpoint = SimulatedGeneralEndpoint::new(
        "metadata",
        vec![json!({
            "memories": {
                "claim-1": {
                    "entity_mentions": [
                        {"field":"content","text":"Sarah Chen","occurrence":-1}
                    ]
                }
            }
        })],
    );

    enrich(&endpoint, &mut candidates).unwrap();
    assert!(
        candidates[0]
            .routing_metadata
            .as_ref()
            .unwrap()
            .entity_mentions
            .is_empty()
    );
}

fn candidate() -> InsomniaCandidate {
    InsomniaCandidate {
        key: "claim-1".into(),
        authority_kind: "direct".into(),
        category: "fact".into(),
        memory_type: "project".into(),
        temporal_status: "current".into(),
        ownership: InsomniaOwnership::Project,
        title: "Reliquary entity work".into(),
        content: "Sarah works at the Vancouver office with Warlock.".into(),
        source_node_id: "u1".into(),
        source_quote: "Sarah works at the Vancouver office with Warlock.".into(),
        authority_source_conversation_id: None,
        authority_source_node_id: None,
        authority_source_quote: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        grounding_source_quote: None,
        routing_metadata: None::<InsomniaRoutingMetadata>,
    }
}
