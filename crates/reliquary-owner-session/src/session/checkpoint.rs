//! Private, disposable startup state for one admitted REL prefix.

mod dream_suffix;
mod pages;
mod private_cache;
mod suffix;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_dream_suffix;
#[cfg(test)]
mod tests_echo;
#[cfg(test)]
mod tests_native_import;
#[cfg(test)]
mod tests_observations;

use super::{OwnerSessionError, RelSession};
use reliquary_container::{Container, FileKind, PhysicalCheckpoint, VerifiedPrefix};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"RELCP010";
const STATE_START: u64 = 16;
const MAX_MANIFEST_BYTES: usize = 16 * 1024 * 1024;
const REFRESH_BYTES: u64 = 16 * 1024 * 1024;
const REFRESH_FRAMES: u64 = 16_384;

#[derive(Serialize, Deserialize)]
struct Manifest {
    physical: PhysicalCheckpoint,
    generation: [u8; 32],
    memory_cut: u64,
    graph_cut: u64,
    archive_cut: u64,
    identity_cut: u64,
    separation_cut: u64,
    observation_cut: u64,
    state_len: u64,
}

impl Manifest {
    fn matches(&self, session: &RelSession) -> bool {
        session.owner_id == self.physical.owner_id
            && session.container_identity == self.physical.identity
            && session.container_file_identity == self.physical.file_identity
            && session.history_generation == self.generation
            && session.memory_version() == self.memory_cut
            && session.graph_version() == self.graph_cut
            && session.archive_version() == self.archive_cut
            && session.identity_history.frontier() == self.identity_cut
            && session.identity_separation_history.frontier() == self.separation_cut
            && session.observation_publication_version() == self.observation_cut
    }
}

struct Entry {
    file: File,
    manifest: Manifest,
}

fn invalid_cache() -> io::Error {
    io::ErrorKind::InvalidData.into()
}

fn encode(value: &impl Serialize) -> io::Result<Vec<u8>> {
    rmp_serde::to_vec_named(value).map_err(io::Error::other)
}

fn cut_seal(physical: &PhysicalCheckpoint, generation: &[u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(MAGIC);
    hash.update(physical.owner_id);
    hash.update(physical.file_identity.0.to_le_bytes());
    hash.update(physical.file_identity.1.to_le_bytes());
    hash.update(physical.covered_end.to_le_bytes());
    hash.update(physical.digest);
    hash.update(generation);
    hash.finalize().into()
}

impl Entry {
    fn open(path: &Path) -> io::Result<Self> {
        let mut file = private_cache::open(path)?;
        let mut magic = [0; 8];
        file.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(invalid_cache());
        }
        let mut manifest_at = [0; 8];
        file.read_exact(&mut manifest_at)?;
        let manifest_at = u64::from_le_bytes(manifest_at);
        let extent = file.metadata()?.len();
        if !(STATE_START..extent).contains(&manifest_at) {
            return Err(invalid_cache());
        }
        file.seek(SeekFrom::Start(manifest_at))?;
        let mut length = [0; 4];
        file.read_exact(&mut length)?;
        let length = u32::from_le_bytes(length) as usize;
        if length == 0
            || length > MAX_MANIFEST_BYTES
            || extent - manifest_at != (36 + length) as u64
        {
            return Err(invalid_cache());
        }
        let mut digest: [u8; 32] = [0; 32];
        file.read_exact(&mut digest)?;
        let mut manifest = vec![0; length];
        file.read_exact(&mut manifest)?;
        if <[u8; 32]>::from(Sha256::digest(&manifest)) != digest {
            return Err(invalid_cache());
        }
        let manifest: Manifest = rmp_serde::from_slice(&manifest).map_err(io::Error::other)?;
        let state_len = usize::try_from(manifest.state_len).map_err(io::Error::other)?;
        if state_len == 0 || state_len > MAX_PROJECTION_BYTES {
            return Err(invalid_cache());
        }
        let state_bytes = state_len + 32 * state_len.div_ceil(PAGE_BYTES);
        if manifest_at != STATE_START + state_bytes as u64 {
            return Err(invalid_cache());
        }
        Ok(Self { file, manifest })
    }

    fn session(&mut self) -> io::Result<RelSession> {
        self.file.seek(SeekFrom::Start(STATE_START))?;
        let mut pages = PageReader::new(
            &mut self.file,
            cut_seal(&self.manifest.physical, &self.manifest.generation),
            self.manifest.state_len as usize,
        );
        let session: RelSession = rmp_serde::from_read(&mut pages).map_err(io::Error::other)?;
        if !pages.is_exhausted() || !self.manifest.matches(&session) {
            return Err(invalid_cache());
        }
        Ok(session)
    }
}

