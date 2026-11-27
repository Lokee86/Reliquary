use super::*;
use serde_json::Value;

impl EgoPendingWork {
    pub(crate) fn require_rel_batches(
        &self,
        run: EgoRun,
        batches: &[&AcceptedEgoBatch],
    ) -> Result<(), EgoPendingError> {
        let stored: Option<(Vec<u8>, bool)> = self
            .connection
            .query_row(
                "SELECT binding, sealed FROM ego_run WHERE id = ?1",
                [run.id().as_slice()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let binding = serde_json::to_vec(&run).map_err(|_| EgoPendingError::Codec)?;
        if !stored.is_some_and(|(stored, sealed)| stored == binding && sealed) {
            return Err(EgoPendingError::InvalidBinding);
        }
        let mut statement = self.connection.prepare(
            "SELECT sequence, id, candidates FROM ego_batch WHERE run_id = ?1 ORDER BY sequence",
        )?;
        let mut rows = statement.query_map([run.id().as_slice()], batch_row)?;
        for claimed in batches {
            let actual = rows.next().ok_or(EgoPendingError::InvalidBinding)??;
            let actual = decode_batch(run, actual)?;
            if actual.sequence != claimed.sequence
                || actual.id != claimed.id
                || actual.candidates != claimed.candidates
            {
                return Err(EgoPendingError::InvalidBinding);
            }
        }
        if rows.next().transpose()?.is_some() {
            return Err(EgoPendingError::InvalidBinding);
        }
        Ok(())
    }

    pub(crate) fn rel_stage(
        &self,
        run: EgoRun,
        key: [u8; 32],
    ) -> Result<Option<Value>, EgoPendingError> {
        self.ensure_rel_stage_table()?;
        let reply: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT reply FROM ego_rel_stage WHERE run_id = ?1 AND fingerprint = ?2",
                params![run.id().as_slice(), key.as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        reply
            .as_deref()
            .map(|bytes| serde_json::from_slice(bytes).map_err(|_| EgoPendingError::Codec))
            .transpose()
    }

    /// Retain only a fully validated stage result. If concurrent responses
    /// differ, the first accepted result remains the one used on retry.
    pub(crate) fn accept_rel_stage(
        &mut self,
        run: EgoRun,
        key: [u8; 32],
        value: &Value,
    ) -> Result<(), EgoPendingError> {
        self.ensure_rel_stage_table()?;
        let bytes = serde_json::to_vec(value).map_err(|_| EgoPendingError::Codec)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<Vec<u8>> = transaction
            .query_row(
                "SELECT reply FROM ego_rel_stage WHERE run_id = ?1 AND fingerprint = ?2",
                params![run.id().as_slice(), key.as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(previous) = existing {
            return if previous == bytes {
                Ok(())
            } else {
                Err(EgoPendingError::InvalidBinding)
            };
        }
        if !ensure_run(&transaction, run)? {
            return Err(EgoPendingError::InvalidBinding);
        }
        transaction.execute(
            "INSERT INTO ego_rel_stage (run_id, fingerprint, reply) VALUES (?1, ?2, ?3)",
            params![run.id().as_slice(), key.as_slice(), bytes],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn ensure_rel_stage_table(&self) -> Result<(), EgoPendingError> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS ego_rel_stage (
                run_id BLOB NOT NULL REFERENCES ego_run(id) ON DELETE CASCADE,
                fingerprint BLOB NOT NULL,
                reply BLOB NOT NULL,
                PRIMARY KEY (run_id, fingerprint)
            );",
        )?;
        Ok(())
    }
}
