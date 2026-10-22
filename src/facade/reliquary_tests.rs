use crate::{Reliquary, ReliquaryScopeKind};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("reliquary-scope-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn new_reliquary_has_generic_rel_identity() {
    let path = test_path("project.rel");
    let rel = Reliquary::create(&path).unwrap();
    let owner_id = rel.owner_id().unwrap();
    assert!(owner_id.starts_with("rel-"));
    assert_eq!(rel.legacy_scope_kind(), None);
    assert_eq!(rel.rel_metadata().type_label, None);
    drop(rel);

    let header = fs::read(&path).unwrap();
    assert_eq!(u32::from_le_bytes(header[12..16].try_into().unwrap()), 40);
    assert_eq!(header[16], 1);
    assert_eq!(header[17], 0);

    let reopened = Reliquary::open(&path).unwrap();
    assert_eq!(reopened.owner_id().as_deref(), Some(owner_id.as_str()));
    assert_eq!(reopened.legacy_scope_kind(), None);
}

#[test]
fn rel_type_is_metadata_not_file_identity() {
    let organization = test_path("acme.rel");
    let project = test_path("warlock.rel");

    let organization = Reliquary::create_organization(&organization).unwrap();
    let project = Reliquary::create_project(&project).unwrap();

    assert!(organization.owner_id().unwrap().starts_with("rel-"));
    assert!(project.owner_id().unwrap().starts_with("rel-"));
    assert_eq!(organization.legacy_scope_kind(), None);
    assert_eq!(project.legacy_scope_kind(), None);
    assert_eq!(
        organization.rel_metadata().type_label.as_deref(),
        Some("Organization")
    );
    assert_eq!(
        project.rel_metadata().type_label.as_deref(),
        Some("Project")
    );
}

#[test]
fn rel_dependencies_round_trip_as_metadata() {
    let path = test_path("nested.rel");
    let mut rel = Reliquary::create_project(&path).unwrap();
    rel.set_rel_metadata(
        Some("Subproject".into()),
        vec!["rel-b".into(), "rel-a".into(), "rel-b".into()],
    )
    .unwrap();
    rel.sync().unwrap();
    drop(rel);

    let reopened = Reliquary::open(&path).unwrap();
    assert_eq!(
        reopened.rel_metadata(),
        crate::RelMetadata {
            type_label: Some("Subproject".into()),
            dependencies: vec!["rel-a".into(), "rel-b".into()],
        }
    );
}

#[test]
fn rel_cannot_depend_on_itself() {
    let path = test_path("self.rel");
    let mut rel = Reliquary::create(&path).unwrap();
    let owner_id = rel.owner_id().unwrap();
    assert!(
        rel.set_rel_metadata(None, vec![owner_id])
            .unwrap_err()
            .to_string()
            .contains("cannot depend on itself")
    );
}
