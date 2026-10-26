use crate::{ArchiveError, Cva};
use std::fs;
use std::path::PathBuf;

fn test_path() -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-files-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join("archive.cva")
}

#[test]
fn files_round_trip_and_share_content_objects() {
    let path = test_path();
    let bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0xff, 0x00];
    let mut cva = Cva::create(&path).unwrap();

    let first = cva
        .store_file("framing-plan.png".into(), Some("image/png".into()), &bytes)
        .unwrap();
    assert_eq!(first.filename, "framing-plan.png");
    assert_eq!(first.mime_type.as_deref(), Some("image/png"));
    assert_eq!(first.byte_length, bytes.len() as u64);
    assert_eq!(cva.stats().files, 1);
    assert_eq!(cva.stats().content_objects, 1);
    assert_eq!(cva.archive_version(), 1);

    let same = cva
        .store_file("framing-plan.png".into(), Some("image/png".into()), &bytes)
        .unwrap();
    assert_eq!(same.id, first.id);
    assert_eq!(cva.stats().files, 1);
    assert_eq!(cva.stats().content_objects, 1);
    assert_eq!(cva.archive_version(), 1);

    let alias = cva
        .store_file("site-photo.png".into(), Some("image/png".into()), &bytes)
        .unwrap();
    assert_ne!(alias.id, first.id);
    assert_eq!(alias.content_id, first.content_id);
    assert_eq!(cva.stats().files, 2);
    assert_eq!(cva.stats().content_objects, 1);
    assert_eq!(cva.archive_version(), 2);
    assert_eq!(cva.file_bytes(first.id).unwrap(), bytes);

    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.stats().files, 2);
    assert_eq!(reopened.stats().content_objects, 1);
    assert_eq!(reopened.archive_version(), 2);
    assert_eq!(reopened.file(first.id).unwrap(), &first);
    assert_eq!(reopened.file(alias.id).unwrap(), &alias);
    assert_eq!(reopened.file_bytes(first.id).unwrap(), bytes);
    assert_eq!(reopened.file_bytes(alias.id).unwrap(), bytes);
}

#[test]
fn filename_search_is_incremental_reopenable_and_does_not_search_file_contents() {
    let path = test_path();
    let mut cva = Cva::create(&path).unwrap();

    cva.store_file(
        "framing-plan-rev3.pdf".into(),
        Some("application/pdf".into()),
        b"hidden-content-needle",
    )
    .unwrap();
    cva.store_file(
        "invoice-183.xlsx".into(),
        Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into()),
        b"invoice bytes",
    )
    .unwrap();

    let framing = cva.search_files("framing plan", 10).unwrap();
    assert_eq!(framing.len(), 1);
    assert_eq!(framing[0].file.filename, "framing-plan-rev3.pdf");
    assert!(framing[0].score > 0.0);
    assert!(
        cva.search_files("hidden content needle", 10)
            .unwrap()
            .is_empty()
    );

    cva.store_file(
        "structural-framing.pdf".into(),
        Some("application/pdf".into()),
        b"structural bytes",
    )
    .unwrap();
    let incremental = cva.search_files("structural", 10).unwrap();
    assert_eq!(incremental.len(), 1);
    assert_eq!(incremental[0].file.filename, "structural-framing.pdf");

    let pdfs = cva.search_files("pdf", 10).unwrap();
    assert_eq!(pdfs.len(), 2);
    assert!(pdfs.iter().all(|hit| hit.file.filename.ends_with(".pdf")));

    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(path).unwrap();
    let reopened_hits = reopened.search_files("framing", 10).unwrap();
    assert_eq!(reopened_hits.len(), 2);
    assert!(
        reopened_hits
            .iter()
            .any(|hit| hit.file.filename == "framing-plan-rev3.pdf")
    );
    assert!(
        reopened_hits
            .iter()
            .any(|hit| hit.file.filename == "structural-framing.pdf")
    );
}

#[test]
fn file_manifest_rejects_empty_names_and_mime_types() {
    let path = test_path();
    let mut cva = Cva::create(path).unwrap();

    assert!(matches!(
        cva.store_file(String::new(), None, b"data"),
        Err(ArchiveError::InvalidField("filename"))
    ));
    assert!(matches!(
        cva.store_file("source.bin".into(), Some(String::new()), b"data"),
        Err(ArchiveError::InvalidField("mime type"))
    ));
    assert_eq!(cva.stats().files, 0);
    assert_eq!(cva.stats().content_objects, 0);
}
