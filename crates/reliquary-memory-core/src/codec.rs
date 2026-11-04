use crate::{error::MemoryError, model::*};

pub(super) const FORMAT: &[u8; 8] = b"RELMFMT1";
pub(super) const BODY: &[u8; 8] = b"RELMBOD1";
pub(super) const CREATION: &[u8; 8] = b"RELMCRT1";
pub(super) const DELTA: &[u8; 8] = b"RELMDLT1";
pub(super) const TOMBSTONE: &[u8; 8] = b"RELMTMB1";
const KNOWN_DELTA_MASK: u64 = (1 << 20) - 1;
pub(super) const MAX_FIELD_BYTES: usize = 16 * 1024 * 1024;
pub(super) const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
pub(super) fn validate_draft(draft: &MemoryDraft) -> Result<(), MemoryError> {
    if draft.mutation_id.is_empty() {
        return Err(MemoryError::InvalidField("mutation_id"));
    }
    if draft.category.is_empty() {
        return Err(MemoryError::InvalidField("category"));
    }
    if draft.memory_type.is_empty() {
        return Err(MemoryError::InvalidField("memory_type"));
    }
    if draft.authority_kind.is_empty() {
        return Err(MemoryError::InvalidField("authority_kind"));
    }
    if draft.temporal_status.is_empty() {
        return Err(MemoryError::InvalidField("temporal_status"));
    }
    if draft.scope.is_empty() {
        return Err(MemoryError::InvalidField("scope"));
    }
    if draft.lifecycle_state.is_empty() {
        return Err(MemoryError::InvalidField("lifecycle_state"));
    }
    if draft.title.len() > MAX_FIELD_BYTES
        || draft.content.len() > MAX_BODY_BYTES
        || draft.mutation_id.len() > MAX_FIELD_BYTES
    {
        return Err(MemoryError::FieldTooLarge);
    }
    if let Some(source) = &draft.source_ref {
        validate_source_ref(source)?;
    }
    for value in [
        draft.source_node_id.as_deref(),
        draft.content_source_conversation_id.as_deref(),
        draft.content_source_node_id.as_deref(),
        draft.grounding_source_conversation_id.as_deref(),
        draft.grounding_source_node_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if value.len() > MAX_FIELD_BYTES {
            return Err(MemoryError::FieldTooLarge);
        }
    }
    Ok(())
}

fn validate_source_ref(source: &MemorySourceRef) -> Result<(), MemoryError> {
    if source.owner_id.is_empty() || source.source_node_id.is_empty() {
        return Err(MemoryError::InvalidField("source_ref"));
    }
    for value in [
        Some(source.owner_id.as_str()),
        Some(source.source_node_id.as_str()),
        source.principal_id.as_deref(),
        source.content_source_conversation_id.as_deref(),
        source.content_source_node_id.as_deref(),
        source.grounding_source_conversation_id.as_deref(),
        source.grounding_source_node_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if value.len() > MAX_FIELD_BYTES {
            return Err(MemoryError::FieldTooLarge);
        }
    }
    Ok(())
}

pub(super) fn validate_replayed_record(
    record: &MemoryRecord,
    body: Option<&MemoryBody>,
) -> Result<(), MemoryError> {
    if [
        &record.mutation_id,
        &record.category,
        &record.memory_type,
        &record.authority_kind,
        &record.temporal_status,
        &record.scope,
        &record.lifecycle_state,
    ]
    .iter()
    .any(|value| value.is_empty())
    {
        return Err(MemoryError::Corrupt("empty required Memory field"));
    }
    if let Some(source) = &record.source_ref {
        validate_source_ref(source)?;
    }
    if let Some(proof) = &record.temporal_inference {
        let body_len = body.map_or(usize::MAX, |body| {
            body.title.len().saturating_add(body.content.len())
        });
        validate_temporal_inference(proof, record.body_id, record.source_time_ns, body_len)?;
    }
    Ok(())
}

