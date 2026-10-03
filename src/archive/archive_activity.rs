use crate::archive_record_index::NodeIndex;
use crate::{Archive, ArchiveError, Node};

/// Disposable projection of Archive publications, aligned with NodeIndex ordinals.
#[derive(Default)]
pub(crate) struct ArchiveActivityIndex {
    first_activity_position_by_node: Vec<u64>,
    activity_changes: Vec<(u64, u64)>,
    current_count: u64,
}

pub(crate) struct ActivityRegistration {
    ordinal: usize,
    position: Option<u64>,
}

pub(crate) struct TurnAcceptance<T> {
    pub(crate) value: T,
    pub(crate) inserted: bool,
    pub(crate) activity_position: Option<u64>,
    pub(crate) rel_turn_count: u64,
}

impl ArchiveActivityIndex {
    /// Reserve everything this projection needs before publication or insertion.
    /// NodeIndex remains the sole authority for identity and immutable equality.
    pub(crate) fn prepare_node(
        &mut self,
        nodes: &mut NodeIndex,
        node: &Node,
    ) -> Result<Option<ActivityRegistration>, ArchiveError> {
        if !nodes.prepare_insert(node)? {
            return Ok(None);
        }
        let ordinal = nodes.len();
        if ordinal != self.first_activity_position_by_node.len() {
            return Err(ArchiveError::CorruptRecord(
                "activity/node ordinal alignment",
            ));
        }
        let position = if matches!(node.role.as_str(), "user" | "assistant") {
            Some(
                self.current_count
                    .checked_add(1)
                    .ok_or(ArchiveError::ActivityPositionExhausted)?,
            )
        } else {
            None
        };
        self.first_activity_position_by_node
            .try_reserve(1)
            .map_err(|_| ArchiveError::IndexCapacityExhausted)?;
        if position.is_some() {
            self.activity_changes
                .try_reserve(1)
                .map_err(|_| ArchiveError::IndexCapacityExhausted)?;
        }
        Ok(Some(ActivityRegistration { ordinal, position }))
    }

    /// Infallible after preflight; called only after publication and acceptance.
    pub(crate) fn commit_new(&mut self, prepared: ActivityRegistration, archive_version: u64) {
        assert_eq!(prepared.ordinal, self.first_activity_position_by_node.len());
        self.first_activity_position_by_node
            .push(prepared.position.unwrap_or(0));
        if let Some(position) = prepared.position {
            self.current_count = position;
            self.activity_changes.push((archive_version, position));
        }
    }

    pub(crate) fn cut_at(&self, archive_version: u64) -> u64 {
        let end = self
            .activity_changes
            .partition_point(|(version, _)| *version <= archive_version);
        end.checked_sub(1)
            .map_or(0, |index| self.activity_changes[index].1)
    }

    pub(crate) fn count(&self) -> u64 {
        self.current_count
    }

    pub(crate) fn position(&self, ordinal: usize) -> Option<u64> {
        self.first_activity_position_by_node
            .get(ordinal)
            .copied()
            .filter(|position| *position != 0)
    }

    pub(crate) fn acceptance<T>(
        &self,
        value: T,
        ordinal: usize,
        inserted: bool,
    ) -> TurnAcceptance<T> {
        TurnAcceptance {
            value,
            inserted,
            activity_position: self.position(ordinal),
            rel_turn_count: self.current_count,
        }
    }
}

impl Archive {
    // Preservation gates consume the canonical projection; they never reconstruct eligibility.
    fn activity_entries(&self) -> impl Iterator<Item = (&str, &str, u64)> {
        self.nodes.iter().enumerate().filter_map(|(ordinal, node)| {
            self.activity
                .position(ordinal)
                .map(|position| (node.conversation_id.as_str(), node.id.as_str(), position))
        })
    }

    /// Fresh replay preserves unique eligible identities and their relative first acceptance.
    pub(crate) fn activity_order_matches(&self, other: &Archive) -> bool {
        self.rel_turn_count() == other.rel_turn_count()
            && self.activity_entries().eq(other.activity_entries())
    }

    /// Physical copies preserve every cut, including non-turn and repeated publications.
    pub(crate) fn activity_history_matches(&self, other: &Archive) -> bool {
        self.archive_version() == other.archive_version()
            && self.activity.activity_changes == other.activity.activity_changes
            && self.activity_order_matches(other)
    }

    /// Divergent replay accepts left first, then only identities absent from left.
    pub(crate) fn activity_merge_order_matches(&self, left: &Archive, right: &Archive) -> bool {
        let expected =
            || {
                left.activity_entries()
                    .chain(right.activity_entries().filter(|(conversation, id, _)| {
                        left.nodes.ordinal(conversation, id).is_none()
                    }))
                    .enumerate()
                    .map(|(ordinal, (conversation, id, _))| (conversation, id, ordinal as u64 + 1))
            };
        u64::try_from(expected().count()).ok() == Some(self.rel_turn_count())
            && self.activity_entries().eq(expected())
    }
    /// Activity at a validated Archive version; version zero is the empty cut.
    pub fn activity_cut_at_archive_version(&self, version: u64) -> Result<u64, ArchiveError> {
        if version > self.archive_version() {
            return Err(ArchiveError::InvalidArchiveRecordVersion);
        }
        Ok(self.activity.cut_at(version))
    }

    pub fn rel_turn_count(&self) -> u64 {
        self.activity.count()
    }

    pub fn activity_position_for_turn(&self, conversation_id: &str, node_id: &str) -> Option<u64> {
        self.activity
            .position(self.nodes.ordinal(conversation_id, node_id)?)
    }
}

#[cfg(test)]
#[path = "archive_activity_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "archive_activity_preservation_tests.rs"]
mod preservation_tests;

#[cfg(test)]
#[path = "archive_activity_measurement_tests.rs"]
mod measurement_tests;
