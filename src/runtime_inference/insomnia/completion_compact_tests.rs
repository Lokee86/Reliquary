use super::completion::{decode_completion, decode_embedded_version_range, encode_completion};
use super::completion_tests::process;
use crate::memory_compact_codec::decode_creation;
use crate::{Cva, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, InsomniaPriority, MemoryId};
use std::fs;
use std::path::{Path, PathBuf};

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "insomnia-compact-completion-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn queued_owner(path: &Path) -> (Cva, crate::Episode) {
    let mut cva = Cva::create(path).unwrap();
    let mut parent = None;
    for index in 0..8 {
        let user = format!("u{index}");
        let assistant = format!("a{index}");
        cva.append_node(
            user.clone(),
            "completion-c".into(),
            parent.clone(),
            "user".into(),
            10 + index as i64 * 2,
            "compact completion input",
        )
        .unwrap();
        cva.append_node(
            assistant.clone(),
            "completion-c".into(),
            Some(user),
            "assistant".into(),
            11 + index as i64 * 2,
            "acknowledged",
        )
        .unwrap();
        parent = Some(assistant);
    }
    let episode = cva
        .materialize_path_episodes(
            "completion-c",
            parent.as_deref().unwrap(),
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::Inactivity, 100)),
        )
        .unwrap()
        .created
        .remove(0);
    cva.queue_insomnia_episode(episode.id, InsomniaPriority::Live, 101)
        .unwrap();
    (cva, episode)
}

pub(super) fn completion_payload(cva: &mut Cva) -> Vec<u8> {
    cva.container
        .chunks()
        .unwrap()
        .into_iter()
        .map(|chunk| cva.container.read(chunk).unwrap())
        .find(|payload| payload.starts_with(b"CVAINSC7"))
        .expect("compact CVAINSC7 completion payload")
}

fn occurrence_offsets(bytes: &[u8], needle: &[u8]) -> Vec<usize> {
    bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == needle).then_some(offset))
        .collect()
}

#[test]
fn grouped_completion_v7_embeds_compact_creation_and_roundtrips_birth_proof() {
    let scratch = ScratchDirectory::new();
    let path = scratch.file("compact-birth.cva");
    let (mut cva, episode) = queued_owner(&path);
    let result = process(&mut cva, 1, 110);
    assert_eq!(result.created.len(), 1);

    let payload = completion_payload(&mut cva);
    assert_eq!(&payload[..8], b"CVAINSC7");
    assert_eq!(payload[84], 1, "one accepted birth proof is present");
    let decoded = decode_completion(&payload).unwrap().unwrap();
    assert_eq!(decoded.episode_id, episode.id);
    assert_eq!(decoded.records.len(), 1);
    assert_eq!(decoded.records[0].revision, 1);
    assert_eq!(decoded.records[0].id, result.created[0].id);
    assert_eq!(
        decoded.freshness_birth_memory_ids,
        vec![result.created[0].id]
    );
    assert_eq!(decoded.freshness_birth_turn, Some(cva.rel_turn_count()));

    let compact_offsets = occurrence_offsets(&payload, b"CVAMC001");
    assert_eq!(compact_offsets.len(), 1);
    let carrier_offset = compact_offsets[0];
    let carrier_len = u32::from_le_bytes(
        payload[carrier_offset - 4..carrier_offset]
            .try_into()
            .unwrap(),
    ) as usize;
    let creation = decode_creation(&payload[carrier_offset..carrier_offset + carrier_len]).unwrap();
    assert_eq!(creation.record, decoded.records[0]);
    assert!(occurrence_offsets(&payload, b"CVAMD001").is_empty());
    for version in 2..=8 {
        assert!(occurrence_offsets(&payload, format!("CVAMEMR{version}").as_bytes()).is_empty());
    }
    // The accepted completion record is decoded from its compact creation,
    // including its envelope coordinates, rather than an embedded full record.
    assert_eq!(
        decoded.records[0].global_version,
        result.created[0].global_version
    );
    assert_eq!(
        decoded.records[0].memory_version,
        result.created[0].memory_version
    );
}

#[test]
fn grouped_completion_v7_encodes_absent_birth_proof_and_rejects_bad_flag_or_carriers() {
    let scratch = ScratchDirectory::new();
    let path = scratch.file("compact-empty.cva");
    let (mut cva, _episode) = queued_owner(&path);
    let result = process(&mut cva, 0, 110);
    assert!(result.created.is_empty());
    let actual_payload = completion_payload(&mut cva);
    let mut without_birth_proof = decode_completion(&actual_payload).unwrap().unwrap();
    without_birth_proof.freshness_birth_turn = None;
    without_birth_proof.freshness_policy_version = None;
    without_birth_proof.freshness_birth_memory_ids.clear();
    let payload = encode_completion(&without_birth_proof).unwrap();
    assert_eq!(payload[84], 0);
    let decoded = decode_completion(&payload).unwrap().unwrap();
    assert!(decoded.records.is_empty());
    assert!(decoded.freshness_birth_turn.is_none());
    assert!(decoded.freshness_policy_version.is_none());
    assert!(decoded.freshness_birth_memory_ids.is_empty());

    let mut invalid_flag = payload.clone();
    invalid_flag[84] = 2;
    assert!(decode_completion(&invalid_flag).is_err());

    let scratch_with_birth = ScratchDirectory::new();
    let birth_path = scratch_with_birth.file("compact-malformed.cva");
    let (mut birth_cva, _episode) = queued_owner(&birth_path);
    process(&mut birth_cva, 1, 110);
    let birth_payload = completion_payload(&mut birth_cva);
    let carrier_offset = occurrence_offsets(&birth_payload, b"CVAMC001")[0];

    let mut malformed_creation = birth_payload.clone();
    malformed_creation[carrier_offset] ^= 1;
    assert!(decode_completion(&malformed_creation).is_err());

    // A delta is structurally meaningful in the owner codec, but a grouped
    // completion accepts only first-revision compact creation carriers.
    let mut delta_carrier = birth_payload;
    delta_carrier[carrier_offset..carrier_offset + 8].copy_from_slice(b"CVAMD001");
    assert!(decode_completion(&delta_carrier).is_err());
}

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

#[test]
fn compact_completion_records_keep_their_exact_memory_identity() {
    let scratch = ScratchDirectory::new();
    let path = scratch.file("compact-identity.cva");
    let (mut cva, _episode) = queued_owner(&path);
    let result = process(&mut cva, 2, 110);
    assert_eq!(result.created.len(), 2);
    let payload = completion_payload(&mut cva);
    let decoded = decode_completion(&payload).unwrap().unwrap();
    let returned_ids: Vec<MemoryId> = decoded.records.iter().map(|record| record.id).collect();
    assert_eq!(
        returned_ids,
        result
            .created
            .iter()
            .map(|memory| memory.id)
            .collect::<Vec<_>>()
    );
    assert!(decoded.records.iter().all(|record| record.revision == 1));
    assert!(
        decoded
            .records
            .iter()
            .all(|record| record.global_version > 0)
    );
    assert!(
        decoded
            .records
            .iter()
            .all(|record| record.memory_version > 0)
    );
}
