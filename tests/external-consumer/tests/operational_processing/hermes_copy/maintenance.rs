use super::*;

pub(super) fn assert_divergent_copy_preserves_receipts(
    source: &std::path::Path,
    root: &std::path::Path,
    first: [u8; 32],
    second: [u8; 32],
) {
    let left = root.join("left.rel");
    let right = root.join("right.rel");
    for (path, label) in [(&left, "Left project note"), (&right, "Right project note")] {
        fs::copy(source, path).unwrap();
        let rel = Rel::open(path).unwrap();
        let (owner, instance, _) = mount(&rel);
        instance
            .workspace_management(owner)
            .unwrap()
            .create_memory(
                label.into(),
                label.into(),
                "fact".into(),
                "project".into(),
                3_000_000_000,
            )
            .unwrap();
    }
    let merged = root.join("merged.rel");
    let report = reliquary_memory::reconcile_rel(&left, &right, &merged).unwrap();
    assert_eq!(
        report.relation,
        reliquary_memory::ReconciliationRelation::Diverged
    );
    let rel = Rel::open(&merged).unwrap();
    assert_eq!(rel.info().unwrap().memory_count, 3);
    let (owner, instance, _) = mount(&rel);
    let scope = HermesCopyScope::new("profile".into(), "session".into()).unwrap();
    for (uid, source_key) in [("accepted", first), ("next", second)] {
        assert_eq!(
            inspect(&instance, owner, &scope, uid)
                .imported
                .unwrap()
                .source_digest,
            source_key
        );
    }
}
