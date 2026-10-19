use super::{
    COMPACT_VERSION, Container, ContainerError, ContainerIdentity, FileKind, FormatVersion,
    ReliquaryScopeKind, encode_identity_header_for_version, read_header_for_version,
};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("container-format-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn write_bytes(path: &PathBuf, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

#[test]
fn compact_identity_headers_roundtrip_reliquary_and_phylactery() {
    let cases = [
        (
            ContainerIdentity {
                file_kind: FileKind::Reliquary,
                scope: Some(ReliquaryScopeKind::Project),
            },
            [0x5a; 16],
        ),
        (
            ContainerIdentity {
                file_kind: FileKind::Phylactery,
                scope: None,
            },
            [0xa5; 16],
        ),
    ];

    for (index, (identity, owner_uuid)) in cases.into_iter().enumerate() {
        let path = test_path(&format!("identity-{index}.cva"));
        let header = encode_identity_header_for_version(identity, owner_uuid, COMPACT_VERSION);
        assert_eq!(&header[..8], b"CVA\0\r\n\x1a\n");
        assert_eq!(&header[8..16], &[2, 0, 0, 0, 40, 0, 0, 0]);
        assert_eq!(&header[24..40], &owner_uuid);
        write_bytes(&path, &header);

        let mut file = File::open(&path).unwrap();
        assert_eq!(
            read_header_for_version(&mut file, COMPACT_VERSION).unwrap(),
            (COMPACT_VERSION, Some(identity), Some(owner_uuid), 40)
        );
        let opened = Container::open_compact(&path).unwrap();
        assert_eq!(opened.version(), COMPACT_VERSION);
        assert_eq!(opened.identity(), Some(identity));
        assert_eq!(opened.owner_uuid(), Some(owner_uuid));
    }
}

#[test]
fn legacy_open_rejects_compact_and_compact_open_accepts_it() {
    let path = test_path("compact.cva");
    let identity = ContainerIdentity {
        file_kind: FileKind::Reliquary,
        scope: None,
    };
    write_bytes(
        &path,
        &encode_identity_header_for_version(identity, [7; 16], COMPACT_VERSION),
    );

    assert!(matches!(
        Container::open(&path),
        Err(ContainerError::UnsupportedVersion(version)) if version == COMPACT_VERSION
    ));
    assert_eq!(
        Container::open_compact(&path).unwrap().version(),
        COMPACT_VERSION
    );
}

#[test]
fn compact_open_rejects_legacy_and_unknown_versions() {
    let legacy = test_path("legacy.cva");
    let mut legacy_header = encode_identity_header_for_version(
        ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: None,
        },
        [1; 16],
        FormatVersion { major: 1, minor: 0 },
    );
    write_bytes(&legacy, &legacy_header);
    assert!(matches!(
        Container::open_compact(&legacy),
        Err(ContainerError::UnsupportedVersion(FormatVersion {
            major: 1,
            minor: 0
        }))
    ));

    let unknown = test_path("unknown.cva");
    legacy_header[8..10].copy_from_slice(&2_u16.to_le_bytes());
    legacy_header[10..12].copy_from_slice(&1_u16.to_le_bytes());
    write_bytes(&unknown, &legacy_header);
    assert!(matches!(
        Container::open_compact(&unknown),
        Err(ContainerError::UnsupportedVersion(FormatVersion {
            major: 2,
            minor: 1
        }))
    ));
}

#[test]
fn strict_version_rejection_precedes_identity_and_payload_validation() {
    let path = test_path("wrong-version-malformed-identity.cva");
    let mut bytes = encode_identity_header_for_version(
        ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: None,
        },
        [2; 16],
        COMPACT_VERSION,
    )
    .to_vec();
    bytes[16] = 0xff;
    bytes.extend_from_slice(&[0xff, 0xff, 0xff]);
    write_bytes(&path, &bytes);

    let mut file = File::open(&path).unwrap();
    assert!(matches!(
        read_header_for_version(&mut file, FormatVersion { major: 1, minor: 0 }),
        Err(ContainerError::UnsupportedVersion(version)) if version == COMPACT_VERSION
    ));
    let mut visited = false;
    let result = Container::open_scanned::<ContainerError>(&path, |_, _, _| {
        visited = true;
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ContainerError::UnsupportedVersion(version)) if version == COMPACT_VERSION
    ));
    assert!(!visited, "visitor must not run before version rejection");
}
