//! Deterministic packing of already authorized and verified Ego context.

use std::{cmp::Ordering, collections::BTreeMap};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OwnerRank {
    ActiveRel,
    ActivePhy,
    Ancestor(u16),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PinPriority {
    High,
    Normal,
    Low,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PinKey {
    pub owner: [u8; 16],
    pub id: [u8; 16],
}

pub struct Pin<'a> {
    pub key: PinKey,
    pub owner_rank: OwnerRank,
    pub priority: PinPriority,
    pub text: &'a str,
}

pub struct PhyProfile<'a> {
    pub text: &'a str,
    pub paragraph_ends: &'a [u32],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PointKey {
    pub owner: [u8; 16],
    pub topic: [u8; 16],
    pub id: [u8; 16],
}

#[derive(Clone, Copy)]
pub struct SupportScore {
    pub memory: [u8; 32],
    pub score: i16,
}

pub fn eligible_freshness(supports: &[SupportScore]) -> bool {
    PointFreshness::from_supports(supports).is_some()
}

pub struct RelPoint<'a> {
    pub key: PointKey,
    pub owner_rank: OwnerRank,
    pub topic: &'a str,
    /// The verified statement with its scope and attribution intact.
    pub statement: &'a str,
    pub match_score: f64,
    pub supports: &'a [SupportScore],
}

#[derive(Default)]
pub struct PackageInput<'a> {
    pub identity: Option<&'a str>,
    pub pins: Vec<Pin<'a>>,
    pub phy: Option<PhyProfile<'a>>,
    pub points: Vec<RelPoint<'a>>,
}

#[derive(Default, Debug, Eq, PartialEq)]
pub struct Package {
    pub identity: Option<String>,
    pub standing_context: String,
    pub token_count: usize,
    pub pins: Vec<PinKey>,
    pub phy_end: Option<u32>,
    pub points: Vec<PointKey>,
}

pub fn pack(input: PackageInput<'_>, budget: usize, count: impl Fn(&str) -> usize) -> Package {
    let mut package = Package::default();
    if budget == 0 {
        return package;
    }

    if let Some(identity) = input.identity.filter(|text| !text.trim().is_empty()) {
        let tokens = count(identity);
        if tokens <= budget {
            package.identity = Some(identity.to_owned());
            package.token_count = tokens;
        }
    }

    let mut pins = input.pins;
    pins.sort_by_key(|pin| (pin.priority, pin.owner_rank, pin.key));
    let available = budget - package.token_count;
    let context = &mut package.standing_context;
    let mut heading = "Pinned context\n";
    for pin in pins {
        let previous = context.len();
        context.push_str(heading);
        context.push_str("\n");
        context.push_str(pin.text);
        if count(context) <= available {
            heading = "\n";
            package.pins.push(pin.key);
        } else {
            context.truncate(previous);
        }
    }

    if let Some(profile) = input.phy {
        let previous = context.len();
        append_heading(context, "Learned user profile");
        let text_start = context.len();
        for &end in profile.paragraph_ends.iter().rev() {
            let Some(prefix) = profile.text.get(..end as usize) else {
                continue;
            };
            context.push_str(prefix);
            if count(context) <= available {
                package.phy_end = Some(end);
                break;
            }
            context.truncate(text_start);
        }
        if package.phy_end.is_none() {
            context.truncate(previous);
        }
    }

    let mut points: Vec<_> = input
        .points
        .into_iter()
        .filter(|point| point.match_score.is_finite() && point.match_score > 0.0)
        .filter_map(|point| {
            let freshness = PointFreshness::from_supports(point.supports)?;
            Some((point, freshness))
        })
        .collect();
    points.sort_by(|(a, a_grade), (b, b_grade)| {
        a_grade
            .cold
            .cmp(&b_grade.cold)
            .then_with(|| a.owner_rank.cmp(&b.owner_rank))
            .then_with(|| b_grade.mean_cmp(a_grade))
            .then_with(|| b.match_score.total_cmp(&a.match_score))
            .then_with(|| a.key.cmp(&b.key))
    });
    if !points.is_empty() {
        let previous = context.len();
        append_heading(context, "Relevant REL summaries");
        let mut starts = Vec::with_capacity(points.len());
        for (point, _) in &points {
            starts.push(context.len());
            context.push_str("- [");
            context.push_str(point.topic);
            context.push_str("] ");
            context.push_str(point.statement);
            context.push('\n');
        }
        while count(context) > available {
            let Some(start) = starts.pop() else {
                break;
            };
            context.truncate(start);
        }
        if starts.is_empty() {
            context.truncate(previous);
        } else {
            package.points = points[..starts.len()]
                .iter()
                .map(|(point, _)| point.key)
                .collect();
        }
    }
    if !context.is_empty() {
        package.token_count += count(context);
    }
    package
}