pub(super) fn validate_temporal_inference(
    proof: &MemoryTemporalInference,
    body_id: MemoryBodyId,
    source_time_ns: Option<i64>,
    body_len: usize,
) -> Result<(), MemoryError> {
    if proof.body_id != body_id || proof.source_time_ns != source_time_ns {
        return Err(MemoryError::InvalidField("temporal_inference_binding"));
    }
    if proof.inference.model.is_empty() || proof.inference.contract_version.is_empty() {
        return Err(MemoryError::InvalidField("temporal_inference"));
    }
    for resolution in &proof.inference.resolutions {
        if resolution.start_byte > resolution.end_byte || resolution.end_byte > body_len {
            return Err(MemoryError::InvalidField("temporal_inference_span"));
        }
        if resolution.evidence.len() > MAX_FIELD_BYTES
            || resolution.canonical_expression.len() > MAX_FIELD_BYTES
        {
            return Err(MemoryError::FieldTooLarge);
        }
    }
    Ok(())
}

pub(super) fn draft_matches(record: &MemoryRecord, draft: &MemoryDraft) -> bool {
    record.category == draft.category
        && record.memory_type == draft.memory_type
        && record.authority_kind == draft.authority_kind
        && record.temporal_status == draft.temporal_status
        && record.scope == draft.scope
        && record.lifecycle_state == draft.lifecycle_state
        && record.archived == draft.archived
        && record.parent_id == draft.parent_id
        && record.superseded_by == draft.superseded_by
        && record.source_node_id == draft.source_node_id
        && record.content_source_conversation_id == draft.content_source_conversation_id
        && record.content_source_node_id == draft.content_source_node_id
        && record.grounding_source_conversation_id == draft.grounding_source_conversation_id
        && record.grounding_source_node_id == draft.grounding_source_node_id
        && record.source_episode_id == draft.source_episode_id
        && draft
            .source_ref
            .as_ref()
            .is_none_or(|source| record.source_ref.as_ref() == Some(source))
        && record.source_time_ns == draft.source_time_ns
        && draft
            .temporal_inference
            .as_ref()
            .is_none_or(|value| record.temporal_inference.as_ref() == Some(value))
        && record.mutation_id == draft.mutation_id
        && record.created_at_ns == draft.created_at_ns
        && record.updated_at_ns == draft.updated_at_ns
}

pub(super) fn encode_body(id: MemoryBodyId, body: &MemoryBody) -> Result<Vec<u8>, MemoryError> {
    let bytes = memory_body_bytes(&body.title, &body.content);
    if bytes.len() > MAX_BODY_BYTES {
        return Err(MemoryError::FieldTooLarge);
    }
    let mut out = Vec::with_capacity(40 + bytes.len());
    out.extend_from_slice(BODY);
    out.extend_from_slice(&id.0);
    out.extend(bytes);
    Ok(out)
}

pub(super) fn decode_body(bytes: &[u8]) -> Result<(MemoryBodyId, MemoryBody), MemoryError> {
    let mut r = Reader::new(&bytes[8..]);
    let id = MemoryBodyId(r.array()?);
    let title = r.string_u64()?;
    let content = r.string_u64()?;
    r.finish()?;
    Ok((id, MemoryBody { title, content }))
}

pub(super) fn encode_creation(record: &MemoryRecord) -> Result<Vec<u8>, MemoryError> {
    let mut out = Vec::new();
    out.extend_from_slice(CREATION);
    write_record_identity(&mut out, record)?;
    write_record_fields(&mut out, record)?;
    Ok(out)
}

