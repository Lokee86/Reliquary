use crate::{DreamTemporalAnalysis, DreamTemporalMatch, GraphRelation, Memory, MemoryBodyId};

#[derive(Clone, Debug, PartialEq)]
pub struct DreamMemoryContext {
    pub memory: Memory,
    pub body_id: MemoryBodyId,
    pub source_timestamp_ns: Option<i64>,
    pub graph_relations: Vec<GraphRelation>,
    pub temporal: DreamTemporalAnalysis,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DreamCandidate {
    pub context: DreamMemoryContext,
    pub semantic_score: Option<f64>,
    pub semantic_rank: Option<usize>,
    pub prior_semantic_rank: Option<usize>,
    pub lexical_score: f64,
    pub lexical_rank: Option<usize>,
    pub temporal_score: f64,
    pub temporal_rank: Option<usize>,
    pub temporal_matches: Vec<DreamTemporalMatch>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DreamCandidateSet {
    pub source: DreamMemoryContext,
    pub candidates: Vec<DreamCandidate>,
}

impl reliquary_dream::DreamContext for DreamMemoryContext {
    fn memory(&self) -> &Memory {
        &self.memory
    }
    fn body_id(&self) -> MemoryBodyId {
        self.body_id
    }
    fn source_timestamp_ns(&self) -> Option<i64> {
        self.source_timestamp_ns
    }
    fn temporal(&self) -> &DreamTemporalAnalysis {
        &self.temporal
    }
    fn relations(&self) -> Vec<reliquary_dream::DreamRelationContext> {
        self.graph_relations
            .iter()
            .map(|r| reliquary_dream::DreamRelationContext {
                source: r.source,
                target: r.target,
                kind: r.kind,
            })
            .collect()
    }
}
impl reliquary_dream::DreamCandidateContexts for DreamCandidateSet {
    type Context = DreamMemoryContext;
    fn source(&self) -> &Self::Context {
        &self.source
    }
    fn candidates(&self) -> Vec<&Self::Context> {
        self.candidates.iter().map(|c| &c.context).collect()
    }
}