fn append_heading(text: &mut String, heading: &str) {
    if !text.is_empty() {
        text.push_str("\n\n");
    }
    text.push_str(heading);
    text.push_str("\n\n");
}

struct PointFreshness {
    total: i128,
    count: i128,
    cold: usize,
}

impl PointFreshness {
    fn from_supports(supports: &[SupportScore]) -> Option<Self> {
        let mut unique = BTreeMap::new();
        for support in supports {
            if !(-100..=100).contains(&support.score) {
                return None;
            }
            if let Some(previous) = unique.insert(support.memory, support.score)
                && previous != support.score
            {
                return None;
            }
        }
        let count = unique.len() as i128;
        let total = unique.values().copied().map(i128::from).sum();
        if count == 0 || total < count {
            return None;
        }
        Some(Self {
            total,
            count,
            cold: unique.values().filter(|&&score| score <= 0).count(),
        })
    }

    fn mean_cmp(&self, other: &Self) -> Ordering {
        (self.total * other.count).cmp(&(other.total * self.count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_freshness_filters_by_mean_and_budget_removes_coldest_first() {
        let fresh = [SupportScore {
            memory: [1; 32],
            score: 70,
        }];
        let mixed = [
            SupportScore {
                memory: [1; 32],
                score: 100,
            },
            SupportScore {
                memory: [2; 32],
                score: 0,
            },
            SupportScore {
                memory: [3; 32],
                score: -10,
            },
        ];
        let below_threshold = [
            SupportScore {
                memory: [1; 32],
                score: 1,
            },
            SupportScore {
                memory: [2; 32],
                score: 0,
            },
        ];
        let make_input = || PackageInput {
            points: vec![
                point(3, OwnerRank::ActiveRel, &mixed),
                point(4, OwnerRank::ActiveRel, &below_threshold),
                point(2, OwnerRank::Ancestor(1), &fresh),
                point(1, OwnerRank::ActiveRel, &fresh),
            ],
            ..PackageInput::default()
        };
        let full = pack(make_input(), 256, str::len);
        assert_eq!(
            full.points.iter().map(|p| p.id[0]).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        let small = pack(make_input(), full.token_count - 1, str::len);
        assert_eq!(
            small.points.iter().map(|p| p.id[0]).collect::<Vec<_>>(),
            [1, 2]
        );
    }

    fn point(id: u8, owner_rank: OwnerRank, supports: &[SupportScore]) -> RelPoint<'_> {
        RelPoint {
            key: PointKey {
                owner: [1; 16],
                topic: [1; 16],
                id: [id; 16],
            },
            owner_rank,
            topic: "Builds",
            statement: "Use the configured release target.",
            match_score: 1.0,
            supports,
        }
    }

    #[test]
    fn pins_prefer_priority_and_phy_prefix_stops_at_verified_paragraph() {
        let first = "Use metric units.";
        let profile = format!("{first}\n\nConsider the local driving rules for new itineraries.");
        let ends = [first.len() as u32, profile.len() as u32];
        let full = pack(
            PackageInput {
                identity: Some("Be clear."),
                pins: vec![
                    Pin {
                        key: PinKey {
                            owner: [1; 16],
                            id: [0; 16],
                        },
                        owner_rank: OwnerRank::ActiveRel,
                        priority: PinPriority::Low,
                        text: "Check the debug log.",
                    },
                    Pin {
                        key: PinKey {
                            owner: [2; 16],
                            id: [1; 16],
                        },
                        owner_rank: OwnerRank::ActivePhy,
                        priority: PinPriority::High,
                        text: "Use accessible language.",
                    },
                ],
                phy: Some(PhyProfile {
                    text: &profile,
                    paragraph_ends: &ends,
                }),
                ..PackageInput::default()
            },
            112,
            str::len,
        );
        assert_eq!(
            full.pins.iter().map(|pin| pin.id[0]).collect::<Vec<_>>(),
            [1, 0]
        );
        assert_eq!(full.phy_end, Some(ends[0]));
        assert!(full.standing_context.ends_with(first));
        assert_eq!(full.token_count, 112);
    }
}