pub(super) fn encode_delta(
    previous: &MemoryRecord,
    next: &MemoryRecord,
) -> Result<Vec<u8>, MemoryError> {
    if previous.id != next.id || previous.body_id != next.body_id {
        return Err(MemoryError::ImmutableBodyChanged);
    }
    if previous.revision.checked_add(1) != Some(next.revision)
        || next.memory_version <= previous.memory_version
        || next.mutation_id == previous.mutation_id
    {
        return Err(MemoryError::RevisionConflict);
    }
    let mask = delta_mask(previous, next);
    let mut out = Vec::new();
    out.extend_from_slice(DELTA);
    out.extend_from_slice(&next.id.0);
    put_u64(&mut out, previous.revision);
    put_u64(&mut out, next.revision);
    out.extend_from_slice(&next.body_id.0);
    put_u64(&mut out, next.memory_version);
    put_string(&mut out, &next.mutation_id)?;
    out.extend_from_slice(&mask.to_le_bytes());
    if mask & (1 << 0) != 0 {
        put_string(&mut out, &next.category)?;
    }
    if mask & (1 << 1) != 0 {
        put_string(&mut out, &next.memory_type)?;
    }
    if mask & (1 << 2) != 0 {
        put_string(&mut out, &next.authority_kind)?;
    }
    if mask & (1 << 3) != 0 {
        put_string(&mut out, &next.temporal_status)?;
    }
    if mask & (1 << 4) != 0 {
        put_string(&mut out, &next.scope)?;
    }
    if mask & (1 << 5) != 0 {
        put_string(&mut out, &next.lifecycle_state)?;
    }
    if mask & (1 << 6) != 0 {
        out.push(u8::from(next.archived));
    }
    if mask & (1 << 7) != 0 {
        put_option_id(&mut out, next.superseded_by);
    }
    if mask & (1 << 8) != 0 {
        put_option_id(&mut out, next.parent_id);
    }
    if mask & (1 << 9) != 0 {
        put_optional_string(&mut out, next.source_node_id.as_deref())?;
    }
    if mask & (1 << 10) != 0 {
        put_optional_string(&mut out, next.content_source_conversation_id.as_deref())?;
    }
    if mask & (1 << 11) != 0 {
        put_optional_string(&mut out, next.content_source_node_id.as_deref())?;
    }
    if mask & (1 << 12) != 0 {
        put_optional_string(&mut out, next.grounding_source_conversation_id.as_deref())?;
    }
    if mask & (1 << 13) != 0 {
        put_optional_string(&mut out, next.grounding_source_node_id.as_deref())?;
    }
    if mask & (1 << 14) != 0 {
        put_option_bytes32(&mut out, next.source_episode_id);
    }
    if mask & (1 << 15) != 0 {
        put_option_i64(&mut out, next.source_time_ns);
    }
    if mask & (1 << 16) != 0 {
        put_source_ref(&mut out, next.source_ref.as_ref())?;
    }
    if mask & (1 << 17) != 0 {
        put_temporal_inference(&mut out, next.temporal_inference.as_ref())?;
    }
    if mask & (1 << 18) != 0 {
        put_i64(&mut out, next.created_at_ns);
    }
    if mask & (1 << 19) != 0 {
        put_i64(&mut out, next.updated_at_ns);
    }
    Ok(out)
}

fn delta_mask(a: &MemoryRecord, b: &MemoryRecord) -> u64 {
    (u64::from(a.category != b.category) << 0)
        | (u64::from(a.memory_type != b.memory_type) << 1)
        | (u64::from(a.authority_kind != b.authority_kind) << 2)
        | (u64::from(a.temporal_status != b.temporal_status) << 3)
        | (u64::from(a.scope != b.scope) << 4)
        | (u64::from(a.lifecycle_state != b.lifecycle_state) << 5)
        | (u64::from(a.archived != b.archived) << 6)
        | (u64::from(a.superseded_by != b.superseded_by) << 7)
        | (u64::from(a.parent_id != b.parent_id) << 8)
        | (u64::from(a.source_node_id != b.source_node_id) << 9)
        | (u64::from(a.content_source_conversation_id != b.content_source_conversation_id) << 10)
        | (u64::from(a.content_source_node_id != b.content_source_node_id) << 11)
        | (u64::from(a.grounding_source_conversation_id != b.grounding_source_conversation_id)
            << 12)
        | (u64::from(a.grounding_source_node_id != b.grounding_source_node_id) << 13)
        | (u64::from(a.source_episode_id != b.source_episode_id) << 14)
        | (u64::from(a.source_time_ns != b.source_time_ns) << 15)
        | (u64::from(a.source_ref != b.source_ref) << 16)
        | (u64::from(a.temporal_inference != b.temporal_inference) << 17)
        | (u64::from(a.created_at_ns != b.created_at_ns) << 18)
        | (u64::from(a.updated_at_ns != b.updated_at_ns) << 19)
}

