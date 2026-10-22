use super::test_path;
use crate::ego_codec::EgoRecord;
use crate::ego_current_fixture::encode_postimage;
use crate::{Cva, CvaError, EgoError, Phylactery};

#[test]
fn phylactery_persona_records_round_trip() {
    let path = test_path("persona.phy");
    let mut phy = Phylactery::create(&path).unwrap();

    let (identity, created) = phy.put_ego_identity(0, "You are Jarvis.".into()).unwrap();
    assert!(created);
    assert_eq!(identity.revision, 1);
    assert_eq!(phy.ego_version(), 1);

    let (personality, created) = phy
        .put_ego_personality(
            0,
            "Be direct, concise, and willing to challenge weak assumptions.".into(),
            0,
        )
        .unwrap();
    assert!(created);
    assert_eq!(personality.revision, 1);
    assert_eq!(personality.source_memory_version, 0);
    assert_eq!(phy.ego_version(), 2);

    let (personality, changed) = phy
        .put_ego_personality(
            1,
            "Be direct, concise, and willing to challenge weak assumptions.".into(),
            0,
        )
        .unwrap();
    assert!(!changed);
    assert_eq!(personality.revision, 1);
    assert_eq!(phy.ego_version(), 2);

    phy.sync().unwrap();
    drop(phy);

    let reopened = Phylactery::open(&path).unwrap();
    assert_eq!(reopened.ego_version(), 2);
    assert_eq!(reopened.ego_identity().unwrap().text, "You are Jarvis.");
    assert_eq!(reopened.ego_personality().unwrap().source_memory_version, 0);
}

#[test]
fn web_synthesis_records_source_memory_version_and_noops() {
    let path = test_path("synthesis.rel");
    let mut rel = Cva::create(&path).unwrap();

    let (synthesis, changed) = rel
        .put_ego_web_synthesis(0, 0, "Current bounded REL synthesis.".into())
        .unwrap();
    assert!(changed);
    assert_eq!(synthesis.revision, 1);
    assert_eq!(synthesis.source_memory_version, 0);
    let version = rel.ego_version();

    let (_, changed) = rel
        .put_ego_web_synthesis(1, 0, "Current bounded REL synthesis.".into())
        .unwrap();
    assert!(!changed);
    assert_eq!(rel.ego_version(), version);

    rel.sync().unwrap();
    drop(rel);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.ego_web_synthesis().unwrap(), &synthesis);
}

#[test]
fn source_memory_watermarks_cannot_point_past_owner_state() {
    let rel_path = test_path("future-synthesis.rel");
    let mut rel = Cva::create(&rel_path).unwrap();
    assert!(matches!(
        rel.put_ego_web_synthesis(0, 1, "Impossible future synthesis.".into()),
        Err(CvaError::Ego(EgoError::InvalidSourceMemoryVersion {
            source: 1,
            current: 0
        }))
    ));
    assert_eq!(rel.ego_version(), 0);

    let phy_path = test_path("future-personality.phy");
    let mut phy = Phylactery::create(&phy_path).unwrap();
    assert!(matches!(
        phy.put_ego_personality(0, "Impossible future personality.".into(), 1),
        Err(crate::PhylacteryError::Ego(
            EgoError::InvalidSourceMemoryVersion {
                source: 1,
                current: 0
            }
        ))
    ));
    assert_eq!(phy.ego_version(), 0);

    let corrupt_path = test_path("corrupt-future-synthesis.rel");
    let mut corrupt = Cva::create(&corrupt_path).unwrap();
    let payload = encode_postimage(&EgoRecord::Synthesis {
        ego_version: 1,
        revision: 1,
        source_memory_version: 1,
        text: "Invalid persisted future synthesis.".into(),
    })
    .unwrap();
    corrupt.container.append(&payload).unwrap();
    corrupt.sync().unwrap();
    drop(corrupt);
    assert!(matches!(
        Cva::open(&corrupt_path),
        Err(CvaError::Ego(EgoError::InvalidSourceMemoryVersion {
            source: 1,
            current: 0
        }))
    ));
}

#[test]
fn revisions_are_guarded_and_rel_rejects_phy_persona_records() {
    let phy_path = test_path("revision.phy");
    let mut phy = Phylactery::create(&phy_path).unwrap();
    phy.put_ego_identity(0, "You are Jarvis.".into()).unwrap();
    assert!(matches!(
        phy.put_ego_identity(0, "You are Optimus Prime.".into()),
        Err(crate::PhylacteryError::Ego(EgoError::RevisionConflict {
            expected: 0,
            actual: 1
        }))
    ));

    let rel_path = test_path("foreign-persona.rel");
    let mut rel = Cva::create(&rel_path).unwrap();
    let payload = encode_postimage(&EgoRecord::Identity {
        ego_version: 1,
        revision: 1,
        text: "This does not belong in a REL.".into(),
        id: crate::ego_store::DEFAULT_IDENTITY_ID,
        deleted: false,
        activate: true,
        name: "Default".into(),
    })
    .unwrap();
    rel.container.append(&payload).unwrap();
    rel.sync().unwrap();
    drop(rel);

    assert!(matches!(
        Cva::open(&rel_path),
        Err(CvaError::Ego(EgoError::InvalidOwnerRecord(_)))
    ));
}
