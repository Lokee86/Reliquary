use super::*;
use crate::{Cva, EgoAnchorPriority, Phylactery};
use std::{collections::BTreeMap, fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str) -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("ego-history-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_phy_head(phy: &Phylactery, cut: &EgoCut) {
    assert_eq!(
        cut.identities,
        phy.ego_identities()
            .into_iter()
            .map(|identity| (identity.id, identity))
            .collect::<BTreeMap<_, _>>()
    );
    assert_eq!(cut.active_identity, phy.active_ego_identity_id());
    assert_eq!(
        cut.anchors,
        phy.ego_anchors()
            .into_iter()
            .map(|anchor| (anchor.id, anchor))
            .collect::<BTreeMap<_, _>>()
    );
    assert_eq!(cut.personality, phy.ego_personality().cloned());
    assert_eq!(cut.synthesis, phy.ego_web_synthesis().cloned());
}

fn assert_rel_head(rel: &Cva, cut: &EgoCut) {
    assert!(cut.identities.is_empty());
    assert_eq!(cut.active_identity, None);
    assert_eq!(
        cut.anchors,
        rel.ego_anchors()
            .into_iter()
            .map(|anchor| (anchor.id, anchor))
            .collect::<BTreeMap<_, _>>()
    );
    assert_eq!(cut.personality, None);
    assert_eq!(cut.synthesis, rel.ego_web_synthesis().cloned());
}

#[test]
fn legacy_identity_carrier_matches_its_canonical_identity_projection() {
    let text = "You are the default assistant.".to_owned();
    let legacy = EgoHistory {
        owner: [7; 16],
        records: vec![EgoRecord::LegacyIdentity {
            ego_version: 1,
            revision: 1,
            text: text.clone(),
        }],
    }
    .at(1)
    .unwrap();
    let current = EgoHistory {
        owner: [7; 16],
        records: vec![EgoRecord::Identity {
            ego_version: 1,
            id: crate::ego_store::LEGACY_IDENTITY_ID,
            revision: 1,
            deleted: false,
            activate: true,
            name: "Default".into(),
            text,
        }],
    }
    .at(1)
    .unwrap();

    assert_eq!(legacy.fingerprint, current.fingerprint);
    assert_eq!(legacy.identities, current.identities);
    assert_eq!(legacy.active_identity, current.active_identity);
}