pub(super) struct WriterCheckpoint {
    slot: PathBuf,
    prefix: Option<VerifiedPrefix>,
    admitted_extent: u64,
    attempted_extent: Option<u64>,
    attempted_epoch: u64,
    refresh_live_cut: bool,
    #[cfg(feature = "d02-observation")]
    pub(super) opened_prefix: Option<u64>,
    #[cfg(feature = "d02-observation")]
    pub(super) refresh_error: Option<String>,
}

impl WriterCheckpoint {
    fn new(
        slot: PathBuf,
        prefix: Option<VerifiedPrefix>,
        admitted_extent: u64,
        session: &RelSession,
    ) -> Self {
        Self {
            slot,
            admitted_extent,
            attempted_extent: prefix.as_ref().map(VerifiedPrefix::covered_end),
            #[cfg(feature = "d02-observation")]
            opened_prefix: prefix.as_ref().map(VerifiedPrefix::covered_end),
            #[cfg(feature = "d02-observation")]
            refresh_error: None,
            prefix,
            attempted_epoch: 0,
            refresh_live_cut: session.common.maintenance.is_empty()
                && session.common.memories.unbacked_body_ids().next().is_none(),
        }
    }

    pub(super) fn refresh(&mut self, container: &Container, session: &RelSession, quiescing: bool) {
        if session.write_poisoned || container.writer_lease_proof() != session.writer_lease_proof {
            return;
        }
        let Ok(extent) = container.extent() else {
            return;
        };
        let epoch = container.append_epoch();
        // Maintenance evidence and newly backed revisions can affect checks
        // outside the current publication. Refresh after full admission.
        if epoch > 0 && !self.refresh_live_cut {
            return;
        }
        if self.attempted_extent == Some(extent) {
            return;
        }
        let refresh_from = self.attempted_extent.unwrap_or(self.admitted_extent);
        if !quiescing
            && extent.saturating_sub(refresh_from) < REFRESH_BYTES
            && epoch.saturating_sub(self.attempted_epoch) < REFRESH_FRAMES
        {
            return;
        }
        self.attempted_extent = Some(extent);
        self.attempted_epoch = epoch;
        match self.publish(container, session) {
            Ok(prefix) => self.prefix = Some(prefix),
            Err(_error) => {
                #[cfg(feature = "d02-observation")]
                {
                    self.refresh_error = Some(_error.to_string());
                }
            }
        }
    }

