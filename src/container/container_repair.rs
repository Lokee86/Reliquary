//! Explicit physical tail evidence and repair mechanics; owners authorize repair.
use super::{Container, ContainerError, Lease};
use std::{
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

/// Opaque authorization evidence for one incomplete physical tail. Only a
/// Container holding the same exclusive lease can consume it.
pub(crate) struct TailRepairPlan {
    valid_end: u64,
    observed_len: u64,
    lease_token: [u8; 16],
}

impl Container {
    pub(crate) fn plan_tail_repair(&mut self) -> Result<Option<TailRepairPlan>, ContainerError> {
        self.require_writer_lease()?;
        let Some(valid_end) = self.inspect_tail()? else {
            return Ok(None);
        };
        let observed_len = self.file.metadata()?.len();
        let lease_token = self.lease.as_ref().expect("writer lease checked").token();
        Ok(Some(TailRepairPlan {
            valid_end,
            observed_len,
            lease_token,
        }))
    }

    pub(crate) fn repair_incomplete_tail(
        &mut self,
        plan: TailRepairPlan,
    ) -> Result<(), ContainerError> {
        self.require_write_admission()?;
        let lease_token = self.lease.as_ref().expect("writer lease checked").token();
        if plan.lease_token != lease_token || self.file.metadata()?.len() != plan.observed_len {
            return Err(ContainerError::UnstableSnapshot);
        }
        if self.inspect_tail()? != Some(plan.valid_end) {
            return Err(ContainerError::UnstableSnapshot);
        }
        if plan.valid_end < self.header_len || plan.valid_end > plan.observed_len {
            return Err(ContainerError::InvalidPrefixBoundary);
        }
        let _mutation = self.mutation_gate()?;
        self.validate_path_identity()?;
        self.file.set_len(plan.valid_end)?;
        self.file.sync_all()?;
        drop(_mutation);
        let clock_state = {
            let mut cursor = self.physical_cursor()?;
            while cursor.next_frame()?.is_some() {}
            cursor.finish_open_scan()?
        };
        self.apply_scan_clock(clock_state);
        Ok(())
    }

    pub(crate) fn copy_accepted_prefix(
        &mut self,
        plan: &TailRepairPlan,
        destination: impl AsRef<Path>,
    ) -> Result<Container, ContainerError> {
        self.require_writer_lease()?;
        let _read = self.read_gate()?;
        let lease_token = self.lease.as_ref().expect("writer lease checked").token();
        if plan.lease_token != lease_token || self.file.metadata()?.len() != plan.observed_len {
            return Err(ContainerError::UnstableSnapshot);
        }
        if self.inspect_tail()? != Some(plan.valid_end) {
            return Err(ContainerError::UnstableSnapshot);
        }
        let target_path = destination.as_ref();
        let mut target = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(target_path)?;
        let target_lease = match Lease::exclusive(&target) {
            Ok(lease) => lease,
            Err(error) => {
                let created_identity = super::lease::file_identity(&target)
                    .ok()
                    .and_then(|identity| identity.as_pair());
                let path_identity = std::fs::File::open(target_path)
                    .ok()
                    .and_then(|file| super::lease::file_identity(&file).ok())
                    .and_then(|identity| identity.as_pair());
                if created_identity.is_some() && created_identity == path_identity {
                    let _ = std::fs::remove_file(target_path);
                }
                return Err(error);
            }
        };
        let result = (|| {
            self.file.seek(SeekFrom::Start(0))?;
            let mut remaining = plan.valid_end;
            let mut buffer = [0_u8; 64 * 1024];
            while remaining > 0 {
                let take = usize::try_from(remaining.min(buffer.len() as u64)).unwrap();
                self.file.read_exact(&mut buffer[..take])?;
                target.write_all(&buffer[..take])?;
                remaining -= take as u64;
            }
            target.sync_all()?;
            self.file.seek(SeekFrom::End(0))?;
            self.validate_path_identity()?;
            target.seek(SeekFrom::Start(0))?;
            super::read_header(&mut target)
        })();
        let (version, identity, owner_uuid, header_len) = match result {
            Ok(header) => header,
            Err(error) => {
                if target_lease.ensure_path_identity(target_path).is_ok() {
                    let _ = std::fs::remove_file(target_path);
                }
                return Err(error);
            }
        };
        Ok(Container {
            file: target,
            lease: Some(target_lease),
            mutation_gate: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            path: target_path.to_path_buf(),
            version,
            identity,
            owner_uuid,
            header_len,
            next_version: 1,
            transaction_times: Default::default(),
            next_transaction_time_override: None,
            write_admission_closed: false,
            owner_write_scope: false,
        })
    }
}