pub(super) fn encode_tombstone(id: MemoryId, revision: u64, memory_version: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(56);
    out.extend_from_slice(TOMBSTONE);
    out.extend_from_slice(&id.0);
    put_u64(&mut out, revision);
    put_u64(&mut out, memory_version);
    out
}

pub(super) fn decode_creation(bytes: &[u8]) -> Result<MemoryRecord, MemoryError> {
    let mut r = Reader::new(&bytes[8..]);
    let id = MemoryId(r.array()?);
    let revision = r.u64()?;
    let body_id = MemoryBodyId(r.array()?);
    let memory_version = r.u64()?;
    let mutation_id = r.string()?;
    let category = r.string()?;
    let memory_type = r.string()?;
    let authority_kind = r.string()?;
    let temporal_status = r.string()?;
    let scope = r.string()?;
    let lifecycle_state = r.string()?;
    let archived = r.bool()?;
    let superseded_by = r.option_id()?;
    let parent_id = r.option_id()?;
    let source_node_id = r.optional_string()?;
    let content_source_conversation_id = r.optional_string()?;
    let content_source_node_id = r.optional_string()?;
    let grounding_source_conversation_id = r.optional_string()?;
    let grounding_source_node_id = r.optional_string()?;
    let source_episode_id = r.option_array32()?;
    let source_ref = r.source_ref()?;
    let source_time_ns = r.option_i64()?;
    let temporal_inference = r.temporal_inference()?;
    let created_at_ns = r.i64()?;
    let updated_at_ns = r.i64()?;
    r.finish()?;
    Ok(MemoryRecord {
        id,
        revision,
        body_id,
        category,
        memory_type,
        authority_kind,
        temporal_status,
        scope,
        lifecycle_state,
        archived,
        superseded_by,
        parent_id,
        source_node_id,
        content_source_conversation_id,
        content_source_node_id,
        grounding_source_conversation_id,
        grounding_source_node_id,
        source_episode_id,
        source_ref,
        source_time_ns,
        temporal_inference,
        mutation_id,
        created_at_ns,
        updated_at_ns,
        memory_version,
    })
}

