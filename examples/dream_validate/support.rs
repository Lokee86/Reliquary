use reliquary_memory::{
    Cva, DreamCandidateConfig, DreamClassifier, DreamMemoryContext, DreamRelationDirection,
    DreamRelationKind, DreamTemporalAnalysis, DreamTemporalFrequency, DreamTemporalPattern,
    DreamTemporalWeekday, DreamVerificationVerdict, DreamVerifier, GeneralEndpoint,
    GeneralEndpointError, GraphRelation, Memory, MemoryBodyId, MemoryId,
    OpenAiReadyGeneralEndpoint,
};
use serde_json::Value;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct DiagnosticEndpoint {
    pub inner: OpenAiReadyGeneralEndpoint,
}

impl GeneralEndpoint for DiagnosticEndpoint {
    fn model(&self) -> &str {
        self.inner.model()
    }

    fn complete_json(
        &self,
        system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        let value = self
            .inner
            .complete_json(system_prompt, user_payload, schema_name, schema)?;
        println!("MODEL_OUTPUT\t{schema_name}\t{value}");
        Ok(value)
    }
}

pub fn run_corpus_validation(
    classifier: &DreamClassifier<DiagnosticEndpoint>,
    verifier: &DreamVerifier<DiagnosticEndpoint>,
    cva_path: PathBuf,
    fixture_path: PathBuf,
) -> Result<(), Box<dyn Error>> {
    let fixture: Value = serde_json::from_slice(&fs::read(fixture_path)?)?;
    let mut cva = Cva::open(cva_path)?;
    let profiles = cva.compatibility_profiles();
    if profiles.len() != 1 {
        return Err(format!(
            "expected one compatibility profile, found {}",
            profiles.len()
        )
        .into());
    }
    let profile_id = profiles[0].id;
    let stats = cva.memory_stats();
    println!(
        "CORPUS\tmemories={}\tbodies={}\tprofile_dimensions={}",
        stats.memories, stats.bodies, profiles[0].dimensions
    );

    let related = fixture["related_pairs"]
        .as_array()
        .ok_or("related_pairs must be an array")?;
    let mut recalled = 0usize;
    let mut non_none = 0usize;
    let mut verifier_accepts = 0usize;
    for (index, pair) in related.iter().enumerate() {
        let left = parse_id(pair["left"].as_str().ok_or("related left")?)?;
        let right = parse_id(pair["right"].as_str().ok_or("related right")?)?;
        let note = pair["note"].as_str().unwrap_or("");
        let candidates = cva.dream_candidates(profile_id, left, DreamCandidateConfig::default())?;
        let rank = candidates
            .candidates
            .iter()
            .position(|candidate| candidate.context.memory.id == right)
            .map(|rank| rank + 1);
        recalled += usize::from(rank.is_some());

        let left_context = memory_context(&mut cva, left)?;
        let right_context = memory_context(&mut cva, right)?;
        let classification = classifier.classify_pair(&left_context, &right_context)?;
        non_none += usize::from(classification.relation != DreamRelationKind::None);
        let verdict = if classification.relation == DreamRelationKind::None {
            None
        } else {
            let verification =
                verifier.verify_pair(&classification, &left_context, &right_context)?;
            verifier_accepts +=
                usize::from(verification.verdict == DreamVerificationVerdict::Accept);
            Some(verification.verdict)
        };
        println!(
            "RELATED\t{}\trank={:?}\trelation={:?}\tdirection={:?}\tverifier={:?}\t{}",
            index + 1,
            rank,
            classification.relation,
            classification.direction,
            verdict,
            note
        );
    }

    let negatives = fixture["negative_pairs"]
        .as_array()
        .ok_or("negative_pairs must be an array")?;
    let mut true_none = 0usize;
    for (index, pair) in negatives.iter().enumerate() {
        let left = parse_id(pair["left"].as_str().ok_or("negative left")?)?;
        let right = parse_id(pair["right"].as_str().ok_or("negative right")?)?;
        let note = pair["note"].as_str().unwrap_or("");
        let left_context = memory_context(&mut cva, left)?;
        let right_context = memory_context(&mut cva, right)?;
        let classification = classifier.classify_pair(&left_context, &right_context)?;
        true_none += usize::from(classification.relation == DreamRelationKind::None);
        println!(
            "NEGATIVE\t{}\trelation={:?}\tdirection={:?}\t{}",
            index + 1,
            classification.relation,
            classification.direction,
            note
        );
    }

    println!(
        "CORPUS_SUMMARY\trecall_at_12={}/{}\trelated_non_none={}/{}\trelated_verifier_accepts={}/{}\tnegative_none={}/{}",
        recalled,
        related.len(),
        non_none,
        related.len(),
        verifier_accepts,
        related.len(),
        true_none,
        negatives.len()
    );
    Ok(())
}

