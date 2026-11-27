//! Restart-safe handoff between the shared audit and the PHY publisher.

use std::{path::Path, time::Duration};

use reliquary_ego::EgoCandidate;
use reliquary_memory_audit::{MAX_CANDIDATES_PER_LIST, OwnerCut, SourceId};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EgoRunKind {
    RelBootstrap,
    RelRecurring,
    PhyBootstrap,
    PhyRefresh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoRun {
    pub phy: [u8; 16],
    pub source: OwnerCut,
    pub kind: EgoRunKind,
    pub memory_version: u64,
    pub route: [u8; 32],
    pub contract: u32,
}

impl EgoRun {
    pub fn id(self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"Reliquary Ego run v1");
        hash.update(self.phy);
        hash.update(self.source.owner_id);
        hash.update(self.source.history_generation);
        hash.update(self.source.frontier.to_le_bytes());
        hash.update(self.memory_version.to_le_bytes());
        hash.update([match self.kind {
            EgoRunKind::RelBootstrap => 0,
            EgoRunKind::RelRecurring => 1,
            EgoRunKind::PhyBootstrap => 2,
            EgoRunKind::PhyRefresh => 3,
        }]);
        hash.update(self.route);
        hash.update(self.contract.to_le_bytes());
        hash.finalize().into()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct EgoBatchKey {
    pub capture: [u8; 32],
    pub ordinal: u32,
}

impl EgoBatchKey {
    pub fn id(self, run: EgoRun) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"Reliquary Ego batch v1");
        hash.update(run.id());
        hash.update(self.capture);
        hash.update(self.ordinal.to_le_bytes());
        hash.finalize().into()
    }
}

pub struct AcceptedEgoBatch {
    pub sequence: i64,
    pub id: [u8; 32],
    pub candidates: Vec<EgoCandidate>,
}

#[derive(Debug)]
pub enum EgoPendingError {
    Store(rusqlite::Error),
    Codec,
    InvalidBinding,
}

impl std::fmt::Display for EgoPendingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(_) => f.write_str("Ego pending-work database unavailable"),
            Self::Codec => f.write_str("invalid Ego pending-work record"),
            Self::InvalidBinding => {
                f.write_str("Ego pending work belongs to a different source cut")
            }
        }
    }
}

impl std::error::Error for EgoPendingError {}

impl From<rusqlite::Error> for EgoPendingError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Store(value)
    }
}

pub struct EgoPendingWork {
    connection: Connection,
}

