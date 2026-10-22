use super::{
    Container, ContainerError, ContainerIdentity, FileKind, FormatVersion, ReliquaryScopeKind,
};
use std::{fs, path::PathBuf};
fn path() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("current-container-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    root.join("owner.cva")
}
#[test]
fn current_identity_headers_preserve_typed_scope_uuid_and_kind() {
    for identity in [
        ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: None,
        },
        ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: Some(ReliquaryScopeKind::Project),
        },
        ContainerIdentity {
            file_kind: FileKind::Phylactery,
            scope: None,
        },
    ] {
        let path = path();
        let owner = [0x5a; 16];
        drop(Container::create_with_identity_and_uuid(&path, identity, owner).unwrap());
        let value = Container::open(&path).unwrap();
        assert_eq!(value.version(), FormatVersion { major: 2, minor: 0 });
        assert_eq!(value.identity(), Some(identity));
        assert_eq!(value.owner_uuid(), Some(owner));
        assert_eq!(value.header_len, 40);
    }
}
#[test]
fn bare_current_container_header_is_for_physical_storage() {
    let path = path();
    drop(Container::create(&path).unwrap());
    let opened = Container::open(&path).unwrap();
    assert_eq!(opened.version(), FormatVersion { major: 2, minor: 0 });
    assert_eq!(opened.identity(), None);
    assert_eq!(opened.header_len, 16);
}
#[test]
fn retired_identityless_header_version_is_unsupported_without_repair() {
    let path = path();
    drop(Container::create(&path).unwrap());
    let mut bytes = fs::read(&path).unwrap();
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&[1, 2, 3]);
    fs::write(&path, &bytes).unwrap();
    assert!(matches!(
        Container::open(&path),
        Err(ContainerError::UnsupportedVersion(FormatVersion {
            major: 1,
            minor: 0
        }))
    ));
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