pub fn case(
    left_tag: u8,
    left_title: &str,
    left_content: &str,
    right_tag: u8,
    right_title: &str,
    right_content: &str,
    relation: DreamRelationKind,
    direction: DreamRelationDirection,
) -> (
    DreamMemoryContext,
    DreamMemoryContext,
    DreamRelationKind,
    DreamRelationDirection,
) {
    (
        synthetic_context(
            left_tag,
            left_title,
            left_content,
            DreamTemporalAnalysis::default(),
        ),
        synthetic_context(
            right_tag,
            right_title,
            right_content,
            DreamTemporalAnalysis::default(),
        ),
        relation,
        direction,
    )
}

pub fn recurring_case() -> (
    DreamMemoryContext,
    DreamMemoryContext,
    DreamRelationKind,
    DreamRelationDirection,
) {
    let pattern = DreamTemporalPattern {
        occurrences_per_period: 1,
        frequency: DreamTemporalFrequency::Weekly,
        interval: 1,
        weekday: Some(DreamTemporalWeekday::Tuesday),
        month_day: None,
        month: None,
        evidence: "weekly safety meeting".into(),
    };
    let left_temporal = DreamTemporalAnalysis {
        source_timestamp_ns: Some(1),
        anchors: Vec::new(),
        times_of_day: Vec::new(),
        durations: Vec::new(),
        duration_ranges: Vec::new(),
        approximate_durations: Vec::new(),
        event_relations: Vec::new(),
        intervals: Vec::new(),
        patterns: vec![pattern.clone()],
    };
    let right_temporal = DreamTemporalAnalysis {
        source_timestamp_ns: Some(2),
        anchors: Vec::new(),
        times_of_day: Vec::new(),
        durations: Vec::new(),
        duration_ranges: Vec::new(),
        approximate_durations: Vec::new(),
        event_relations: Vec::new(),
        intervals: Vec::new(),
        patterns: vec![pattern],
    };
    (
        synthetic_context(
            9,
            "Safety meeting 2026-08-18",
            "The weekly safety meeting occurred on 2026-08-18.",
            left_temporal,
        ),
        synthetic_context(
            10,
            "Safety meeting 2026-08-25",
            "The weekly safety meeting occurred on 2026-08-25.",
            right_temporal,
        ),
        DreamRelationKind::Recurrent,
        DreamRelationDirection::Undirected,
    )
}

fn memory_context(cva: &mut Cva, id: MemoryId) -> Result<DreamMemoryContext, Box<dyn Error>> {
    let memory = cva.memory(id)?;
    let body_id = cva.memory_body_id(id)?;
    let temporal = cva.dream_temporal_analysis(id)?;
    let graph_relations = cva
        .graph_relations()
        .into_iter()
        .filter(|relation| relation.source == id || relation.target == id)
        .collect::<Vec<GraphRelation>>();
    Ok(DreamMemoryContext {
        memory,
        body_id,
        source_timestamp_ns: temporal.source_timestamp_ns,
        graph_relations,
        temporal,
    })
}

fn synthetic_context(
    tag: u8,
    title: &str,
    content: &str,
    temporal: DreamTemporalAnalysis,
) -> DreamMemoryContext {
    let id = MemoryId([tag; 32]);
    DreamMemoryContext {
        memory: Memory {
            id,
            revision: 1,
            category: "validation".into(),
            memory_type: "synthetic".into(),
            authority_kind: "unknown".into(),
            temporal_status: "unknown".into(),
            title: title.into(),
            content: content.into(),
            scope: "validation".into(),
            lifecycle_state: "extracted".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_time_ns: None,
            source_ref: None,
            temporal_inference: None,
            routing_metadata: None,
            mutation_id: format!("dream-validation-{tag}"),
            created_at_ns: 0,
            updated_at_ns: 0,
            global_version: 0,
            memory_version: 0,
        },
        body_id: MemoryBodyId([tag.wrapping_add(100); 32]),
        source_timestamp_ns: temporal.source_timestamp_ns,
        graph_relations: Vec::new(),
        temporal,
    }
}

fn parse_id(value: &str) -> Result<MemoryId, Box<dyn Error>> {
    if value.len() != 64 {
        return Err("MemoryId must be 64 hex characters".into());
    }
    let mut bytes = [0u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = u8::from_str_radix(std::str::from_utf8(pair)?, 16)?;
    }
    Ok(MemoryId(bytes))
}
