//! Exact historical Dream populations backed by the checkpoint's admitted deltas.

use crate::MemoryId;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, OnceLock};

#[derive(Clone)]
pub(crate) struct Population(PopulationStorage);

#[derive(Clone)]
enum PopulationStorage {
    Materialized(Arc<BTreeSet<MemoryId>>),
    Historical {
        history: PopulationHistory,
        step: usize,
    },
}

#[derive(Clone)]
pub(crate) struct PopulationHistory(Arc<History>);

struct History {
    // A sorted list of transitions per identity. The identity is active when
    // the number of transitions through the selected step is odd.
    transitions: BTreeMap<MemoryId, Vec<usize>>,
    steps: Vec<Step>,
}

#[derive(Default)]
struct Step {
    len: usize,
    added: Vec<MemoryId>,
    removed: Vec<MemoryId>,
    materialized: OnceLock<BTreeSet<MemoryId>>,
    digest: OnceLock<[u8; 32]>,
}

impl PopulationHistory {
    pub(crate) fn from_deltas(
        deltas: impl IntoIterator<Item = (Vec<MemoryId>, Vec<MemoryId>)>,
    ) -> Result<Self, &'static str> {
        const INVALID_DELTA: &str = "invalid Dream checkpoint population delta";
        let mut transitions = BTreeMap::<_, Vec<usize>>::new();
        let mut steps = vec![Step::default()];
        let mut len = 0;
        for (mut added, mut removed) in deltas {
            let step = steps.len();
            for id in &removed {
                let changes = transitions.get_mut(id).ok_or(INVALID_DELTA)?;
                if changes.len() % 2 == 0 {
                    return Err(INVALID_DELTA);
                }
                changes.push(step);
                len -= 1;
            }
            for id in &added {
                let changes = transitions.entry(*id).or_default();
                if changes.len() % 2 != 0 {
                    return Err(INVALID_DELTA);
                }
                if changes.last() == Some(&step) {
                    // A remove followed by an add in one delta leaves the
                    // identity active and has no net effect on this step.
                    changes.pop();
                } else {
                    changes.push(step);
                }
                len += 1;
            }
            added.retain(|id| transitions[id].last() == Some(&step));
            removed.retain(|id| transitions[id].last() == Some(&step));
            added.sort_unstable();
            removed.sort_unstable();
            steps.push(Step {
                len,
                added,
                removed,
                materialized: OnceLock::new(),
                digest: OnceLock::new(),
            });
        }
        Ok(Self(Arc::new(History { transitions, steps })))
    }

    pub(crate) fn snapshot(&self, step: usize) -> Population {
        assert!(step < self.0.steps.len());
        Population(PopulationStorage::Historical {
            history: self.clone(),
            step,
        })
    }
}

fn active_at(transitions: &[usize], step: usize) -> bool {
    transitions.partition_point(|&transition| transition <= step) % 2 != 0
}

impl History {
    fn active_ids(&self, step: usize) -> impl Iterator<Item = &MemoryId> {
        self.transitions
            .iter()
            .filter_map(move |(id, transitions)| active_at(transitions, step).then_some(id))
    }

    fn materialize(&self, step: usize) -> BTreeSet<MemoryId> {
        self.active_ids(step).copied().collect()
    }

    fn changes(&self, from: usize, to: usize) -> (Vec<MemoryId>, Vec<MemoryId>) {
        if from == to {
            return (Vec::new(), Vec::new());
        }
        let steps = &self.steps[from + 1..=to];
        if let [step] = steps {
            return (step.added.clone(), step.removed.clone());
        }

        let mut added = BTreeSet::new();
        let mut removed = BTreeSet::new();
        for step in steps {
            for id in &step.added {
                if !removed.remove(id) {
                    added.insert(*id);
                }
            }
            for id in &step.removed {
                if !added.remove(id) {
                    removed.insert(*id);
                }
            }
        }
        (added.into_iter().collect(), removed.into_iter().collect())
    }
}

impl Population {
    #[cfg(test)]
    pub(super) fn is_materialized(&self) -> bool {
        match &self.0 {
            PopulationStorage::Materialized(_) => true,
            PopulationStorage::Historical { history, step } => {
                history.0.steps[*step].materialized.get().is_some()
            }
        }
    }

