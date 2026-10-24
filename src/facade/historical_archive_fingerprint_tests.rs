use super::*;
use std::{fs, path::PathBuf};

fn scratch() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("archive-oracle-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn archive_cuts_agree_with_branch_history_and_ignore_inert_backing() {
    let dir = scratch();
    let path = dir.join("owner.rel");
    let mut cva = Cva::create(&path).unwrap();
    cva.append_node("a".into(), "c".into(), None, "user".into(), 1, "exact é")
        .unwrap();
    cva.append_branch(Branch {
        id: "main".into(),
        conversation_id: "c".into(),
        leaf_node_id: "a".into(),
        canonical: true,
    })
    .unwrap();
    let first = cva.archive_version();
    let baseline = archive_cut(&mut cva, first).unwrap().fingerprint;
    cva.append_node(
        "b".into(),
        "c".into(),
        Some("a".into()),
        "assistant".into(),
        2,
        "next",
    )
    .unwrap();
    cva.append_branch(Branch {
        id: "main".into(),
        conversation_id: "c".into(),
        leaf_node_id: "b".into(),
        canonical: true,
    })
    .unwrap();
    for cut in 0..=cva.archive_version() {
        let projected = archive_cut(&mut cva, cut).unwrap();
        assert_eq!(
            projected
                .branches
                .get(&("c".into(), "main".into()))
                .cloned(),
            cva.branch_at("c", "main", cut).unwrap()
        );
    }
    assert_eq!(baseline, archive_cut(&mut cva, first).unwrap().fingerprint);
    let head = cva.archive_version();
    let latest = archive_cut(&mut cva, head).unwrap().fingerprint;
    assert_ne!(baseline.sha256, latest.sha256);
    cva.container
        .append(
            &crate::archive_codec::encode_branch(&Branch {
                id: "inert".into(),
                conversation_id: "c".into(),
                leaf_node_id: "a".into(),
                canonical: false,
            })
            .unwrap(),
        )
        .unwrap();
    assert_eq!(latest, archive_cut(&mut cva, head).unwrap().fingerprint);
    assert!(archive_cut(&mut cva, head + 1).is_err());
    cva.sync().unwrap();
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(
        latest,
        archive_cut(&mut reopened, head).unwrap().fingerprint
    );
    drop(reopened);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn archive_fingerprint_survives_object_relocation() {
    let dir = scratch();
    let mut cva = Cva::create(dir.join("owner.rel")).unwrap();
    let orphan = b"unreferenced Archive content before accepted content";
    cva.owner
        .archive
        .put_content_bytes(
            &mut cva.owner.container,
            crate::archive_store::hash_content(orphan),
            orphan,
        )
        .unwrap();
    cva.append_node("a".into(), "c".into(), None, "user".into(), 1, "body")
        .unwrap();
    cva.sync().unwrap();
    let head = cva.archive_version();
    let before = archive_cut(&mut cva, head).unwrap().fingerprint;
    let output = dir.join("reclaimed.rel");
    let source_location = cva.archive.record_versions()[0].record.legacy_bytes();
    let report = cva.reclaim_storage(&output).unwrap();
    assert_eq!(report.orphan_content_chunks, 1);
    let mut copy = Cva::open(&output).unwrap();
    assert_ne!(
        source_location,
        copy.archive.record_versions()[0].record.legacy_bytes()
    );
    assert_eq!(before, archive_cut(&mut copy, head).unwrap().fingerprint);
    drop(copy);
    drop(cva);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn archive_terminal_closure_keeps_prior_cuts_exact() {
    let dir = scratch();
    let path = dir.join("owner.rel");
    let mut cva = Cva::create(&path).unwrap();
    cva.append_node(
        "a".into(),
        "c".into(),
        None,
        "user".into(),
        1,
        "retained body",
    )
    .unwrap();
    let prior = cva.archive_version();
    let before = archive_cut(&mut cva, prior).unwrap().fingerprint;
    cva.tombstone_conversation("c").unwrap();
    let head = cva.archive_version();
    assert_eq!(before, archive_cut(&mut cva, prior).unwrap().fingerprint);
    let terminal = archive_cut(&mut cva, head).unwrap().fingerprint;
    assert_ne!(before.sha256, terminal.sha256);
    cva.sync().unwrap();
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(
        before,
        archive_cut(&mut reopened, prior).unwrap().fingerprint
    );
    assert_eq!(
        terminal,
        archive_cut(&mut reopened, head).unwrap().fingerprint
    );
    drop(reopened);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn authoritative_archive_cuts_fingerprint_deterministically() {
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
        let path = dir.join(format!("{index}.rel"));
        // Copy bytes only: fs::copy also preserves the authoritative fixture's
        // Windows read-only attribute, while normal Cva open needs a writable
        // disposable destination for its existing tail-recovery behavior.
        let mut source = fs::File::open(root.join(corpus).join("project.prj.rel")).unwrap();
        let mut destination = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::copy(&mut source, &mut destination).unwrap();
        drop(destination);
        let mut cva = Cva::open(&path).unwrap();
        let head = cva.archive_version();
        for cut in [0, 1, head / 2, head] {
            eprintln!("Archive fixture {corpus}: fingerprint cut {cut}/{head}");
            let first = archive_cut(&mut cva, cut).unwrap().fingerprint;
            assert_eq!(first, archive_cut(&mut cva, cut).unwrap().fingerprint);
        }
    }
    fs::remove_dir_all(dir).unwrap();
}
