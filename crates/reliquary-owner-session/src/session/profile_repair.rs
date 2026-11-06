//! Owner-verified, copy-only compatibility profile retirement.

use super::*;
use crate::{VerifiedOwnerCopyPlan, staged_copy::StagedCopy};
use reliquary_vector_store::VectorNormalization;
use sha2::{Digest, Sha256};
use std::{fs, io::Read};

const REPAIR_MAGIC: &[u8; 8] = b"RELCPRP1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileRepairError {
    InvalidRetainedSet,
    InactiveProfile(CompatibilityProfileId),
    IncompleteMemoryCoverage(CompatibilityProfileId),
    IncompleteArchiveCoverage(CompatibilityProfileId),
    OwnerMismatch,
    Verification(String),
}

impl fmt::Display for ProfileRepairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRetainedSet => {
                f.write_str("retain at least one profile with no duplicates")
            }
            Self::InactiveProfile(id) => write!(f, "profile {id:?} is absent or retired"),
            Self::IncompleteMemoryCoverage(id) => {
                write!(f, "profile {id:?} lacks current Memory vector coverage")
            }
            Self::IncompleteArchiveCoverage(id) => write!(
                f,
                "profile {id:?} lacks a complete current Archive vector generation"
            ),
            Self::OwnerMismatch => {
                f.write_str("profile repair authority does not match the source owner")
            }
            Self::Verification(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for ProfileRepairError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileRepairPlan {
    source: VerifiedOwnerCopyPlan,
    retained: Vec<CompatibilityProfileId>,
    retiring: Vec<CompatibilityProfileId>,
}

impl ProfileRepairPlan {
    pub fn owner_id(&self) -> [u8; 16] {
        self.source.owner_id()
    }

    pub fn retained(&self) -> &[CompatibilityProfileId] {
        &self.retained
    }

    pub fn retiring(&self) -> &[CompatibilityProfileId] {
        &self.retiring
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileRepairReport {
    pub owner_id: [u8; 16],
    pub retired: Vec<CompatibilityProfileId>,
    pub evidence_digest: [u8; 32],
}

/// Validate coverage and freeze the exact source cut without creating output.
pub fn plan_profile_repair(
    source: &Path,
    admission: &GraphAdmission,
    retained: &[CompatibilityProfileId],
) -> Result<ProfileRepairPlan, ProfileRepairError> {
    let mut retained = retained.to_vec();
    retained.sort_by_key(|id| id.0);
    if retained.is_empty() || retained.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ProfileRepairError::InvalidRetainedSet);
    }
    let proof = VerifiedOwnerCopyPlan::prepare(source).map_err(verification)?;
    let container = Container::open_read_only(source).map_err(verification)?;
    proof
        .verify_source(&container, source)
        .map_err(verification)?;
    let active = verify_coverage(&container, admission, &retained)?;
    let retiring = active
        .into_iter()
        .filter(|id| {
            retained
                .binary_search_by_key(&id.0, |entry| entry.0)
                .is_err()
        })
        .collect();
    Ok(ProfileRepairPlan {
        source: proof,
        retained,
        retiring,
    })
}

/// Append retirement records to a staged exact copy, then reopen and publish.
/// Keep the source leased until publication and never replace an existing path.
pub fn execute_profile_repair(
    plan: &ProfileRepairPlan,
    source: &Path,
    destination: &Path,
    admission: &GraphAdmission,
) -> Result<ProfileRepairReport, ProfileRepairError> {
    let source_lease = Container::open_read_only(source).map_err(verification)?;
    plan.source
        .verify_source(&source_lease, source)
        .map_err(verification)?;
    if admission.owner_id() != Some(plan.owner_id()) {
        return Err(ProfileRepairError::OwnerMismatch);
    }
    let staged = StagedCopy::create(destination).map_err(verification)?;
    let copy = plan
        .source
        .copy_to(source, &staged.path)
        .map_err(verification)?;
    let mut candidate = Container::open_writer(&staged.path).map_err(verification)?;
    let mut profiles = CompatibilityProfileStore::open(&candidate).map_err(verification)?;
    for id in &plan.retiring {
        if !profiles.retire(&mut candidate, *id).map_err(verification)? {
            return Err(verification("planned profile was already retired"));
        }
    }
    let evidence = repair_evidence(plan, copy.source_digest);
    let evidence_digest = Sha256::digest(&evidence).into();
    let marker = candidate.append(&evidence).map_err(verification)?;
    candidate.sync().map_err(verification)?;
    drop(candidate);

    let reopened = Container::open_read_only(&staged.path).map_err(verification)?;
    if reopened.identity() != copy.identity
        || reopened.owner_id() != copy.owner_id
        || reopened.latest_version() != copy.latest_version
        || !has_evidence(&reopened, marker, &evidence)?
        || verify_coverage(&reopened, admission, &plan.retained)? != plan.retained
    {
        return Err(verification(
            "profile repair candidate differs from the frozen plan",
        ));
    }
    drop(reopened);
    plan.source
        .verify_source(&source_lease, source)
        .map_err(verification)?;
    fs::hard_link(&staged.path, destination).map_err(verification)?;
    Ok(ProfileRepairReport {
        owner_id: copy.owner_id,
        retired: plan.retiring.clone(),
        evidence_digest,
    })
}

fn verify_coverage(
    container: &Container,
    admission: &GraphAdmission,
    retained: &[CompatibilityProfileId],
) -> Result<Vec<CompatibilityProfileId>, ProfileRepairError> {
    if admission.owner_id() != Some(container.owner_id()) {
        return Err(ProfileRepairError::OwnerMismatch);
    }
    match container.identity().file_kind {
        FileKind::Phylactery => {
            let session = PhySession::open(container, admission, []).map_err(verification)?;
            verify_memory_coverage(&session.common, retained)
        }
        FileKind::Reliquary => {
            let session = RelSession::open(container, admission, [], []).map_err(verification)?;
            let active = verify_memory_coverage(&session.common, retained)?;
            for id in retained {
                let input = super::archive_vector_publication::archive_vector_input(
                    container.owner_id(),
                    &session.archive,
                    &session.common.packed_vectors,
                    &session.common.profiles,
                    &session.archive_vectors,
                    &session.generations,
                    *id,
                )
                .map_err(|_| ProfileRepairError::IncompleteArchiveCoverage(*id))?;
                if !input.complete_current_generation() {
                    return Err(ProfileRepairError::IncompleteArchiveCoverage(*id));
                }
            }
            Ok(active)
        }
    }
}

fn verify_memory_coverage(
    common: &CommonState,
    retained: &[CompatibilityProfileId],
) -> Result<Vec<CompatibilityProfileId>, ProfileRepairError> {
    let cut = common.memories.memory_version();
    let bodies = common
        .memories
        .memory_ids()
        .filter_map(|id| common.memories.record_at_cut(id, cut))
        .map(|record| reliquary_vector_store::MemoryBodyId(record.body_id.0))
        .collect::<std::collections::HashSet<_>>();
    for id in retained {
        let profile = common
            .profiles
            .get_active(*id)
            .ok_or(ProfileRepairError::InactiveProfile(*id))?;
        for body in &bodies {
            let row = common
                .memory_vectors
                .location(*id, *body)
                .and_then(|location| {
                    Some((common.memory_vectors.info(location.set_id)?, location.row))
                })
                .and_then(|(mapping, row)| {
                    common
                        .packed_vectors
                        .read_f32_row(mapping.packed_vector_id, row)
                        .ok()
                });
            let valid = row.is_some_and(|row| {
                row.len() == profile.dimensions as usize
                    && row.iter().all(|v| v.is_finite())
                    && (profile.normalization != VectorNormalization::L2
                        || (row
                            .iter()
                            .map(|v| f64::from(*v).powi(2))
                            .sum::<f64>()
                            .sqrt()
                            - 1.0)
                            .abs()
                            <= 1e-4)
            });
            if !valid {
                return Err(ProfileRepairError::IncompleteMemoryCoverage(*id));
            }
        }
    }
    let mut active: Vec<_> = common
        .profiles
        .iter()
        .filter(|profile| !common.profiles.is_retired(profile.id))
        .map(|profile| profile.id)
        .collect();
    active.sort_by_key(|id| id.0);
    Ok(active)
}

fn repair_evidence(plan: &ProfileRepairPlan, source_digest: [u8; 32]) -> Vec<u8> {
    let mut bytes = REPAIR_MAGIC.to_vec();
    bytes.extend_from_slice(&plan.owner_id());
    bytes.extend_from_slice(&source_digest);
    bytes.extend_from_slice(&plan.source.source_latest_version().to_le_bytes());
    for profiles in [&plan.retained, &plan.retiring] {
        bytes.extend_from_slice(&(profiles.len() as u64).to_le_bytes());
        for profile in profiles {
            bytes.extend_from_slice(&profile.0);
        }
    }
    bytes.extend_from_slice(&Sha256::digest(&bytes));
    bytes
}

fn has_evidence(
    container: &Container,
    marker: reliquary_container::ObjectRef,
    expected: &[u8],
) -> Result<bool, ProfileRepairError> {
    let mut cursor = container.physical_cursor().map_err(verification)?;
    while let Some(mut frame) = cursor.next_frame().map_err(verification)? {
        if frame.object() == marker {
            let mut payload = vec![0; expected.len()];
            frame.read_exact(&mut payload).map_err(verification)?;
            return Ok(payload == expected);
        }
    }
    Ok(false)
}

fn verification(error: impl fmt::Display) -> ProfileRepairError {
    ProfileRepairError::Verification(error.to_string())
}