impl EgoPendingWork {
    /// Open in the participating profile's existing protected data directory.
    pub fn open(path: &Path) -> Result<Self, EgoPendingError> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_millis(250))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA synchronous = FULL;
             PRAGMA secure_delete = ON;",
        )?;
        let schema = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = schema.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        match version {
            0 => schema.execute_batch(
                "CREATE TABLE ego_run (
                    id BLOB PRIMARY KEY NOT NULL,
                    phy BLOB NOT NULL,
                    source BLOB NOT NULL,
                    binding BLOB NOT NULL,
                    sealed INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE INDEX ego_run_pair ON ego_run (phy, source, sealed);
                 CREATE TABLE ego_batch (
                    sequence INTEGER PRIMARY KEY,
                    id BLOB UNIQUE NOT NULL,
                    run_id BLOB NOT NULL REFERENCES ego_run(id) ON DELETE CASCADE,
                    candidates BLOB NOT NULL
                 );
                 CREATE INDEX ego_batch_run ON ego_batch (run_id, sequence);
                 PRAGMA user_version = 1;",
            )?,
            1 => {}
            _ => return Err(EgoPendingError::Codec),
        }
        schema.commit()?;
        Ok(Self { connection })
    }

    pub fn accepted_batch(
        &self,
        run: EgoRun,
        key: EgoBatchKey,
    ) -> Result<Option<AcceptedEgoBatch>, EgoPendingError> {
        self.connection
            .query_row(
                "SELECT sequence, id, candidates FROM ego_batch WHERE run_id = ?1 AND id = ?2",
                params![run.id().as_slice(), key.id(run).as_slice()],
                batch_row,
            )
            .optional()?
            .map(|row| decode_batch(run, row))
            .transpose()
    }

    /// The first accepted reply wins on retry, including an accepted empty list.
    pub fn accept_batch(
        &mut self,
        run: EgoRun,
        key: EgoBatchKey,
        candidates: &[EgoCandidate],
    ) -> Result<AcceptedEgoBatch, EgoPendingError> {
        validate_candidates(run, candidates)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let sealed = ensure_run(&tx, run)?;
        let id = key.id(run);
        let existing = tx
            .query_row(
                "SELECT sequence, id, candidates FROM ego_batch WHERE run_id = ?1 AND id = ?2",
                params![run.id().as_slice(), id.as_slice()],
                batch_row,
            )
            .optional()?;
        if let Some(row) = existing {
            let batch = decode_batch(run, row)?;
            tx.commit()?;
            return Ok(batch);
        }
        if sealed {
            return Err(EgoPendingError::InvalidBinding);
        }
        let encoded = serde_json::to_vec(candidates).map_err(|_| EgoPendingError::Codec)?;
        tx.execute(
            "INSERT INTO ego_batch (id, run_id, candidates) VALUES (?1, ?2, ?3)",
            params![id.as_slice(), run.id().as_slice(), encoded],
        )?;
        let sequence = tx.last_insert_rowid();
        tx.commit()?;
        Ok(AcceptedEgoBatch {
            sequence,
            id,
            candidates: candidates.to_vec(),
        })
    }

    /// Called after the audit cursor has covered the run.
    pub fn seal(&mut self, run: EgoRun) -> Result<(), EgoPendingError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_run(&tx, run)?;
        tx.execute(
            "UPDATE ego_run SET sealed = 1 WHERE id = ?1",
            [run.id().as_slice()],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn ready_run(
        &self,
        phy: [u8; 16],
        source: [u8; 16],
    ) -> Result<Option<EgoRun>, EgoPendingError> {
        let row = self
            .connection
            .query_row(
                "SELECT id, binding FROM ego_run
                 WHERE phy = ?1 AND source = ?2 AND sealed = 1 ORDER BY rowid LIMIT 1",
                params![phy.as_slice(), source.as_slice()],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()?;
        row.map(|(id, binding)| {
            let run: EgoRun =
                serde_json::from_slice(&binding).map_err(|_| EgoPendingError::Codec)?;
            if run.phy != phy || run.source.owner_id != source || id != run.id() {
                return Err(EgoPendingError::InvalidBinding);
            }
            Ok(run)
        })
        .transpose()
    }

    pub fn next_batch(
        &self,
        run: EgoRun,
        after: i64,
    ) -> Result<Option<AcceptedEgoBatch>, EgoPendingError> {
        self.connection
            .query_row(
                "SELECT sequence, id, candidates FROM ego_batch
                 WHERE run_id = ?1 AND sequence > ?2 ORDER BY sequence LIMIT 1",
                params![run.id().as_slice(), after],
                batch_row,
            )
            .optional()?
            .map(|row| decode_batch(run, row))
            .transpose()
    }

    /// Delete only after the owner has a durable completion and the audit cursor
    /// can no longer replay these batches.
    pub fn clear_completed(&mut self, run: EgoRun) -> Result<(), EgoPendingError> {
        self.connection.execute(
            "DELETE FROM ego_run WHERE id = ?1 AND sealed = 1",
            [run.id().as_slice()],
        )?;
        Ok(())
    }
}

fn ensure_run(connection: &Connection, run: EgoRun) -> Result<bool, EgoPendingError> {
    let binding = serde_json::to_vec(&run).map_err(|_| EgoPendingError::Codec)?;
    let id = run.id();
    connection.execute(
        "INSERT INTO ego_run (id, phy, source, binding) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO NOTHING",
        params![
            id.as_slice(),
            run.phy.as_slice(),
            run.source.owner_id.as_slice(),
            &binding,
        ],
    )?;
    let (stored, sealed): (Vec<u8>, bool) = connection.query_row(
        "SELECT binding, sealed FROM ego_run WHERE id = ?1",
        [id.as_slice()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if stored != binding {
        return Err(EgoPendingError::InvalidBinding);
    }
    Ok(sealed)
}

type BatchRow = (i64, Vec<u8>, Vec<u8>);

fn batch_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BatchRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
}

fn decode_batch(
    run: EgoRun,
    (sequence, id, encoded): BatchRow,
) -> Result<AcceptedEgoBatch, EgoPendingError> {
    let candidates: Vec<EgoCandidate> =
        serde_json::from_slice(&encoded).map_err(|_| EgoPendingError::Codec)?;
    validate_candidates(run, &candidates)?;
    Ok(AcceptedEgoBatch {
        sequence,
        id: id.try_into().map_err(|_| EgoPendingError::Codec)?,
        candidates,
    })
}

fn validate_candidates(run: EgoRun, candidates: &[EgoCandidate]) -> Result<(), EgoPendingError> {
    if candidates.len() > MAX_CANDIDATES_PER_LIST
        || candidates.iter().any(|candidate| {
            candidate.subject.trim().is_empty()
                || candidate.subject.len() > 160
                || !(1..=6).contains(&candidate.sources.len())
                || candidate.sources.iter().any(|source| {
                    source.owner_id != run.source.owner_id
                        || source.history_generation != run.source.history_generation
                        || !matches!(source.source, SourceId::Memory(_))
                        || source.revision == 0
                        || source.start_byte >= source.end_byte
                })
        })
    {
        return Err(EgoPendingError::InvalidBinding);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn accepted_empty_batch_replays_and_is_cleared_after_completion() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&directory).unwrap();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = directory.join(format!("ego-pending-{}-{nonce}.sqlite", std::process::id()));
        let run = EgoRun {
            phy: [1; 16],
            source: OwnerCut {
                owner_id: [2; 16],
                history_generation: [3; 32],
                frontier: 14,
            },
            kind: EgoRunKind::RelBootstrap,
            memory_version: 31,
            route: [4; 32],
            contract: 3,
        };
        let key = EgoBatchKey {
            capture: [5; 32],
            ordinal: 0,
        };
        {
            let mut pending = EgoPendingWork::open(&path).unwrap();
            assert_eq!(pending.accept_batch(run, key, &[]).unwrap().id, key.id(run));
            assert_eq!(
                pending.ready_run(run.phy, run.source.owner_id).unwrap(),
                None
            );
        }
        {
            let mut pending = EgoPendingWork::open(&path).unwrap();
            let receipt = pending.accepted_batch(run, key).unwrap().unwrap();
            assert!(receipt.candidates.is_empty());
            assert_eq!(
                pending.accept_batch(run, key, &[]).unwrap().sequence,
                receipt.sequence
            );
            pending.seal(run).unwrap();
            assert_eq!(
                pending.ready_run(run.phy, run.source.owner_id).unwrap(),
                Some(run)
            );
            assert_eq!(pending.next_batch(run, 0).unwrap().unwrap().id, receipt.id);
            pending.clear_completed(run).unwrap();
            assert!(pending.accepted_batch(run, key).unwrap().is_none());
        }
        fs::remove_file(path).unwrap();
    }
}
