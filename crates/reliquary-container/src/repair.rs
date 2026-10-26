//! Explicit physical tail evidence and repair mechanics; owners authorize repair.
use super::{Container, ContainerError};

/// Opaque physical evidence for one incomplete tail; owner authorization remains above Container.
pub struct TailRepairPlan {
    valid_end: u64,
    observed_len: u64,
    lease_token: [u8; 16],
}

impl TailRepairPlan {
    /// End of the complete physical prefix accepted for this repair plan.
    pub fn valid_end(&self) -> u64 {
        self.valid_end
    }

    /// Physical length observed when the incomplete-tail evidence was created.
    pub fn observed_len(&self) -> u64 {
        self.observed_len
    }
}

impl Container {
    pub fn plan_tail_repair(&mut self) -> Result<Option<TailRepairPlan>, ContainerError> {
        self.require_writer_lease()?;
        if self.mode != super::SessionMode::Repair {
            return Err(ContainerError::ReadOnly);
        }
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

    pub fn repair_incomplete_tail(&mut self, plan: TailRepairPlan) -> Result<(), ContainerError> {
        self.require_writer_lease()?;
        if self.mode != super::SessionMode::Repair {
            return Err(ContainerError::ReadOnly);
        }
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
}
