use super::*;
use crate::management_workflow::hex;
use reliquary_owner_session::RelSession;
use reliquary_owner_session::model::{MemoryRecord, MemorySourceRef};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExactMemoryProvenance {
    pub memory_id: String,
    pub revision: u64,
    pub body_id: String,
    pub memory_version: u64,
    pub source_owner_id: Option<String>,
    pub principal_id: Option<String>,
    pub source_episode_id: Option<String>,
    pub source_node_id: Option<String>,
    pub source_time_ns: Option<i64>,
    /// Creation time of this exact Memory revision. Unlike source_time_ns,
    /// this belongs to the Memory record itself and remains available even
    /// when source content or source authorization is unavailable.
    pub created_at_ns: i64,
    pub source_archive_version: Option<u64>,
    pub content_source_reference: Option<SourceCoordinate>,
    pub grounding_source_reference: Option<SourceCoordinate>,
    pub source_turn: Option<SourceTurn>,
    pub content_source: Option<SourceTurn>,
    pub grounding_source: Option<SourceTurn>,
    pub availability: SourceAvailability,
    #[serde(skip)]
    retained_time_ns: Option<i64>,
    /// Native Memory records identify the supporting turn. They do not retain
    /// an authoritative byte span for the extraction quote.
    #[serde(skip)]
    source_ref: Option<MemorySourceRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceCoordinate {
    pub conversation_id: String,
    pub node_id: String,
}

impl ExactMemoryProvenance {
    pub fn source_owner(&self) -> Option<&str> {
        self.source_ref
            .as_ref()
            .map(|source| source.owner_id.as_str())
    }
}

pub(super) fn prepare_provenance(
    read: &ManagementRead<'_>,
    record: &MemoryRecord,
) -> Result<ExactMemoryProvenance, ManagementError> {
    if record.source_ref.as_ref().is_some_and(|source| {
        [
            Some(source.owner_id.as_str()),
            source.principal_id.as_deref(),
            Some(source.source_node_id.as_str()),
            source.content_source_conversation_id.as_deref(),
            source.content_source_node_id.as_deref(),
            source.grounding_source_conversation_id.as_deref(),
            source.grounding_source_node_id.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|field| field.len() > 4096)
    }) {
        return Err(ManagementError::PageBytesExceeded);
    }
    let mut provenance = ExactMemoryProvenance {
        memory_id: hex(&record.id.0),
        revision: record.revision,
        body_id: hex(&record.body_id.0),
        memory_version: record.memory_version,
        source_owner_id: None,
        principal_id: None,
        source_episode_id: None,
        source_node_id: None,
        source_time_ns: record.source_time_ns,
        created_at_ns: record.created_at_ns,
        source_archive_version: None,
        content_source_reference: None,
        grounding_source_reference: None,
        source_turn: None,
        content_source: None,
        grounding_source: None,
        availability: SourceAvailability::Absent,
        source_ref: record.source_ref.clone(),
        retained_time_ns: record.source_time_ns,
    };
    if let Some(source) = provenance.source_ref.as_ref() {
        if source.owner_id == hex(&read.owner_id) {
            expose_reference(&mut provenance);
            if let Some(rel) = read.rel {
                resolve_provenance(rel, &mut provenance)?;
            }
        } else {
            provenance.availability = SourceAvailability::Restricted;
            provenance.source_time_ns = None;
        }
    }
    Ok(provenance)
}

pub fn resolve_provenance(
    source_owner: &RelSession,
    result: &mut ExactMemoryProvenance,
) -> Result<(), ManagementError> {
    let Some(source) = result.source_ref.as_ref().cloned() else {
        return Ok(());
    };
    if hex(&source_owner.owner_id()) != source.owner_id {
        return Err(ManagementError::Stale);
    }
    expose_reference(result);
    result.source_archive_version = Some(source_owner.archive_version());
    if source.source_episode_id == [0; 32] && source.source_node_id.starts_with("management:") {
        result.availability = SourceAvailability::Direct;
        return Ok(());
    }
    let Some(conversation) = source_owner
        .management_episode_source(source.source_episode_id, &source.source_node_id)
        .map_err(|error| ManagementError::Owner(error.to_string()))?
    else {
        result.availability = SourceAvailability::Unavailable;
        return Ok(());
    };
    result.source_turn =
        archive::source_turn(source_owner, conversation, &source.source_node_id, true)?;
    if let Some(turn) = &result.source_turn {
        if source
            .principal_id
            .as_ref()
            .is_some_and(|id| Some(id) != turn.principal_id.as_ref())
        {
            return Err(ManagementError::Owner(
                "source principal disagrees with accepted Archive".into(),
            ));
        }
    }
    result.content_source = resolve_distinct(
        source_owner,
        source.content_source_conversation_id.as_deref(),
        source.content_source_node_id.as_deref(),
        conversation,
        &source.source_node_id,
    )?;
    result.grounding_source = resolve_distinct(
        source_owner,
        source.grounding_source_conversation_id.as_deref(),
        source.grounding_source_node_id.as_deref(),
        conversation,
        &source.source_node_id,
    )?;
    result.availability = result
        .source_turn
        .as_ref()
        .map_or(SourceAvailability::Unavailable, |turn| turn.availability);
    for (reference, turn) in [
        (&result.content_source_reference, &result.content_source),
        (&result.grounding_source_reference, &result.grounding_source),
    ] {
        if reference.as_ref().is_some_and(|reference| {
            (
                reference.conversation_id.as_str(),
                reference.node_id.as_str(),
            ) != (conversation, source.source_node_id.as_str())
        }) && turn
            .as_ref()
            .is_none_or(|turn| turn.availability != SourceAvailability::Available)
            && result.availability == SourceAvailability::Available
        {
            result.availability = SourceAvailability::Unavailable;
        }
    }
    Ok(())
}

fn expose_reference(result: &mut ExactMemoryProvenance) {
    if let Some(source) = result.source_ref.as_ref() {
        result.source_owner_id = Some(source.owner_id.clone());
        result.principal_id = source.principal_id.clone();
        result.source_episode_id =
            (source.source_episode_id != [0; 32]).then(|| hex(&source.source_episode_id));
        result.source_node_id = Some(source.source_node_id.clone());
        result.content_source_reference = coordinate(
            source.content_source_conversation_id.as_deref(),
            source.content_source_node_id.as_deref(),
        );
        result.grounding_source_reference = coordinate(
            source.grounding_source_conversation_id.as_deref(),
            source.grounding_source_node_id.as_deref(),
        );
        result.source_time_ns = result.retained_time_ns;
        result.availability = SourceAvailability::Unavailable;
    }
}

fn coordinate(conversation: Option<&str>, node: Option<&str>) -> Option<SourceCoordinate> {
    Some(SourceCoordinate {
        conversation_id: conversation?.to_owned(),
        node_id: node?.to_owned(),
    })
}

pub fn finish_provenance(page: &mut InspectionPage) -> Result<(), ManagementError> {
    for item in &mut page.items {
        if let InspectionItem::Provenance(provenance) = item {
            provenance.source_ref = None;
            provenance.retained_time_ns = None;
        }
    }
    if serde_json::to_vec(page)
        .map_err(|error| ManagementError::Owner(error.to_string()))?
        .len()
        > MAX_MANAGEMENT_PAGE_BYTES
    {
        return Err(ManagementError::PageBytesExceeded);
    }
    Ok(())
}

fn resolve_distinct(
    owner: &RelSession,
    conversation: Option<&str>,
    node: Option<&str>,
    primary_conversation: &str,
    primary_node: &str,
) -> Result<Option<SourceTurn>, ManagementError> {
    match (conversation, node) {
        (Some(conversation), Some(node))
            if (conversation, node) != (primary_conversation, primary_node) =>
        {
            archive::source_turn(owner, conversation, node, true)
        }
        (None, None) | (Some(_), Some(_)) => Ok(None),
        _ => Err(ManagementError::Owner(
            "incomplete accepted Archive source identity".into(),
        )),
    }
}
