use super::*;
use reliquary_owner_session::{OwnerScope, RelWriter};
use std::{cell::Cell, fs, path::PathBuf};

#[test]
fn raw_branch_selection_resumes_after_interrupted_staging() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("oc03-raw-import-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("conversations.json");
    let rel = dir.join("owner.rel");
    fs::write(&path, r#"[{
        "id": "raw-conversation",
        "current_node": "main",
        "mapping": {
            "hidden": {"id":"hidden", "parent":"a", "children":["main","alternate"],
                "message":{"id":"hidden-message","channel":"analysis","author":{"role":"assistant"},
                    "create_time":2,"content":{"content_type":"text","parts":["private reasoning"]}}},
            "alternate": {"parent":"hidden", "children":[],
                "message":{"id":"alternate-message","author":{"role":"assistant"},"create_time":3,
                    "content":{"content_type":"text","parts":["alternate"]}}},
            "main": {"parent":"hidden", "children":["outside"],
                "message":{"id":"main-message","author":{"role":"assistant"},"create_time":3.000000001,
                    "content":{"content_type":"text","parts":["main"]}}},
            "a": {"parent":null, "children":["hidden"],
                "message":{"id":"user-message","author":{"role":"user"},"create_time":1,
                    "content":{"content_type":"multimodal_text","parts":["question",{"asset_pointer":"unused"}]}}},
            "outside": {"parent":"main", "children":[],
                "message":{"id":"outside-message","author":{"role":"assistant"},"create_time":4,
                    "content":{"content_type":"text","parts":["outside the half-open window"]}}}
        }
    }]"#).unwrap();
    let window =
        NativeImportWindow::from_rfc3339("1970-01-01T00:00:01Z", "1970-01-01T00:00:04Z").unwrap();
    let plan =
        || NativeImportPlan::from_path(&path, NativeImportFormat::ChatGpt, Some(window)).unwrap();
    let owner = [0x31; 16];
    let id = plan().import_id(owner);
    assert_eq!(plan().coverage().turns, 3);
    assert_eq!(plan().coverage().branches, 3);
    assert_eq!(plan().coverage().episodes, 2);
    assert_eq!(plan().coverage().omitted_hidden_messages, 1);
    assert_eq!(plan().coverage().omitted_outside_window, 1);
    assert_eq!(plan().coverage().omitted_asset_references, 1);
    assert_eq!(plan().coverage().remapped_parents, 2);
    {
        let mut writer = RelWriter::create_current_path(&rel, OwnerScope::Project, owner).unwrap();
        let calls = Cell::new(0);
        let interrupted = plan().stage(&mut writer, "authenticated", || {
            calls.set(calls.get() + 1);
            calls.get() <= 2
        });
        assert!(matches!(interrupted, Err(NativeImportStageError::Revoked)));
        assert_eq!(
            native_import_status(writer.read_session(), &id)
                .unwrap()
                .state,
            NativeImportState::Staging
        );
        assert_eq!(writer.read_session().rel_turn_count(), 1);
    }
    {
        let mut writer = RelWriter::open_path_current(&rel).unwrap();
        assert!(matches!(
            plan().stage(&mut writer, "different", || true),
            Err(NativeImportStageError::Conflict)
        ));
        let resumed = plan().stage(&mut writer, "authenticated", || true).unwrap();
        assert_eq!(resumed.import_id, id);
        assert_eq!(writer.read_session().rel_turn_count(), 3);
        assert_eq!(
            native_import_status(writer.read_session(), &id)
                .unwrap()
                .state,
            NativeImportState::Staged
        );
        let receipt = writer
            .read_session()
            .native_import_receipt(&plan().import_key)
            .unwrap();
        let sources = &receipt.conversations[0].messages;
        assert_eq!(sources[0].source_id, "user-message");
        assert_eq!(
            sources
                .iter()
                .find(|message| message.source_id == "main-message")
                .unwrap()
                .source_parent_id
                .as_deref(),
            Some("user-message")
        );
        let main = writer
            .read_session()
            .archive_node("raw-conversation", "main")
            .unwrap();
        assert_eq!(main.timestamp_ns, 3_000_000_001);
        assert_eq!(main.parent_id.as_deref(), Some("a"));
    }
    fs::remove_dir_all(dir).unwrap();
}
