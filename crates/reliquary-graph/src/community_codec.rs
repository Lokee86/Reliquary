use crate::{
    Community, CommunityError, CommunityId, CommunitySemanticName, CommunitySemanticNameSource,
    CommunitySnapshot, MemoryId,
};

const SNAPSHOT_MAGIC: &[u8; 8] = b"CVACOMM1";
const SNAPSHOT_SCHEMA: u32 = 1;
const HEADER_LEN: usize = 60;
const COMMUNITY_HEADER_LEN: usize = 36;
const NAME_MAGIC: &[u8; 8] = b"CVACNAM1";
const NAME_SCHEMA: u32 = 3;
const NAME_HEADER_V1_LEN: usize = 52;
const NAME_HEADER_V2_LEN: usize = 53;
const NAME_HEADER_V3_LEN: usize = 85;

pub fn encode_snapshot(snapshot: &CommunitySnapshot) -> Result<Vec<u8>, CommunityError> {
    let member_count = snapshot
        .communities
        .iter()
        .try_fold(0_usize, |total, community| {
            total.checked_add(community.members.len())
        })
        .ok_or(CommunityError::SizeOverflow)?;
    let member_bytes = member_count
        .checked_mul(32)
        .ok_or(CommunityError::SizeOverflow)?;
    let community_bytes = snapshot
        .communities
        .len()
        .checked_mul(COMMUNITY_HEADER_LEN)
        .ok_or(CommunityError::SizeOverflow)?;
    let capacity = HEADER_LEN
        .checked_add(community_bytes)
        .and_then(|value| value.checked_add(member_bytes))
        .ok_or(CommunityError::SizeOverflow)?;
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(SNAPSHOT_MAGIC);
    out.extend_from_slice(&SNAPSHOT_SCHEMA.to_le_bytes());
    out.extend_from_slice(&snapshot.generation.to_le_bytes());
    out.extend_from_slice(&snapshot.derived_graph_version.to_le_bytes());
    out.extend_from_slice(&snapshot.algorithm_version.to_le_bytes());
    out.extend_from_slice(&snapshot.seed.to_le_bytes());
    out.extend_from_slice(&snapshot.resolution.to_le_bytes());
    out.extend_from_slice(&snapshot.quality.to_le_bytes());
    let count =
        u32::try_from(snapshot.communities.len()).map_err(|_| CommunityError::SizeOverflow)?;
    out.extend_from_slice(&count.to_le_bytes());
    for community in &snapshot.communities {
        out.extend_from_slice(&community.id.0);
        let members =
            u32::try_from(community.members.len()).map_err(|_| CommunityError::SizeOverflow)?;
        out.extend_from_slice(&members.to_le_bytes());
        for member in &community.members {
            out.extend_from_slice(member);
        }
    }
    Ok(out)
}

pub fn decode_snapshot(bytes: &[u8]) -> Result<Option<CommunitySnapshot>, CommunityError> {
    if !bytes.starts_with(SNAPSHOT_MAGIC) {
        return Ok(None);
    }
    if bytes.len() < HEADER_LEN || read_u32(bytes, 8)? != SNAPSHOT_SCHEMA {
        return Err(CommunityError::CorruptRecord("snapshot header"));
    }
    let generation = read_u64(bytes, 12)?;
    let derived_graph_version = read_u64(bytes, 20)?;
    let algorithm_version = read_u32(bytes, 28)?;
    let seed = read_u64(bytes, 32)?;
    let resolution = read_f64(bytes, 40)?;
    let quality = read_f64(bytes, 48)?;
    let count = usize::try_from(read_u32(bytes, 56)?).map_err(|_| CommunityError::SizeOverflow)?;
    let mut offset = HEADER_LEN;
    let mut communities = Vec::with_capacity(count);
    for _ in 0..count {
        let header = bytes
            .get(offset..offset + COMMUNITY_HEADER_LEN)
            .ok_or(CommunityError::CorruptRecord("community header"))?;
        let id = CommunityId(header[..32].try_into().expect("community id width"));
        let member_count = usize::try_from(u32::from_le_bytes(header[32..36].try_into().unwrap()))
            .map_err(|_| CommunityError::SizeOverflow)?;
        offset = offset
            .checked_add(COMMUNITY_HEADER_LEN)
            .ok_or(CommunityError::SizeOverflow)?;
        let member_bytes = member_count
            .checked_mul(32)
            .ok_or(CommunityError::SizeOverflow)?;
        let end = offset
            .checked_add(member_bytes)
            .ok_or(CommunityError::SizeOverflow)?;
        let raw = bytes
            .get(offset..end)
            .ok_or(CommunityError::CorruptRecord("community members"))?;
        let members = raw
            .chunks_exact(32)
            .map(|chunk| chunk.try_into().expect("memory id width"))
            .collect();
        communities.push(Community { id, members });
        offset = end;
    }
    if offset != bytes.len() {
        return Err(CommunityError::CorruptRecord("snapshot trailing bytes"));
    }
    Ok(Some(CommunitySnapshot {
        generation,
        derived_graph_version,
        algorithm_version,
        seed,
        resolution,
        quality,
        communities,
    }))
}

