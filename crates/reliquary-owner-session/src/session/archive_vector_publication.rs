//! Bounded source reads and owner-authorized Archive-vector publication.

use super::*;
use reliquary_archive::{FragmentConfig, FragmentDefinition};
use reliquary_vector_store::{
    AcceptedArchiveMapping, ArchiveFragmentId, CompatibilityProfileId, VectorGeneration,
    VectorNormalization,
};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_ARCHIVE_VECTOR_BATCH_ROWS: usize = 64;
pub const MAX_ARCHIVE_FRAGMENT_TEXT_BYTES: usize = 256 << 10;
pub const MAX_ARCHIVE_VECTOR_RESPONSE_BYTES: usize = 8 << 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveFragmentVersion {
    pub id: ArchiveFragmentId,
    pub max_archive_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveVectorSource {
    pub fragment_id: ArchiveFragmentId,
    pub max_archive_version: u64,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArchiveVectorRow {
    pub fragment_id: ArchiveFragmentId,
    pub max_archive_version: u64,
    pub vector: Vec<f32>,
}

/// Immutable owner snapshot for one bounded embedding batch. Existing rows
/// are the accepted current generation's canonical prefix and remain exact.
#[derive(Clone, Debug)]
pub struct ArchiveVectorInput {
    owner_id: [u8; 16],
    profile: CompatibilityProfile,
    source_archive_version: u64,
    fragments: Vec<ArchiveFragmentVersion>,
    sources: Vec<ArchiveVectorSource>,
    existing_rows: Vec<ArchiveVectorRow>,
    current_generation: Option<VectorGeneration>,
    existing_max_fragment_archive_version: u64,
    complete_current_generation: bool,
}

impl ArchiveVectorInput {
    pub fn owner_id(&self) -> [u8; 16] {
        self.owner_id
    }
    pub fn profile(&self) -> &CompatibilityProfile {
        &self.profile
    }
    pub fn source_archive_version(&self) -> u64 {
        self.source_archive_version
    }
    pub fn fragments(&self) -> &[ArchiveFragmentVersion] {
        &self.fragments
    }
    pub fn sources(&self) -> &[ArchiveVectorSource] {
        &self.sources
    }
    pub fn existing_rows(&self) -> &[ArchiveVectorRow] {
        &self.existing_rows
    }
    pub fn current_generation(&self) -> Option<VectorGeneration> {
        self.current_generation
    }
    pub fn complete_current_generation(&self) -> bool {
        self.complete_current_generation
    }
}

impl RelWriter {
    pub fn archive_vector_input(
        &mut self,
        profile_id: CompatibilityProfileId,
    ) -> Result<ArchiveVectorInput, OwnerSessionError> {
        self.ensure_write_ready()?;
        check_archive_vector_lease(self)?;
        if self
            .session
            .common
            .profiles
            .get_active(profile_id)
            .is_none()
        {
            return Err(vector_invalid(
                "Archive vector profile is missing or retired",
            ));
        }
        let result = (|| {
            ensure_fragment_definitions(&mut self.session.archive, &mut self.container)?;
            archive_vector_input(
                self.session.owner_id,
                &self.session.archive,
                &self.session.common.packed_vectors,
                &self.session.common.profiles,
                &self.session.archive_vectors,
                &self.session.generations,
                profile_id,
            )
        })();
        finish_archive_vector_write(self, result)
    }

    /// Publish one bounded batch as an immutable, FragmentId-ordered union
    /// and activate it through the existing VectorGeneration records.
    pub fn publish_archive_vector_batch(
        &mut self,
        owner_id: [u8; 16],
        profile_id: CompatibilityProfileId,
        source_archive_version: u64,
        rows: &[ArchiveVectorRow],
    ) -> Result<Option<VectorGeneration>, OwnerSessionError> {
        self.ensure_write_ready()?;
        check_archive_vector_lease(self)?;
        let result = publish_archive_vector_batch(
            &mut self.session,
            &mut self.container,
            owner_id,
            profile_id,
            source_archive_version,
            rows,
        );
        finish_archive_vector_write(self, result)
    }
}

pub(super) fn archive_vector_input(
    owner_id: [u8; 16],
    archive: &Archive,
    packed: &PackedVectorStore,
    profiles: &CompatibilityProfileStore,
    archives: &ArchiveVectorStore,
    generations: &VectorGenerationStore,
    profile_id: CompatibilityProfileId,
) -> Result<ArchiveVectorInput, OwnerSessionError> {
    let profile = profiles
        .get_active(profile_id)
        .ok_or_else(|| vector_invalid("Archive vector profile is missing or retired"))?
        .clone();
    let source_archive_version = archive.archive_version();
    let fragments = archive_fragment_versions(archive)?;
    if fragments.is_empty() {
        return Err(vector_invalid("Archive contains no source fragments"));
    }

    let current_generation = generations.current_generation(profile_id);
    let mut existing_rows = BTreeMap::new();
    let mut existing_max_fragment_archive_version = 0;
    if let Some(generation) = current_generation {
        if generation.source_archive_version > source_archive_version {
            return Err(vector_invalid(
                "Archive vector generation is from the future",
            ));
        }
        let binding = archives.get(generation.archive_vector_id)?;
        let info = archives
            .info(generation.archive_vector_id)
            .ok_or_else(|| vector_invalid("Archive vector binding is unavailable"))?;
        let packed_info = packed
            .info(binding.packed_vector_id)
            .ok_or_else(|| vector_invalid("Archive vector matrix is unavailable"))?;
        if packed_info.schema.dimensions != profile.dimensions
            || packed_info.count != info.rows
            || binding.fragment_ids.len() != usize::try_from(info.rows).unwrap_or(usize::MAX)
        {
            return Err(vector_invalid(
                "current Archive vector rows do not match profile",
            ));
        }
        existing_max_fragment_archive_version = info.max_fragment_archive_version;
        let mut verified_max = 0;
        for (row, fragment_id) in binding.fragment_ids.into_iter().enumerate() {
            let definition = archive
                .fragment_definition(reliquary_archive::FragmentId(fragment_id.0))
                .ok_or_else(|| {
                    vector_invalid("accepted vector has no Archive fragment definition")
                })?;
            let source = &definition.fragment;
            let version = archive
                .node_archive_version(&source.conversation_id, &source.end_node_id)
                .ok_or_else(|| vector_invalid("accepted vector has no Archive source version"))?;
            if version > generation.source_archive_version {
                return Err(vector_invalid(
                    "accepted vector source is newer than its generation",
                ));
            }
            verified_max = verified_max.max(version);
            if existing_rows.contains_key(&fragment_id) {
                return Err(vector_invalid(
                    "current Archive vector binding repeats a fragment",
                ));
            }
            let vector = packed.read_f32_row(binding.packed_vector_id, row as u64)?;
            validate_archive_vector_row(&profile, &vector)?;
            existing_rows.insert(
                fragment_id,
                ArchiveVectorRow {
                    fragment_id,
                    max_archive_version: version,
                    vector,
                },
            );
        }
        if verified_max != existing_max_fragment_archive_version {
            return Err(vector_invalid(
                "accepted vector source version does not match its mapping",
            ));
        }
    }

    let current_ids = fragments
        .iter()
        .map(|(fragment, _)| fragment.id)
        .collect::<BTreeSet<_>>();
    let missing = fragments
        .iter()
        .filter(|(fragment, _)| !existing_rows.contains_key(&fragment.id))
        .cloned()
        .collect::<Vec<_>>();
    let sources = missing
        .into_iter()
        .map(|(fragment, definition)| {
            let text = render_definition(archive, &definition)?;
            if text.len() > MAX_ARCHIVE_FRAGMENT_TEXT_BYTES {
                return Err(vector_invalid("Archive fragment exceeds source byte limit"));
            }
            Ok(ArchiveVectorSource {
                fragment_id: fragment.id,
                max_archive_version: fragment.max_archive_version,
                text,
            })
        })
        .collect::<Result<Vec<_>, OwnerSessionError>>()?;
    let complete_current_generation = current_generation.is_some_and(|generation| {
        generation.source_archive_version == source_archive_version
            && current_ids.iter().all(|id| existing_rows.contains_key(id))
    });

    Ok(ArchiveVectorInput {
        owner_id,
        profile,
        source_archive_version,
        fragments: fragments.into_iter().map(|(version, _)| version).collect(),
        sources,
        existing_rows: existing_rows.into_values().collect(),
        current_generation,
        existing_max_fragment_archive_version,
        complete_current_generation,
    })
}

fn ensure_fragment_definitions(
    archive: &mut Archive,
    container: &mut Container,
) -> Result<(), OwnerSessionError> {
    let mut branches = archive.branches();
    branches.sort_by(|left, right| {
        left.conversation_id
            .cmp(&right.conversation_id)
            .then_with(|| left.id.cmp(&right.id))
    });
    for branch in branches {
        archive
            .define_branch_fragments(
                container,
                &branch.conversation_id,
                &branch.id,
                FragmentConfig::default(),
                true,
            )
            .map_err(|error| OwnerSessionError::Record(error.to_string()))?;
    }
    Ok(())
}

fn archive_fragment_versions(
    archive: &Archive,
) -> Result<Vec<(ArchiveFragmentVersion, FragmentDefinition)>, OwnerSessionError> {
    let mut branches = archive.branches();
    branches.sort_by(|left, right| {
        left.conversation_id
            .cmp(&right.conversation_id)
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut definitions =
        BTreeMap::<ArchiveFragmentId, (ArchiveFragmentVersion, FragmentDefinition)>::new();
    for branch in branches {
        let fragments = archive
            .branch_fragment_windows(
                &branch.conversation_id,
                &branch.id,
                FragmentConfig::default(),
                true,
            )
            .map_err(|error| OwnerSessionError::Record(error.to_string()))?;
        for fragment in fragments {
            let definition = archive
                .fragment_definition(fragment.id)
                .ok_or_else(|| vector_invalid("Archive fragment definition is unavailable"))?
                .clone();
            if definition.fragment != fragment {
                return Err(vector_invalid(
                    "Archive fragment definition conflicts with branch",
                ));
            }
            // The end node is the last accepted node on this immutable path.
            let max_archive_version = archive
                .node_archive_version(&fragment.conversation_id, &fragment.end_node_id)
                .ok_or_else(|| vector_invalid("fragment source has no Archive version"))?;
            let id = ArchiveFragmentId(fragment.id.0);
            let pair = (
                ArchiveFragmentVersion {
                    id,
                    max_archive_version,
                },
                definition,
            );
            if let Some(prior) = definitions.insert(id, pair.clone())
                && prior != pair
            {
                return Err(vector_invalid("conflicting Archive fragment source"));
            }
        }
    }
    Ok(definitions.into_values().collect())
}

fn render_definition(
    archive: &Archive,
    definition: &FragmentDefinition,
) -> Result<String, OwnerSessionError> {
    let fragment = &definition.fragment;
    let turns = archive.bounded_conversation_range(
        &fragment.conversation_id,
        &fragment.start_node_id,
        &fragment.end_node_id,
        reliquary_archive::MAX_EVIDENCE_TURNS,
        MAX_ARCHIVE_FRAGMENT_TEXT_BYTES.min(reliquary_archive::MAX_EVIDENCE_RESULT_BYTES),
    )?;
    Ok(turns
        .iter()
        .map(|turn| format!("{}: {}", turn.role, turn.content))
        .collect::<Vec<_>>()
        .join("\n"))
}

fn publish_archive_vector_batch(
    session: &mut RelSession,
    container: &mut Container,
    owner_id: [u8; 16],
    profile_id: CompatibilityProfileId,
    source_archive_version: u64,
    rows: &[ArchiveVectorRow],
) -> Result<Option<VectorGeneration>, OwnerSessionError> {
    if owner_id != container.owner_id() || owner_id != session.owner_id {
        return Err(vector_invalid("Archive vector owner mismatch"));
    }

    let input = archive_vector_input(
        owner_id,
        &session.archive,
        &session.common.packed_vectors,
        &session.common.profiles,
        &session.archive_vectors,
        &session.generations,
        profile_id,
    )?;
    if source_archive_version != input.source_archive_version {
        return Err(vector_invalid("Archive vector source cut changed"));
    }
    let expected = input
        .sources
        .iter()
        .map(|source| (source.fragment_id, source.max_archive_version))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for row in rows {
        if !seen.insert(row.fragment_id) {
            return Err(vector_invalid("duplicate Archive vector source"));
        }
        if expected.get(&row.fragment_id) != Some(&row.max_archive_version) {
            // Repeated successful publication may now classify these rows as
            // already present; the accepted bytes must match exactly.
            let existing = input
                .existing_rows
                .iter()
                .find(|prior| prior.fragment_id == row.fragment_id);
            if expected.contains_key(&row.fragment_id)
                || existing.is_none_or(|prior| {
                    prior.max_archive_version != row.max_archive_version
                        || !same_vector(&prior.vector, &row.vector)
                })
            {
                return Err(vector_invalid("Archive vector row is not source-bound"));
            }
        }
        validate_archive_vector_row(&input.profile, &row.vector)?;
    }
    if expected.keys().any(|id| !seen.contains(id)) {
        return Err(vector_invalid(
            "Archive vector batch omits a prepared source",
        ));
    }
    let mut union = input
        .existing_rows
        .into_iter()
        .map(|row| (row.fragment_id, row.vector))
        .collect::<BTreeMap<_, _>>();
    for row in rows {
        if let Some(prior) = union.get(&row.fragment_id) {
            if !same_vector(prior, &row.vector) {
                return Err(vector_invalid(
                    "Archive vector binding conflicts with prior row",
                ));
            }
        } else {
            union.insert(row.fragment_id, row.vector.clone());
        }
    }
    if union.is_empty() {
        return Err(vector_invalid("Archive vector generation has no rows"));
    }
    let dimensions = usize::try_from(input.profile.dimensions)
        .map_err(|_| vector_invalid("Archive vector dimensions overflow"))?;
    let matrix_bytes = union
        .len()
        .checked_mul(dimensions)
        .and_then(|values| values.checked_mul(std::mem::size_of::<f32>()))
        .ok_or_else(|| vector_invalid("Archive vector matrix size overflow"))?;
    let packed_frame_bytes = matrix_bytes
        .checked_add(64)
        .ok_or_else(|| vector_invalid("Archive vector frame size overflow"))?;
    u64::try_from(packed_frame_bytes)
        .map_err(|_| vector_invalid("Archive vector matrix exceeds physical size"))?;

    let mut max_fragment_archive_version = input.existing_max_fragment_archive_version;
    for version in &input.fragments {
        if union.contains_key(&version.id) {
            max_fragment_archive_version =
                max_fragment_archive_version.max(version.max_archive_version);
        }
    }
    let expected_ids = input
        .fragments
        .iter()
        .map(|fragment| fragment.id)
        .collect::<BTreeSet<_>>();
    if expected_ids.iter().any(|id| !union.contains_key(id)) {
        return Err(vector_invalid("Archive vector generation is incomplete"));
    }
    let ids = union.keys().copied().collect::<Vec<_>>();
    let vectors = union.into_values().collect::<Vec<_>>();
    let packed = session
        .common
        .packed_vectors
        .put_f32_rows(container, &vectors)?;
    container.sync()?;
    let archive_vector = session.archive_vectors.put(
        container,
        &session.common.packed_vectors,
        AcceptedArchiveMapping {
            packed_vector_id: packed,
            packed_rows: u64::try_from(ids.len())
                .map_err(|_| vector_invalid("Archive vector row count overflow"))?,
            fragment_ids: ids,
            max_fragment_archive_version,
        },
    )?;
    container.sync()?;
    let generation = session.generations.publish(
        container,
        &session.common.profiles,
        &session.archive_vectors,
        profile_id,
        archive_vector,
        source_archive_version,
    )?;
    container.sync()?;
    Ok(Some(generation))
}

fn validate_archive_vector_row(
    profile: &CompatibilityProfile,
    vector: &[f32],
) -> Result<(), OwnerSessionError> {
    let norm = vector
        .iter()
        .map(|value| f64::from(*value).powi(2))
        .sum::<f64>()
        .sqrt();
    if vector.len() != profile.dimensions as usize
        || vector.iter().any(|value| !value.is_finite())
        || !norm.is_finite()
        || (profile.normalization == VectorNormalization::L2 && (norm - 1.0).abs() > 1e-4)
    {
        return Err(vector_invalid(
            "Archive vector row is incompatible with profile",
        ));
    }
    Ok(())
}

fn same_vector(left: &[f32], right: &[f32]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.to_bits() == right.to_bits())
}

fn vector_invalid(message: &str) -> OwnerSessionError {
    OwnerSessionError::Record(message.into())
}

fn check_archive_vector_lease(writer: &RelWriter) -> Result<(), OwnerSessionError> {
    if writer.container.writer_lease_proof().is_none()
        || writer.container.writer_lease_proof() != writer.session.writer_lease_proof
    {
        Err(vector_invalid("REL Archive vector writer is stale"))
    } else {
        Ok(())
    }
}

fn finish_archive_vector_write<T>(
    writer: &mut RelWriter,
    result: Result<T, OwnerSessionError>,
) -> Result<T, OwnerSessionError> {
    if result.is_ok() {
        writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    } else if writer.session.writer_lease_proof != writer.container.writer_lease_proof()
        || result.as_ref().is_err_and(|error| {
            matches!(
                error,
                OwnerSessionError::Container(_)
                    | OwnerSessionError::PackedVector(PackedVectorError::Container(_))
                    | OwnerSessionError::ArchiveVector(ArchiveVectorError::Container(_))
                    | OwnerSessionError::Generation(GenerationStoreError::Container(_))
            )
        })
    {
        writer.session.write_poisoned = true;
    }
    result
}

pub(super) fn validate_generation_sources(
    archive: &Archive,
    common: &CommonState,
    mappings: &ArchiveVectorStore,
    generations: &VectorGenerationStore,
) -> Result<(), OwnerSessionError> {
    for generation in generations.generations() {
        let cut = generation.source_archive_version;
        if cut > archive.archive_version() {
            return Err(vector_invalid(
                "Archive generation source cut is unavailable",
            ));
        }
        let mapping = mappings.get(generation.archive_vector_id)?;
        let info = mappings
            .info(mapping.id)
            .ok_or_else(|| vector_invalid("Archive mapping is unavailable"))?;
        let profile = common
            .profiles
            .get(generation.compatibility_profile_id)
            .ok_or_else(|| vector_invalid("Archive generation profile is unavailable"))?;
        let mut max_version = 0;
        if mapping.fragment_ids.is_empty() {
            return Err(vector_invalid("Archive generation has no sources"));
        }
        let packed = common
            .packed_vectors
            .info(mapping.packed_vector_id)
            .ok_or_else(|| vector_invalid("Archive generation matrix is missing"))?;
        if packed.schema.dimensions != profile.dimensions
            || packed.count != mapping.fragment_ids.len() as u64
        {
            return Err(vector_invalid("Archive generation matrix shape changed"));
        }
        let pruned = common.packed_vectors.is_pruned(mapping.packed_vector_id);
        if pruned
            && !common
                .profiles
                .is_retired(generation.compatibility_profile_id)
        {
            return Err(vector_invalid(
                "active Archive generation has pruned backing",
            ));
        }
        for (index, id) in mapping.fragment_ids.iter().enumerate() {
            let definition = archive
                .fragment_definition(reliquary_archive::FragmentId(id.0))
                .ok_or_else(|| vector_invalid("Archive generation has an unknown fragment"))?;
            let fragment = &definition.fragment;
            let version = archive
                .node_archive_version(&fragment.conversation_id, &fragment.end_node_id)
                .ok_or_else(|| {
                    vector_invalid("Archive generation fragment has no source version")
                })?;
            if version > cut || definition.archive_cut >= cut {
                return Err(vector_invalid(
                    "Archive generation references a later fragment",
                ));
            }
            max_version = max_version.max(version);
            // Terminal, retired-profile backing is intentionally unavailable.
            // Its marker and owner-exclusive retention manifest are validated
            // by RelSession before this source/fragment history check.
            if !pruned {
                let row = common
                    .packed_vectors
                    .read_f32_row(mapping.packed_vector_id, index as u64)?;
                validate_archive_vector_row(profile, &row)?;
            }
        }
        if info.max_fragment_archive_version != max_version {
            return Err(vector_invalid(
                "Archive generation source version differs from its mapping",
            ));
        }
    }
    Ok(())
}
