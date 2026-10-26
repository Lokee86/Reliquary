use crate::archive_lookup::DenseLookup;
use crate::{ArchiveError, ContentId, Fragment, FragmentId};
use reliquary_container::ObjectRef;
use std::collections::HashMap;
use std::hash::BuildHasher;

#[derive(Default)]
pub(crate) struct ContentIndex {
    records: HashMap<ContentId, ObjectRef>,
}

impl ContentIndex {
    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn get(&self, id: ContentId) -> Option<ObjectRef> {
        self.records.get(&id).copied()
    }

    pub(crate) fn contains(&self, id: ContentId) -> bool {
        self.records.contains_key(&id)
    }

    pub(crate) fn insert(&mut self, id: ContentId, chunk: ObjectRef) -> bool {
        if self.records.contains_key(&id) {
            return false;
        }
        self.records.insert(id, chunk);
        true
    }
}

#[derive(Default)]
pub(crate) struct FragmentIndex {
    records: Vec<Fragment>,
    archive_versions: Vec<u64>,
    lookup: DenseLookup,
}

impl FragmentIndex {
    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn get(&self, id: FragmentId) -> Option<&Fragment> {
        self.index(id).map(|index| &self.records[index])
    }

    pub(crate) fn archive_version(&self, id: FragmentId) -> Option<u64> {
        self.index(id).map(|index| self.archive_versions[index])
    }

    pub(crate) fn insert(
        &mut self,
        fragment: Fragment,
        archive_version: u64,
    ) -> Result<bool, ArchiveError> {
        if let Some(existing) = self.get(fragment.id) {
            return if existing == &fragment {
                Ok(false)
            } else {
                Err(ArchiveError::ConflictingFragment)
            };
        }
        let hash = self.lookup.hash(&fragment.id);
        let index = self.records.len();
        self.records.push(fragment);
        self.archive_versions.push(archive_version);
        let records = &self.records;
        self.lookup.insert(hash, index, |state, found| {
            state.hash_one(&records[found].id)
        });
        Ok(true)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Fragment> {
        self.records.iter()
    }

    fn index(&self, id: FragmentId) -> Option<usize> {
        let hash = self.lookup.hash(&id);
        self.lookup.find(hash, |index| self.records[index].id == id)
    }
}