pub(super) struct DecodedDelta {
    pub(super) id: MemoryId,
    pub(super) previous_revision: u64,
    pub(super) mask: u64,
    pub(super) record: MemoryRecord,
}
pub(super) fn decode_delta(bytes: &[u8]) -> Result<DecodedDelta, MemoryError> {
    let mut r = Reader::new(&bytes[8..]);
    let id = MemoryId(r.array()?);
    let previous_revision = r.u64()?;
    let revision = r.u64()?;
    let body_id = MemoryBodyId(r.array()?);
    let memory_version = r.u64()?;
    let mutation_id = r.string()?;
    let mask = r.u64()?;
    if mask & !KNOWN_DELTA_MASK != 0 {
        return Err(MemoryError::Corrupt("unknown Memory delta field"));
    }
    let mut record = MemoryRecord {
        id,
        revision,
        body_id,
        category: String::new(),
        memory_type: String::new(),
        authority_kind: String::new(),
        temporal_status: String::new(),
        scope: String::new(),
        lifecycle_state: String::new(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: None,
        source_time_ns: None,
        temporal_inference: None,
        mutation_id,
        created_at_ns: 0,
        updated_at_ns: 0,
        memory_version,
    };
    // Delta fields are applied to the accepted predecessor during replay.
    if mask & (1 << 0) != 0 {
        record.category = r.string()?;
    }
    if mask & (1 << 1) != 0 {
        record.memory_type = r.string()?;
    }
    if mask & (1 << 2) != 0 {
        record.authority_kind = r.string()?;
    }
    if mask & (1 << 3) != 0 {
        record.temporal_status = r.string()?;
    }
    if mask & (1 << 4) != 0 {
        record.scope = r.string()?;
    }
    if mask & (1 << 5) != 0 {
        record.lifecycle_state = r.string()?;
    }
    if mask & (1 << 6) != 0 {
        record.archived = r.bool()?;
    }
    if mask & (1 << 7) != 0 {
        record.superseded_by = r.option_id()?;
    }
    if mask & (1 << 8) != 0 {
        record.parent_id = r.option_id()?;
    }
    if mask & (1 << 9) != 0 {
        record.source_node_id = r.optional_string()?;
    }
    if mask & (1 << 10) != 0 {
        record.content_source_conversation_id = r.optional_string()?;
    }
    if mask & (1 << 11) != 0 {
        record.content_source_node_id = r.optional_string()?;
    }
    if mask & (1 << 12) != 0 {
        record.grounding_source_conversation_id = r.optional_string()?;
    }
    if mask & (1 << 13) != 0 {
        record.grounding_source_node_id = r.optional_string()?;
    }
    if mask & (1 << 14) != 0 {
        record.source_episode_id = r.option_array32()?;
    }
    if mask & (1 << 15) != 0 {
        record.source_time_ns = r.option_i64()?;
    }
    if mask & (1 << 16) != 0 {
        record.source_ref = r.source_ref()?;
    }
    if mask & (1 << 17) != 0 {
        record.temporal_inference = r.temporal_inference()?;
    }
    if mask & (1 << 18) != 0 {
        record.created_at_ns = r.i64()?;
    }
    if mask & (1 << 19) != 0 {
        record.updated_at_ns = r.i64()?;
    }
    r.finish()?;
    Ok(DecodedDelta {
        id,
        previous_revision,
        mask,
        record,
    })
}

fn write_record_identity(out: &mut Vec<u8>, record: &MemoryRecord) -> Result<(), MemoryError> {
    out.extend_from_slice(&record.id.0);
    put_u64(out, record.revision);
    out.extend_from_slice(&record.body_id.0);
    put_u64(out, record.memory_version);
    put_string(out, &record.mutation_id)
}
fn write_record_fields(out: &mut Vec<u8>, record: &MemoryRecord) -> Result<(), MemoryError> {
    put_string(out, &record.category)?;
    put_string(out, &record.memory_type)?;
    put_string(out, &record.authority_kind)?;
    put_string(out, &record.temporal_status)?;
    put_string(out, &record.scope)?;
    put_string(out, &record.lifecycle_state)?;
    out.push(u8::from(record.archived));
    put_option_id(out, record.superseded_by);
    put_option_id(out, record.parent_id);
    put_optional_string(out, record.source_node_id.as_deref())?;
    put_optional_string(out, record.content_source_conversation_id.as_deref())?;
    put_optional_string(out, record.content_source_node_id.as_deref())?;
    put_optional_string(out, record.grounding_source_conversation_id.as_deref())?;
    put_optional_string(out, record.grounding_source_node_id.as_deref())?;
    put_option_bytes32(out, record.source_episode_id);
    put_source_ref(out, record.source_ref.as_ref())?;
    put_option_i64(out, record.source_time_ns);
    put_temporal_inference(out, record.temporal_inference.as_ref())?;
    put_i64(out, record.created_at_ns);
    put_i64(out, record.updated_at_ns);
    Ok(())
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes())
}
fn put_i64(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_le_bytes())
}
fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), MemoryError> {
    let len = u32::try_from(value.len()).map_err(|_| MemoryError::FieldTooLarge)?;
    if value.len() > MAX_FIELD_BYTES {
        return Err(MemoryError::FieldTooLarge);
    }
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
fn put_option_id(out: &mut Vec<u8>, value: Option<MemoryId>) {
    match value {
        None => out.push(0),
        Some(id) => {
            out.push(1);
            out.extend_from_slice(&id.0)
        }
    }
}
fn put_option_i64(out: &mut Vec<u8>, value: Option<i64>) {
    match value {
        None => out.push(0),
        Some(n) => {
            out.push(1);
            put_i64(out, n)
        }
    }
}
fn put_option_bytes32(out: &mut Vec<u8>, value: Option<[u8; 32]>) {
    match value {
        None => out.push(0),
        Some(value) => {
            out.push(1);
            out.extend_from_slice(&value);
        }
    }
}
fn put_temporal_inference(
    out: &mut Vec<u8>,
    value: Option<&MemoryTemporalInference>,
) -> Result<(), MemoryError> {
    let Some(value) = value else {
        out.push(0);
        return Ok(());
    };
    out.push(1);
    out.extend_from_slice(&value.body_id.0);
    put_option_i64(out, value.source_time_ns);
    put_string(out, &value.inference.model)?;
    put_string(out, &value.inference.contract_version)?;
    let count =
        u32::try_from(value.inference.resolutions.len()).map_err(|_| MemoryError::FieldTooLarge)?;
    out.extend_from_slice(&count.to_le_bytes());
    for resolution in &value.inference.resolutions {
        put_u64(
            out,
            u64::try_from(resolution.start_byte).map_err(|_| MemoryError::FieldTooLarge)?,
        );
        put_u64(
            out,
            u64::try_from(resolution.end_byte).map_err(|_| MemoryError::FieldTooLarge)?,
        );
        out.push(match resolution.kind {
            TemporalIndicationKind::Explicit => 0,
            TemporalIndicationKind::Calendar => 1,
            TemporalIndicationKind::Relative => 2,
            TemporalIndicationKind::Boundary => 3,
            TemporalIndicationKind::Recurrence => 4,
            TemporalIndicationKind::Duration => 5,
        });
        put_string(out, &resolution.evidence)?;
        put_string(out, &resolution.canonical_expression)?;
    }
    Ok(())
}
fn put_source_ref(out: &mut Vec<u8>, value: Option<&MemorySourceRef>) -> Result<(), MemoryError> {
    let Some(value) = value else {
        out.push(0);
        return Ok(());
    };
    out.push(1);
    put_string(out, &value.owner_id)?;
    put_optional_string(out, value.principal_id.as_deref())?;
    out.extend_from_slice(&value.source_episode_id);
    put_string(out, &value.source_node_id)?;
    put_optional_string(out, value.content_source_conversation_id.as_deref())?;
    put_optional_string(out, value.content_source_node_id.as_deref())?;
    put_optional_string(out, value.grounding_source_conversation_id.as_deref())?;
    put_optional_string(out, value.grounding_source_node_id.as_deref())?;
    Ok(())
}
fn put_optional_string(out: &mut Vec<u8>, value: Option<&str>) -> Result<(), MemoryError> {
    match value {
        None => out.push(0),
        Some(value) => {
            out.push(1);
            put_string(out, value)?
        }
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], MemoryError> {
        let end = self
            .offset
            .checked_add(n)
            .ok_or(MemoryError::Corrupt("offset overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(MemoryError::Corrupt("truncated field"))?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], MemoryError> {
        Ok(self.take(N)?.try_into().unwrap())
    }
    fn u32(&mut self) -> Result<u32, MemoryError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, MemoryError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn i64(&mut self) -> Result<i64, MemoryError> {
        Ok(i64::from_le_bytes(self.array()?))
    }
    fn bool(&mut self) -> Result<bool, MemoryError> {
        match self.take(1)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MemoryError::Corrupt("invalid boolean")),
        }
    }
    fn string(&mut self) -> Result<String, MemoryError> {
        let len = self.u32()? as usize;
        if len > MAX_FIELD_BYTES {
            return Err(MemoryError::FieldTooLarge);
        }
        String::from_utf8(self.take(len)?.to_vec())
            .map_err(|_| MemoryError::Corrupt("invalid UTF-8"))
    }
    fn string_u64(&mut self) -> Result<String, MemoryError> {
        let len = usize::try_from(self.u64()?).map_err(|_| MemoryError::FieldTooLarge)?;
        if len > MAX_BODY_BYTES {
            return Err(MemoryError::FieldTooLarge);
        }
        String::from_utf8(self.take(len)?.to_vec())
            .map_err(|_| MemoryError::Corrupt("invalid UTF-8"))
    }
    fn option_id(&mut self) -> Result<Option<MemoryId>, MemoryError> {
        match self.take(1)?[0] {
            0 => Ok(None),
            1 => Ok(Some(MemoryId(self.array()?))),
            _ => Err(MemoryError::Corrupt("invalid optional identity tag")),
        }
    }
    fn option_i64(&mut self) -> Result<Option<i64>, MemoryError> {
        match self.take(1)?[0] {
            0 => Ok(None),
            1 => Ok(Some(self.i64()?)),
            _ => Err(MemoryError::Corrupt("invalid optional timestamp tag")),
        }
    }
    fn option_array32(&mut self) -> Result<Option<[u8; 32]>, MemoryError> {
        match self.take(1)?[0] {
            0 => Ok(None),
            1 => Ok(Some(self.array()?)),
            _ => Err(MemoryError::Corrupt(
                "invalid optional 32-byte identity tag",
            )),
        }
    }
    fn temporal_inference(&mut self) -> Result<Option<MemoryTemporalInference>, MemoryError> {
        if self.take(1)?[0] == 0 {
            return Ok(None);
        }
        let body_id = MemoryBodyId(self.array()?);
        let source_time_ns = self.option_i64()?;
        let model = self.string()?;
        let contract_version = self.string()?;
        let count = self.u32()? as usize;
        if count > 1_000_000 {
            return Err(MemoryError::FieldTooLarge);
        }
        let mut resolutions = Vec::with_capacity(count.min(4096));
        for _ in 0..count {
            let start_byte =
                usize::try_from(self.u64()?).map_err(|_| MemoryError::FieldTooLarge)?;
            let end_byte = usize::try_from(self.u64()?).map_err(|_| MemoryError::FieldTooLarge)?;
            let kind = match self.take(1)?[0] {
                0 => TemporalIndicationKind::Explicit,
                1 => TemporalIndicationKind::Calendar,
                2 => TemporalIndicationKind::Relative,
                3 => TemporalIndicationKind::Boundary,
                4 => TemporalIndicationKind::Recurrence,
                5 => TemporalIndicationKind::Duration,
                _ => return Err(MemoryError::Corrupt("unknown temporal indication kind")),
            };
            resolutions.push(TemporalInferenceResolution {
                start_byte,
                end_byte,
                kind,
                evidence: self.string()?,
                canonical_expression: self.string()?,
            });
        }
        Ok(Some(MemoryTemporalInference {
            body_id,
            source_time_ns,
            inference: TemporalInference {
                model,
                contract_version,
                resolutions,
            },
        }))
    }
    fn optional_string(&mut self) -> Result<Option<String>, MemoryError> {
        match self.take(1)?[0] {
            0 => Ok(None),
            1 => Ok(Some(self.string()?)),
            _ => Err(MemoryError::Corrupt("invalid optional string tag")),
        }
    }
    fn source_ref(&mut self) -> Result<Option<MemorySourceRef>, MemoryError> {
        if self.take(1)?[0] == 0 {
            return Ok(None);
        }
        let owner_id = self.string()?;
        let principal_id = self.optional_string()?;
        let source_episode_id = self.array()?;
        let source_node_id = self.string()?;
        let content_source_conversation_id = self.optional_string()?;
        let content_source_node_id = self.optional_string()?;
        let grounding_source_conversation_id = self.optional_string()?;
        let grounding_source_node_id = self.optional_string()?;
        let source = MemorySourceRef {
            owner_id,
            principal_id,
            source_episode_id,
            source_node_id,
            content_source_conversation_id,
            content_source_node_id,
            grounding_source_conversation_id,
            grounding_source_node_id,
        };
        validate_source_ref(&source)?;
        Ok(Some(source))
    }
    fn finish(&self) -> Result<(), MemoryError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(MemoryError::Corrupt("trailing bytes"))
        }
    }
}

pub(super) fn decode_tombstone(bytes: &[u8]) -> Result<(MemoryId, u64, u64), MemoryError> {
    let mut r = Reader::new(&bytes[8..]);
    let id = MemoryId(r.array()?);
    let revision = r.u64()?;
    let version = r.u64()?;
    r.finish()?;
    Ok((id, revision, version))
}
