//! Step 26: retained live receipt authority across a later nonempty import.
use super::*;

#[test]
fn later_import_preserves_prestart_accepted_use_without_replacing_live_memory() {
    let (path, mut cva, _) = setup_origin("later-import-accepted-use.rel", EpisodeOrigin::Live);
    let old = process(&mut cva, 1, 110).created[0].id;
    cva.freshness_admit(old, cva.rel_turn_count()).unwrap();
    settle_initial_pass(&mut cva, old, 120);

    // Make lost or duplicated credit observable rather than hidden at score 100.
    let mut parent = None;
    for index in 0..300 {
        let id = format!("decay-{index}");
        cva.append_node(
            id.clone(),
            "live-decay".into(),
            parent,
            "user".into(),
            130 + index,
            "Live activity before the later import.",
        )
        .unwrap();
        parent = Some(id);
    }
    let cut = cva.rel_turn_count();
    let without_use = cva.freshness_record_at(old, cut).unwrap().unwrap();
    let receipt = crate::AcceptedMemoryUseReceipt {
        owner_uuid: cva.owner_uuid().unwrap(),
        accepted_turn: cut,
        use_id: "live-use-before-later-import".into(),
        memories: vec![old],
    };
    assert_eq!(
        cva.record_accepted_memory_use(receipt.clone(), 500)
            .unwrap(),
        1
    );
    let live = cva.freshness_record_at(old, cut).unwrap().unwrap();
    assert_eq!(
        live.score,
        without_use.score + FreshnessPolicy::default_v1().access_principal
    );
    assert!(
        live.score < 100,
        "fixture must expose duplicate reinforcement"
    );
    let resident = cva.freshness_record(old).unwrap();
    cva.sync().unwrap();
    let pre_import_bytes = std::fs::read(&path).unwrap();
    drop(cva);
    let mut cva = Cva::open(&path).unwrap();
    assert_eq!(cva.freshness_record_at(old, cut).unwrap(), Some(live));

    cva.import_native(&imported_snapshot("later-import-after-live-use"), 600)
        .unwrap();
    let scope = cva.import_epoch_scope().unwrap();
    assert_eq!(scope.base_cut, cut);
    assert_eq!(cva.historical_import_base(&scope).unwrap()[&old], live);
    process_import_episode(&mut cva, 610);
    for id in cva.import_epoch_scope().unwrap().cohort {
        settle_initial_pass(&mut cva, id, 620);
    }
    let frozen = cva.freeze_historical_import().unwrap();
    assert!(
        !frozen.settlement.cohort.is_empty(),
        "exercise a real import birth"
    );
    assert!(!frozen.settlement.cohort.contains(&old));
    assert!(frozen.settlement.superseded.is_empty());
    let prepared = frozen.prepare_native().unwrap();
    assert!(!prepared.records().contains_key(&old));
    let mut expected = live;
    expected
        .settle_with_policy(
            prepared.settlement.settlement_cut,
            &FreshnessPolicy::default_v1(),
        )
        .unwrap();
    let mut counterfactual = without_use;
    counterfactual
        .settle_with_policy(
            prepared.settlement.settlement_cut,
            &FreshnessPolicy::default_v1(),
        )
        .unwrap();
    assert_ne!(expected, counterfactual, "lost use must fail this fixture");
    assert!(cva.commit_historical_import(&prepared).unwrap());
    assert_eq!(
        cva.freshness_record(old),
        Some(resident),
        "unaffected resident record is not rewritten"
    );
    assert_eq!(cva.freshness_record_at(old, cut).unwrap(), Some(live));
    assert_eq!(
        cva.freshness_record_at(old, prepared.settlement.settlement_cut)
            .unwrap(),
        Some(expected)
    );
    let settled_bytes = std::fs::read(&path).unwrap();
    assert!(
        settled_bytes.starts_with(&pre_import_bytes),
        "original live receipts remain byte-for-byte"
    );
    drop(cva);

    let mut cva = Cva::open(&path).unwrap();
    assert_eq!(cva.freshness_record_at(old, cut).unwrap(), Some(live));
    assert_eq!(
        cva.freshness_record_at(old, prepared.settlement.settlement_cut)
            .unwrap(),
        Some(expected)
    );
    assert!(
        !cva.reconstruct_historical_freshness(Default::default())
            .unwrap()
            .committed
    );
    // Test-only scope permits checking durable receipt idempotency while normal
    // open-epoch admission remains closed until the later completion steps.
    cva.container.set_owner_write_scope(true);
    let retried = cva.record_accepted_memory_use(receipt, 700);
    cva.container.set_owner_write_scope(false);
    assert_eq!(retried.unwrap(), 0);
    assert_eq!(std::fs::read(&path).unwrap(), settled_bytes);
    assert_eq!(
        cva.freshness_record_at(old, prepared.settlement.settlement_cut)
            .unwrap(),
        Some(expected)
    );
    drop(cva);
    std::fs::remove_file(path).unwrap();
}
