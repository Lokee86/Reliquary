use super::*;
use crate::{EntityDraft, Phylactery};
use std::{fs, path::PathBuf};

fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("entity-oracle-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn draft(name: &str, mutation: &str) -> EntityDraft {
    EntityDraft {
        canonical_name: name.into(),
        aliases: vec![format!("{name} alias")],
        kind: "person".into(),
        summary: format!("{name} exact summary"),
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn entity_cuts_keep_revision_merge_and_direct_terminal_history_exact() {
    let dir = scratch();
    let path = dir.join("owner.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let survivor = phy
        .publish_entity(None, 0, draft("Ada", "create-a"))
        .unwrap()
        .0;
    let before = entity_cut(&phy.container, &phy.entities, 1)
        .unwrap()
        .fingerprint;
    let mut revision = draft("Ada", "revise-a");
    revision.summary = "new exact summary".into();
    phy.publish_entity(Some(survivor.id), 1, revision).unwrap();
    let retired = phy
        .publish_entity(None, 0, draft("Other", "create-b"))
        .unwrap()
        .0;
    let before_merge = phy.entity_version();
    phy.merge_entities(survivor.id, retired.id, 3).unwrap();
    let merged = phy.entity_version();
    let cut = entity_cut(&phy.container, &phy.entities, merged).unwrap();
    assert_eq!(cut.replacements[&retired.id], Some(survivor.id));
    assert_eq!(
        cut.replacements[&retired.id],
        phy.retired_entity_replacement(retired.id)
    );
    assert!(!cut.current.contains_key(&retired.id));
    assert!(
        entity_cut(&phy.container, &phy.entities, before_merge)
            .unwrap()
            .current
            .contains_key(&retired.id)
    );
    assert_eq!(
        before,
        entity_cut(&phy.container, &phy.entities, 1)
            .unwrap()
            .fingerprint
    );
    let current = phy.entity(survivor.id).unwrap();
    phy.tombstone_entity(current.id, current.revision).unwrap();
    let head = phy.entity_version();
    let terminal = entity_cut(&phy.container, &phy.entities, head).unwrap();
    assert!(terminal.current.is_empty());
    assert_eq!(terminal.replacements[&survivor.id], None);
    assert_eq!(phy.entity_revision(survivor.id, 1).unwrap(), survivor);
    assert!(entity_cut(&phy.container, &phy.entities, head + 1).is_err());
    let expected: Vec<_> = (0..=head)
        .map(|cut| {
            entity_cut(&phy.container, &phy.entities, cut)
                .unwrap()
                .fingerprint
        })
        .collect();
    phy.sync().unwrap();
    drop(phy);
    let reopened = Phylactery::open(&path).unwrap();
    for (cut, expected) in expected.into_iter().enumerate() {
        assert_eq!(
            expected,
            entity_cut(&reopened.container, &reopened.entities, cut as u64)
                .unwrap()
                .fingerprint
        );
    }
    drop(reopened);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn entity_oracle_distinguishes_pruned_description_from_authored_empty_text() {
    let dir = scratch();
    let mut phy = Phylactery::create(dir.join("owner.phy")).unwrap();
    let entity = phy
        .publish_entity(None, 0, draft("Ada", "create"))
        .unwrap()
        .0;
    phy.tombstone_entity(entity.id, entity.revision).unwrap();
    let output = dir.join("pruned.phy");
    phy.prune_tombstoned_history(
        &output,
        crate::HistoryPruneRequest {
            targets: vec![crate::owner_lifecycle::LifecycleTarget::Entity(entity.id)],
        },
    )
    .unwrap();
    let pruned = Phylactery::open(&output).unwrap();
    let raw = &pruned.entities.records()[0];
    let value = revision_value(
        raw,
        true,
        pruned.container.transaction_time_ns(raw.global_version),
    );
    assert_eq!(value["pruned"], true);
    assert!(value["payload"].is_null());
    assert!(matches!(
        pruned.historical_entity_revision(entity.id, 1).unwrap(),
        crate::owner_lifecycle::HistoricalPayload::Pruned { .. }
    ));
    assert!(entity_cut(&pruned.container, &pruned.entities, pruned.entity_version()).is_ok());
    drop(pruned);
    drop(phy);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn authoritative_entity_cuts_fingerprint_deterministically() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("fixture root"));
    let dir = scratch();
    for (index, corpus) in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ]
    .iter()
    .enumerate()
    {
        for (kind, name) in [("rel", "project.prj.rel"), ("phy", "user.phy")] {
            let path = dir.join(format!("{index}.{kind}"));
            let mut source = fs::File::open(root.join(corpus).join(name)).unwrap();
            let mut destination = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            std::io::copy(&mut source, &mut destination).unwrap();
            drop(destination);
            let verify = |container: &Container, store: &EntityStore| {
                let head = store.entity_version();
                for cut in [0, head.min(1), head / 2, head] {
                    assert_eq!(
                        entity_cut(container, store, cut).unwrap().fingerprint,
                        entity_cut(container, store, cut).unwrap().fingerprint
                    );
                }
                assert_eq!(
                    entity_cut(container, store, head)
                        .unwrap()
                        .current
                        .keys()
                        .copied()
                        .collect::<Vec<_>>(),
                    store
                        .current()
                        .into_iter()
                        .map(|e| e.id)
                        .collect::<Vec<_>>()
                );
                eprintln!(
                    "Entity fixture {corpus}/{kind}: head {head}, {} revisions",
                    store.records().len()
                );
            };
            if kind == "rel" {
                let cva = crate::Cva::open(&path).unwrap();
                verify(&cva.container, &cva.entities);
            } else {
                let phy = Phylactery::open(&path).unwrap();
                verify(&phy.container, &phy.entities);
            }
        }
    }
    fs::remove_dir_all(dir).unwrap();
}