    fn publish(&self, container: &Container, session: &RelSession) -> io::Result<VerifiedPrefix> {
        let (physical, prefix) = container
            .capture_checkpoint(self.prefix.as_ref())
            .map_err(io::Error::other)?;
        let parent = self.slot.parent().ok_or_else(invalid_cache)?;
        let mut staged = private_cache::stage(parent)?;
        staged.write_all(MAGIC)?;
        staged.write_all(&[0; 8])?;
        let mut pages = PageWriter::new(
            &mut staged,
            cut_seal(&physical, &session.history_generation),
        );
        session
            .serialize(&mut rmp_serde::Serializer::new(&mut pages).with_struct_map())
            .map_err(io::Error::other)?;
        let state_len = pages.finish()?;
        let manifest_at = staged.stream_position()?;
        let manifest = encode(&Manifest {
            physical,
            generation: session.history_generation,
            memory_cut: session.memory_version(),
            graph_cut: session.graph_version(),
            archive_cut: session.archive_version(),
            identity_cut: session.identity_history.frontier(),
            separation_cut: session.identity_separation_history.frontier(),
            observation_cut: session.observation_publication_version(),
            state_len: state_len as u64,
        })?;
        if manifest.len() > MAX_MANIFEST_BYTES {
            return Err(invalid_cache());
        }

        let digest: [u8; 32] = Sha256::digest(&manifest).into();
        staged.write_all(&(manifest.len() as u32).to_le_bytes())?;
        staged.write_all(&digest)?;
        staged.write_all(&manifest)?;
        staged.seek(SeekFrom::Start(8))?;
        staged.write_all(&manifest_at.to_le_bytes())?;
        staged.as_file().sync_all()?;
        staged
            .persist(&self.slot)
            .map_err(|failure| failure.error)?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(prefix)
    }
}

fn cache_slot(path: &Path) -> io::Result<PathBuf> {
    let canonical = path.canonicalize()?;
    let mut hash = Sha256::new();
    hash.update(b"Reliquary owner-session checkpoint: REL");
    hash.update(canonical.as_os_str().as_encoded_bytes());
    let slot = format!("{:x}.rel-checkpoint", hash.finalize());
    Ok(private_cache::root()?.join(slot))
}

pub(super) fn open(
    path: &Path,
    writable: bool,
    expected: Option<[u8; 16]>,
) -> Result<(Container, RelSession, Option<WriterCheckpoint>), OwnerSessionError> {
    let lease = Container::prepare_checkpoint_open(path, writable, FileKind::Reliquary, expected)?;
    let extent = lease.extent();
    let slot = cache_slot(lease.path())
        .inspect_err(|_error| {
            #[cfg(feature = "d02-observation")]
            eprintln!("checkpoint cache directory: {_error}");
        })
        .ok();
    open_leased(lease, slot, extent, writable)
}

fn open_leased(
    lease: reliquary_container::LeasedContainer,
    slot: Option<PathBuf>,
    extent: u64,
    writable: bool,
) -> Result<(Container, RelSession, Option<WriterCheckpoint>), OwnerSessionError> {
    let mut entry = slot.as_deref().and_then(|path| Entry::open(path).ok());
    let physical = entry.as_ref().map(|entry| entry.manifest.physical.clone());
    let (mut container, prefix) = lease.finish(physical)?;

    if let (Some(entry), Some(_)) = (&mut entry, prefix.as_ref())
        && let Ok(mut session) = entry.session().inspect_err(|_error| {
            #[cfg(feature = "d02-observation")]
            eprintln!("checkpoint state: {_error}");
        })
        && session.bind_checkpoint_reader(&container).is_ok()
        && suffix::replay(&mut session, &container, &entry.manifest.physical, extent)?
    {
        container.require_extent(extent)?;
        container.release_replay_directory();
        let refresh = slot
            .filter(|_| writable)
            .map(|slot| WriterCheckpoint::new(slot, prefix, extent, &session));
        return Ok((container, session, refresh));
    }
    // Full admission can replace this checkpoint. Release its file handle first.
    drop(entry);
    if prefix.is_some() {
        container.validate_from_start()?;
    }
    let session = RelSession::open_leased_current(&container)?;
    container.require_extent(extent)?;
    container.release_replay_directory();

    // The writer publishes the first cache at its normal refresh boundary or
    // when it closes. A cache miss should not make admission wait for a cache.
    let refresh = slot
        .filter(|_| writable)
        .map(|slot| WriterCheckpoint::new(slot, None, extent, &session));
    Ok((container, session, refresh))
}
use self::pages::{MAX_PROJECTION_BYTES, PAGE_BYTES, PageReader, PageWriter};