#[test]
fn phylactery_ego_cuts_match_revisions_activation_tombstones_and_reopen() {
    let fixture = Fixture::new("phy-synthetic");
    let path = fixture.path("owner.phy");
    let mut phy = Phylactery::create(&path).unwrap();

    let first = phy
        .create_ego_identity("Default".into(), "First identity.".into())
        .unwrap();
    let cut_after_first = phy.ego_version();
    let before_second = EgoHistory::read(&phy.container, &phy.ego)
        .unwrap()
        .at(cut_after_first)
        .unwrap();

    let second = phy
        .create_ego_identity("Work".into(), "Work identity.".into())
        .unwrap();
    let (second, changed) = phy
        .update_ego_identity(
            second.id,
            second.revision,
            "Work".into(),
            "Updated work identity.".into(),
        )
        .unwrap();
    assert!(changed);
    let cut_after_update = phy.ego_version();
    assert!(phy.activate_ego_identity(second.id).unwrap());
    assert!(phy.delete_ego_identity(first.id, first.revision).unwrap());

    let anchor = phy
        .create_ego_anchor(EgoAnchorPriority::High, "High priority anchor.".into())
        .unwrap();
    let (anchor, changed) = phy
        .update_ego_anchor(
            anchor.id,
            anchor.revision,
            EgoAnchorPriority::Low,
            "Low priority revised anchor.".into(),
        )
        .unwrap();
    assert!(changed);
    let cut_after_anchor_update = phy.ego_version();
    assert!(phy.delete_ego_anchor(anchor.id, anchor.revision).unwrap());

    let (personality, changed) = phy
        .put_ego_personality(0, "Be clear and direct.".into(), 0)
        .unwrap();
    assert!(changed);
    let personality_version = phy.ego_version();
    assert!(
        !phy.put_ego_personality(1, "Be clear and direct.".into(), 0)
            .unwrap()
            .1
    );
    assert_eq!(phy.ego_version(), personality_version);
    let (synthesis, changed) = phy
        .put_ego_web_synthesis(0, 0, "Current synthesized context.".into())
        .unwrap();
    assert!(changed);

    let history = EgoHistory::read(&phy.container, &phy.ego).unwrap();
    let head = phy.ego_version();
    let head_cut = history.at(head).unwrap();
    assert_phy_head(&phy, &head_cut);
    assert_eq!(head_cut.identities.get(&second.id), Some(&second));
    assert_eq!(head_cut.active_identity, Some(second.id));
    assert_eq!(head_cut.anchors, BTreeMap::new());
    assert_eq!(head_cut.personality, Some(personality.clone()));
    assert_eq!(head_cut.synthesis, Some(synthesis.clone()));
    assert!(history.at(head + 1).is_err());

    assert_eq!(
        history.at(cut_after_first).unwrap().fingerprint,
        before_second.fingerprint
    );
    assert_eq!(
        history
            .at(cut_after_first)
            .unwrap()
            .identities
            .get(&first.id),
        Some(&first)
    );
    assert_eq!(
        history.at(cut_after_first).unwrap().active_identity,
        Some(first.id)
    );
    assert_eq!(
        phy.historical_ego_identity(second.id, 1),
        Some(crate::EgoIdentity {
            id: second.id,
            revision: 1,
            name: "Work".into(),
            text: "Work identity.".into(),
        })
    );
    assert_eq!(
        phy.historical_ego_identity(first.id, 1),
        Some(first.clone())
    );
    assert_eq!(phy.historical_ego_identity(first.id, 2), None);
    assert_eq!(
        phy.historical_ego_anchor(anchor.id, 1),
        Some(crate::EgoAnchor {
            id: anchor.id,
            revision: 1,
            priority: EgoAnchorPriority::High,
            text: "High priority anchor.".into(),
        })
    );
    assert_eq!(
        phy.historical_ego_anchor(anchor.id, 2),
        Some(anchor.clone())
    );
    assert_eq!(phy.historical_ego_anchor(anchor.id, 3), None);

    let no_op_version = phy.ego_version();
    assert!(!phy.activate_ego_identity(second.id).unwrap());
    assert!(!phy.delete_ego_identity(first.id, first.revision).unwrap());
    assert!(!phy.delete_ego_anchor(anchor.id, anchor.revision).unwrap());
    assert_eq!(phy.ego_version(), no_op_version);
    phy.sync().unwrap();
    drop(phy);

    let reopened = Phylactery::open(&path).unwrap();
    let reopened_history = EgoHistory::read(&reopened.container, &reopened.ego).unwrap();
    for cut in [
        0,
        cut_after_first,
        cut_after_update,
        cut_after_anchor_update,
        head,
    ] {
        let before = history.at(cut).unwrap();
        let after = reopened_history.at(cut).unwrap();
        assert_eq!(before.fingerprint, after.fingerprint);
        assert_eq!(before.identities, after.identities);
        assert_eq!(before.active_identity, after.active_identity);
        assert_eq!(before.anchors, after.anchors);
        assert_eq!(before.personality, after.personality);
        assert_eq!(before.synthesis, after.synthesis);
    }
    assert_phy_head(&reopened, &reopened_history.at(head).unwrap());
}

