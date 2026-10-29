use reliquary_chronos::{
    TemporalAnalysis, TemporalFrequency as DreamTemporalFrequency,
    TemporalGranularity as DreamTemporalGranularity, TemporalOrigin as DreamTemporalOrigin,
    TemporalWeekday as DreamTemporalWeekday,
};
use reliquary_graph::GraphRelationKind;
use reliquary_memory_core::{Memory, MemoryBodyId, MemoryId};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub struct DreamRelationContext {
    pub source: MemoryId,
    pub target: MemoryId,
    pub kind: GraphRelationKind,
}
pub trait DreamContext {
    fn memory(&self) -> &Memory;
    fn body_id(&self) -> MemoryBodyId;
    fn source_timestamp_ns(&self) -> Option<i64>;
    fn temporal(&self) -> &TemporalAnalysis;
    fn relations(&self) -> Vec<DreamRelationContext>;
}
pub trait DreamCandidateContexts {
    type Context: DreamContext;
    fn source(&self) -> &Self::Context;
    fn candidates(&self) -> Vec<&Self::Context>;
}
pub(crate) fn canonical_pair<'a, C: DreamContext>(
    left: &'a C,
    right: &'a C,
) -> Option<(&'a C, &'a C)> {
    if left.memory().id == right.memory().id {
        return None;
    }
    Some(if left.memory().id.0 < right.memory().id.0 {
        (left, right)
    } else {
        (right, left)
    })
}

pub(crate) fn context_json(context: &impl DreamContext) -> Value {
    let relations: Vec<_> = context
        .relations()
        .iter()
        .map(|relation| {
            json!({
                "source": hex(&relation.source.0),
                "target": hex(&relation.target.0),
                "kind": graph_relation_name(relation.kind),
            })
        })
        .collect();
    json!({
        "memory_id": hex(&context.memory().id.0),
        "body_id": hex(&context.body_id().0),
        "category": context.memory().category,
        "memory_type": context.memory().memory_type,
        "lifecycle_state": context.memory().lifecycle_state,
        "title": context.memory().title,
        "content": context.memory().content,
        "source_timestamp_ns": context.source_timestamp_ns(),
        "temporal": temporal_json(context),
        "graph_relations": relations,
    })
}

fn temporal_json(context: &impl DreamContext) -> Value {
    let anchors: Vec<_> = context
        .temporal()
        .anchors
        .iter()
        .map(|anchor| {
            json!({
                "start_ns": anchor.start_ns,
                "end_ns": anchor.end_ns,
                "granularity": granularity_name(anchor.granularity),
                "origin": origin_name(anchor.origin),
                "evidence": anchor.evidence,
            })
        })
        .collect();
    let patterns: Vec<_> = context
        .temporal()
        .patterns
        .iter()
        .map(|pattern| {
            json!({
                "frequency": frequency_name(pattern.frequency),
                "interval": pattern.interval,
                "weekday": pattern.weekday.map(weekday_name),
                "month_day": pattern.month_day,
                "month": pattern.month,
                "evidence": pattern.evidence,
            })
        })
        .collect();
    json!({"anchors": anchors, "patterns": patterns})
}

pub(crate) fn memory_text(context: &impl DreamContext) -> String {
    format!("{}\n{}", context.memory().title, context.memory().content)
}

pub(crate) fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn granularity_name(value: DreamTemporalGranularity) -> &'static str {
    match value {
        DreamTemporalGranularity::Instant => "instant",
        DreamTemporalGranularity::Day => "day",
        DreamTemporalGranularity::Week => "week",
        DreamTemporalGranularity::Month => "month",
        DreamTemporalGranularity::Quarter => "quarter",
        DreamTemporalGranularity::Season => "season",
        DreamTemporalGranularity::Year => "year",
        DreamTemporalGranularity::Range => "range",
    }
}

fn origin_name(value: DreamTemporalOrigin) -> &'static str {
    match value {
        DreamTemporalOrigin::Explicit => "explicit",
        DreamTemporalOrigin::Relative => "relative",
    }
}

fn frequency_name(value: DreamTemporalFrequency) -> &'static str {
    match value {
        DreamTemporalFrequency::Daily => "daily",
        DreamTemporalFrequency::Weekly => "weekly",
        DreamTemporalFrequency::Monthly => "monthly",
        DreamTemporalFrequency::Quarterly => "quarterly",
        DreamTemporalFrequency::Yearly => "yearly",
    }
}

fn weekday_name(value: DreamTemporalWeekday) -> &'static str {
    match value {
        DreamTemporalWeekday::Monday => "monday",
        DreamTemporalWeekday::Tuesday => "tuesday",
        DreamTemporalWeekday::Wednesday => "wednesday",
        DreamTemporalWeekday::Thursday => "thursday",
        DreamTemporalWeekday::Friday => "friday",
        DreamTemporalWeekday::Saturday => "saturday",
        DreamTemporalWeekday::Sunday => "sunday",
    }
}

fn graph_relation_name(kind: GraphRelationKind) -> &'static str {
    match kind {
        GraphRelationKind::Topical => "topical",
        GraphRelationKind::Factual => "factual",
        GraphRelationKind::Causal => "causal",
        GraphRelationKind::Recurrent => "recurrent",
        GraphRelationKind::References => "references",
        GraphRelationKind::DuplicateOf => "duplicate_of",
        GraphRelationKind::Supersedes => "supersedes",
        GraphRelationKind::StructuralParent => "structural_parent",
    }
}
