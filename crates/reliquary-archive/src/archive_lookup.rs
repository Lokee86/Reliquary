use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash};

pub(crate) struct DenseLookup {
    state: RandomState,
    slots: Vec<usize>,
    len: usize,
}

impl Default for DenseLookup {
    fn default() -> Self {
        Self {
            state: RandomState::new(),
            slots: Vec::new(),
            len: 0,
        }
    }
}

impl DenseLookup {
    pub(crate) fn hash<T: Hash + ?Sized>(&self, key: &T) -> u64 {
        self.state.hash_one(key)
    }

    pub(crate) fn find(&self, hash: u64, mut matches: impl FnMut(usize) -> bool) -> Option<usize> {
        if self.slots.is_empty() {
            return None;
        }
        let mask = self.slots.len() - 1;
        let mut position = hash as usize & mask;
        loop {
            let index_plus_one = self.slots[position];
            if index_plus_one == 0 {
                return None;
            }
            let index = index_plus_one - 1;
            if matches(index) {
                return Some(index);
            }
            position = (position + 1) & mask;
        }
    }

    pub(crate) fn insert(
        &mut self,
        hash: u64,
        index: usize,
        mut hash_at: impl FnMut(&RandomState, usize) -> u64,
    ) {
        if self.needs_growth() {
            self.grow(&mut hash_at);
        }
        self.place(hash, index);
        self.len += 1;
    }

    pub(crate) fn try_reserve_insert(
        &mut self,
        mut hash_at: impl FnMut(&RandomState, usize) -> u64,
    ) -> Result<(), crate::ArchiveError> {
        let capacity_error = || crate::ArchiveError::IndexCapacityExhausted;
        let next = self.len.checked_add(1).ok_or_else(capacity_error)?;
        let load = next.checked_mul(10).ok_or_else(capacity_error)?;
        let limit = self.slots.len().checked_mul(7).ok_or_else(capacity_error)?;
        if !self.slots.is_empty() && load < limit {
            return Ok(());
        }
        let new_len = if self.slots.is_empty() {
            8
        } else {
            self.slots.len().checked_mul(2).ok_or_else(capacity_error)?
        };
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(new_len)
            .map_err(|_| capacity_error())?;
        replacement.resize(new_len, 0);
        let old = std::mem::replace(&mut self.slots, replacement);
        for index_plus_one in old.into_iter().filter(|value| *value != 0) {
            let index = index_plus_one - 1;
            self.place(hash_at(&self.state, index), index);
        }
        Ok(())
    }

    fn needs_growth(&self) -> bool {
        self.slots.is_empty() || (self.len + 1) * 10 >= self.slots.len() * 7
    }

    fn grow(&mut self, hash_at: &mut impl FnMut(&RandomState, usize) -> u64) {
        let new_len = if self.slots.is_empty() {
            8
        } else {
            self.slots.len() * 2
        };
        let old = std::mem::replace(&mut self.slots, vec![0; new_len]);
        for index_plus_one in old.into_iter().filter(|value| *value != 0) {
            let index = index_plus_one - 1;
            self.place(hash_at(&self.state, index), index);
        }
    }

    fn place(&mut self, hash: u64, index: usize) {
        let mask = self.slots.len() - 1;
        let mut position = hash as usize & mask;
        while self.slots[position] != 0 {
            position = (position + 1) & mask;
        }
        self.slots[position] = index + 1;
    }
}