#[test]
fn reliquary_ego_cuts_match_anchor_and_synthesis_history_after_reopen() {
    let fixture = Fixture::new("rel-synthetic");
    let path = fixture.path("owner.rel");
    let mut rel = Cva::create(&path).unwrap();
    let anchor = rel
        .create_ego_anchor(EgoAnchorPriority::Normal, "REL anchor.".into())
        .unwrap();
    let after_create = rel.ego_version();
    let before_change = EgoHistory::read(&rel.container, &rel.ego)
        .unwrap()
        .at(after_create)
        .unwrap();
    let (anchor, changed) = rel
        .update_ego_anchor(
            anchor.id,
            anchor.revision,
            EgoAnchorPriority::High,
            "Revised REL anchor.".into(),
        )
        .unwrap();
    assert!(changed);
    let after_update = rel.ego_version();
    assert!(rel.delete_ego_anchor(anchor.id, anchor.revision).unwrap());
    let (synthesis, changed) = rel
        .put_ego_web_synthesis(0, 0, "REL synthesis.".into())
        .unwrap();
    assert!(changed);
    let history = EgoHistory::read(&rel.container, &rel.ego).unwrap();
    let head = rel.ego_version();
    assert_rel_head(&rel, &history.at(head).unwrap());
    assert_eq!(
        history.at(after_create).unwrap().fingerprint,
        before_change.fingerprint
    );
    assert_eq!(
        history.at(after_create).unwrap().anchors.get(&anchor.id),
        Some(&crate::EgoAnchor {
            id: anchor.id,
            revision: 1,
            priority: EgoAnchorPriority::Normal,
            text: "REL anchor.".into(),
        })
    );
    assert_eq!(
        history.at(after_update).unwrap().anchors.get(&anchor.id),
        Some(&anchor)
    );
    assert_eq!(history.at(head).unwrap().synthesis, Some(synthesis));
    assert_eq!(
        rel.historical_ego_anchor(anchor.id, 1).unwrap().text,
        "REL anchor."
    );
    assert_eq!(
        rel.historical_ego_anchor(anchor.id, 2),
        Some(anchor.clone())
    );
    assert_eq!(rel.historical_ego_anchor(anchor.id, 3), None);

    rel.sync().unwrap();
    drop(rel);
    let reopened = Cva::open(&path).unwrap();
    let reopened_history = EgoHistory::read(&reopened.container, &reopened.ego).unwrap();
    for cut in [0, after_create, after_update, head] {
        assert_eq!(
            reopened_history.at(cut).unwrap().fingerprint,
            history.at(cut).unwrap().fingerprint
        );
    }
    assert_rel_head(&reopened, &reopened_history.at(head).unwrap());
}

#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn authoritative_ego_history_fingerprints_are_repeatable_at_sampled_cuts() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("fixture root"));
    let fixture = Fixture::new("authoritative");
    for (index, corpus) in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ]
    .iter()
    .enumerate()
    {
        for (kind, name) in [("rel", "project.prj.rel"), ("phy", "user.phy")] {
            let path = fixture.path(&format!("{index}.{kind}"));
            let mut source = fs::File::open(root.join(corpus).join(name)).unwrap();
            let mut destination = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            std::io::copy(&mut source, &mut destination).unwrap();
            drop(destination);

            if kind == "rel" {
                let rel = Cva::open(&path).unwrap();
                let first_history = EgoHistory::read(&rel.container, &rel.ego).unwrap();
                let second_history = EgoHistory::read(&rel.container, &rel.ego).unwrap();
                let head = rel.ego_version();
                let mut cuts = vec![0, 1.min(head), head / 2, head];
                cuts.sort_unstable();
                cuts.dedup();
                for cut in cuts {
                    let first = first_history.at(cut).unwrap();
                    let second = second_history.at(cut).unwrap();
                    assert_eq!(first.fingerprint, second.fingerprint);
                    assert_eq!(first.anchors, second.anchors);
                    assert_eq!(first.synthesis, second.synthesis);
                    if cut == head {
                        assert_rel_head(&rel, &first);
                    }
                }
                eprintln!("Ego fixture {corpus}/rel: head {head}");
            } else {
                let phy = Phylactery::open(&path).unwrap();
                let first_history = EgoHistory::read(&phy.container, &phy.ego).unwrap();
                let second_history = EgoHistory::read(&phy.container, &phy.ego).unwrap();
                let head = phy.ego_version();
                let mut cuts = vec![0, 1.min(head), head / 2, head];
                cuts.sort_unstable();
                cuts.dedup();
                for cut in cuts {
                    let first = first_history.at(cut).unwrap();
                    let second = second_history.at(cut).unwrap();
                    assert_eq!(first.fingerprint, second.fingerprint);
                    assert_eq!(first.identities, second.identities);
                    assert_eq!(first.active_identity, second.active_identity);
                    assert_eq!(first.anchors, second.anchors);
                    assert_eq!(first.personality, second.personality);
                    assert_eq!(first.synthesis, second.synthesis);
                    if cut == head {
                        assert_phy_head(&phy, &first);
                    }
                }
                eprintln!("Ego fixture {corpus}/phy: head {head}");
            }
        }
    }
}