pub fn encode_semantic_name(record: &CommunitySemanticName) -> Result<Vec<u8>, CommunityError> {
    let representative_bytes = record
        .representative_memories
        .len()
        .checked_mul(32)
        .ok_or(CommunityError::SizeOverflow)?;
    let capacity = NAME_HEADER_V3_LEN
        .checked_add(representative_bytes)
        .and_then(|value| value.checked_add(4))
        .and_then(|value| value.checked_add(record.name.len()))
        .ok_or(CommunityError::SizeOverflow)?;
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(NAME_MAGIC);
    out.extend_from_slice(&NAME_SCHEMA.to_le_bytes());
    out.extend_from_slice(&record.contract_version.to_le_bytes());
    out.push(match record.source {
        CommunitySemanticNameSource::Dream => 1,
        CommunitySemanticNameSource::User => 2,
    });
    out.extend_from_slice(&record.community_id.0);
    out.extend_from_slice(&record.baseline_community_id.0);
    let representative_count = u32::try_from(record.representative_memories.len())
        .map_err(|_| CommunityError::SizeOverflow)?;
    out.extend_from_slice(&representative_count.to_le_bytes());
    for memory_id in &record.representative_memories {
        out.extend_from_slice(memory_id);
    }
    let name_len = u32::try_from(record.name.len()).map_err(|_| CommunityError::SizeOverflow)?;
    out.extend_from_slice(&name_len.to_le_bytes());
    out.extend_from_slice(record.name.as_bytes());
    Ok(out)
}

pub fn decode_semantic_name(bytes: &[u8]) -> Result<Option<CommunitySemanticName>, CommunityError> {
    if !bytes.starts_with(NAME_MAGIC) {
        return Ok(None);
    }
    let schema = read_u32(bytes, 8)?;
    let (header_len, source, community_start, baseline_start, count_offset) = match schema {
        1 => (
            NAME_HEADER_V1_LEN,
            CommunitySemanticNameSource::Dream,
            16,
            None,
            48,
        ),
        2 => {
            let source = decode_name_source(bytes)?;
            (NAME_HEADER_V2_LEN, source, 17, None, 49)
        }
        3 => {
            let source = decode_name_source(bytes)?;
            (NAME_HEADER_V3_LEN, source, 17, Some(49), 81)
        }
        _ => return Err(CommunityError::CorruptRecord("semantic name header")),
    };
    if bytes.len() < header_len {
        return Err(CommunityError::CorruptRecord("semantic name header"));
    }
    let contract_version = read_u32(bytes, 12)?;
    let community_id = CommunityId(
        bytes
            .get(community_start..community_start + 32)
            .ok_or(CommunityError::CorruptRecord("semantic name community id"))?
            .try_into()
            .expect("community id width"),
    );
    let baseline_community_id = baseline_start.map_or(community_id, |offset| {
        CommunityId(
            bytes
                .get(offset..offset + 32)
                .expect("validated semantic name header width")
                .try_into()
                .expect("community id width"),
        )
    });
    let representative_count = usize::try_from(read_u32(bytes, count_offset)?)
        .map_err(|_| CommunityError::SizeOverflow)?;
    let representative_bytes = representative_count
        .checked_mul(32)
        .ok_or(CommunityError::SizeOverflow)?;
    let representatives_end = header_len
        .checked_add(representative_bytes)
        .ok_or(CommunityError::SizeOverflow)?;
    let raw_representatives =
        bytes
            .get(header_len..representatives_end)
            .ok_or(CommunityError::CorruptRecord(
                "semantic name representatives",
            ))?;
    let representative_memories = raw_representatives
        .chunks_exact(32)
        .map(|chunk| chunk.try_into().expect("memory id width"))
        .collect();
    let name_len = usize::try_from(read_u32(bytes, representatives_end)?)
        .map_err(|_| CommunityError::SizeOverflow)?;
    let name_start = representatives_end
        .checked_add(4)
        .ok_or(CommunityError::SizeOverflow)?;
    let name_end = name_start
        .checked_add(name_len)
        .ok_or(CommunityError::SizeOverflow)?;
    let name = std::str::from_utf8(
        bytes
            .get(name_start..name_end)
            .ok_or(CommunityError::CorruptRecord("semantic name text"))?,
    )
    .map_err(|_| CommunityError::CorruptRecord("semantic name utf-8"))?
    .to_owned();
    if name_end != bytes.len() {
        return Err(CommunityError::CorruptRecord(
            "semantic name trailing bytes",
        ));
    }
    Ok(Some(CommunitySemanticName {
        community_id,
        baseline_community_id,
        contract_version,
        source,
        name,
        representative_memories,
    }))
}

fn decode_name_source(bytes: &[u8]) -> Result<CommunitySemanticNameSource, CommunityError> {
    match *bytes
        .get(16)
        .ok_or(CommunityError::CorruptRecord("semantic name source"))?
    {
        1 => Ok(CommunitySemanticNameSource::Dream),
        2 => Ok(CommunitySemanticNameSource::User),
        _ => Err(CommunityError::CorruptRecord("semantic name source")),
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, CommunityError> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or(CommunityError::CorruptRecord("u32"))?;
    Ok(u32::from_le_bytes(raw.try_into().expect("u32 width")))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, CommunityError> {
    let raw = bytes
        .get(offset..offset + 8)
        .ok_or(CommunityError::CorruptRecord("u64"))?;
    Ok(u64::from_le_bytes(raw.try_into().expect("u64 width")))
}

fn read_f64(bytes: &[u8], offset: usize) -> Result<f64, CommunityError> {
    let raw = bytes
        .get(offset..offset + 8)
        .ok_or(CommunityError::CorruptRecord("f64"))?;
    Ok(f64::from_le_bytes(raw.try_into().expect("f64 width")))
}