    #[cfg(test)]
    pub(super) fn shares_history(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (
                PopulationStorage::Historical { history: left, .. },
                PopulationStorage::Historical { history: right, .. },
            ) => Arc::ptr_eq(&left.0, &right.0),
            _ => false,
        }
    }

    pub(crate) fn from_set(ids: BTreeSet<MemoryId>) -> Self {
        Self(PopulationStorage::Materialized(Arc::new(ids)))
    }

    pub(super) fn len(&self) -> usize {
        match &self.0 {
            PopulationStorage::Materialized(ids) => ids.len(),
            PopulationStorage::Historical { history, step } => history.0.steps[*step].len,
        }
    }

    pub(crate) fn contains(&self, id: &MemoryId) -> bool {
        match &self.0 {
            PopulationStorage::Materialized(ids) => ids.contains(id),
            PopulationStorage::Historical { history, step } => history
                .0
                .transitions
                .get(id)
                .is_some_and(|changes| active_at(changes, *step)),
        }
    }

    pub(crate) fn digest(&self) -> [u8; 32] {
        match &self.0 {
            PopulationStorage::Materialized(ids) => super::population_digest(ids),
            PopulationStorage::Historical { history, step } => {
                let snapshot = &history.0.steps[*step];
                *snapshot.digest.get_or_init(|| {
                    super::population_digest_iter(snapshot.len, history.0.active_ids(*step))
                })
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = &MemoryId> {
        let (materialized, historical) = match &self.0 {
            PopulationStorage::Materialized(ids) => (Some(ids.iter()), None),
            PopulationStorage::Historical { history, step } => {
                (None, Some(history.0.active_ids(*step)))
            }
        };
        materialized
            .into_iter()
            .flatten()
            .chain(historical.into_iter().flatten())
    }

    pub(super) fn matches_set(&self, ids: &BTreeSet<MemoryId>) -> bool {
        match &self.0 {
            PopulationStorage::Materialized(known) => known.as_ref() == ids,
            PopulationStorage::Historical { .. } => {
                ids.len() == self.len() && ids.iter().all(|id| self.contains(id))
            }
        }
    }

    pub(super) fn as_set(&self) -> &BTreeSet<MemoryId> {
        match &self.0 {
            PopulationStorage::Materialized(ids) => ids,
            PopulationStorage::Historical { history, step } => history.0.steps[*step]
                .materialized
                .get_or_init(|| history.0.materialize(*step)),
        }
    }

    pub(super) fn to_set(&self) -> BTreeSet<MemoryId> {
        match &self.0 {
            PopulationStorage::Materialized(ids) => ids.as_ref().clone(),
            PopulationStorage::Historical { history, step } => history.0.steps[*step]
                .materialized
                .get()
                .cloned()
                .unwrap_or_else(|| history.0.materialize(*step)),
        }
    }

    pub(super) fn changes_since(&self, previous: &Self) -> (Vec<MemoryId>, Vec<MemoryId>) {
        if let PopulationStorage::Historical { history, step } = &self.0 {
            if let PopulationStorage::Historical {
                history: previous_history,
                step: previous_step,
            } = &previous.0
                && Arc::ptr_eq(&history.0, &previous_history.0)
            {
                if previous_step <= step {
                    return history.0.changes(*previous_step, *step);
                }
                let (added, removed) = history.0.changes(*step, *previous_step);
                return (removed, added);
            }
            if previous.len() == 0 {
                return history.0.changes(0, *step);
            }
        }
        (
            self.iter()
                .filter(|id| !previous.contains(id))
                .copied()
                .collect(),
            previous
                .iter()
                .filter(|id| !self.contains(id))
                .copied()
                .collect(),
        )
    }
}

impl fmt::Debug for Population {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Population")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

impl PartialEq for Population {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (PopulationStorage::Materialized(left), PopulationStorage::Materialized(right)) => {
                Arc::ptr_eq(left, right) || left == right
            }
            (
                PopulationStorage::Historical {
                    history: left,
                    step: left_step,
                },
                PopulationStorage::Historical {
                    history: right,
                    step: right_step,
                },
            ) if left_step == right_step && Arc::ptr_eq(&left.0, &right.0) => true,
            _ => self.len() == other.len() && self.iter().eq(other.iter()),
        }
    }
}

impl Eq for Population {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn population_membership_and_deltas_survive_removals_and_readmissions() {
        let a = MemoryId([1; 32]);
        let b = MemoryId([2; 32]);
        let c = MemoryId([3; 32]);
        let history = PopulationHistory::from_deltas([
            (vec![b, a], vec![]),
            (vec![c, b], vec![a, b]),
            (vec![a], vec![b]),
        ])
        .unwrap();

        let initial = history.snapshot(1);
        let changed = history.snapshot(2);
        let readmitted = history.snapshot(3);
        assert_eq!(initial.len(), 2);
        assert!(initial.contains(&a));
        assert!(!changed.contains(&a));
        assert!(changed.contains(&b));
        assert!(readmitted.contains(&a));
        assert!(!readmitted.contains(&b));
        assert_eq!(
            initial.changes_since(&history.snapshot(0)),
            (vec![a, b], vec![])
        );
        assert_eq!(changed.changes_since(&initial), (vec![c], vec![a]));
        assert_eq!(readmitted.changes_since(&changed), (vec![a], vec![b]));
        assert_eq!(readmitted.changes_since(&initial), (vec![c], vec![b]));
        assert_eq!(initial.changes_since(&readmitted), (vec![b], vec![c]));
        assert_eq!(
            readmitted.changes_since(&Population::from_set(BTreeSet::new())),
            (vec![a, c], vec![])
        );
        assert!(
            history
                .0
                .steps
                .iter()
                .all(|step| step.materialized.get().is_none())
        );
        assert_eq!(readmitted, Population::from_set(BTreeSet::from([a, c])));
    }

    #[test]
    fn population_digest_is_shared_without_materializing_the_snapshot() {
        let a = MemoryId([1; 32]);
        let b = MemoryId([2; 32]);
        let history = PopulationHistory::from_deltas([(vec![b, a], vec![])]).unwrap();
        let initial = history.snapshot(1);
        let other = history.snapshot(1);
        let expected = super::super::population_digest(&BTreeSet::from([a, b]));

        assert!(initial.shares_history(&other));
        assert!(history.0.steps[1].digest.get().is_none());
        assert_eq!(initial.digest(), expected);
        assert_eq!(history.0.steps[1].digest.get(), Some(&expected));
        assert_eq!(other.digest(), expected);
        assert!(!other.is_materialized());
    }

    #[test]
    fn population_history_rejects_inconsistent_deltas() {
        let a = MemoryId([1; 32]);
        assert!(PopulationHistory::from_deltas([(vec![], vec![a])]).is_err());
        assert!(PopulationHistory::from_deltas([(vec![a, a], vec![])]).is_err());
        assert!(PopulationHistory::from_deltas([(vec![a], vec![]), (vec![], vec![a, a])]).is_err());
    }
}
