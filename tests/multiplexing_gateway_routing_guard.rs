//! Cheap local enforcement of the gateway/legacy call-direction boundary.
//! This source guard complements runtime routing and shared-mount claim tests;
//! it does not replace the per-method authorization audit.
use std::path::Path;

fn forbidden_dependency(source: &str) -> Option<&'static str> {
    let identifiers = source.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_');
    for identifier in identifiers {
        if matches!(
            identifier,
            "ReliquaryRuntimeHost"
                | "active_execution"
                | "OwnerExecution"
                | "RuntimeMemoryProfiles"
                | "PerceptionOwner"
        ) {
            return Some("legacy host authority");
        }
    }
    let compact: String = source.chars().filter(|ch| !ch.is_whitespace()).collect();
    if compact.contains(".active_rel_id()")
        || compact.contains("::active_rel_id()")
        || compact.contains(".phylactery_owner_id(")
        || compact.contains("::phylactery_owner_id(")
    {
        return Some("global REL selection");
    }
    None
}

#[test]
fn gateway_sources_do_not_depend_on_legacy_selection_authority() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runtime_inference");
    let mut audited = Vec::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if ![
            "runtime_host_gateway",
            "runtime_host_rel_",
            "runtime_host_phy_",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
            || !name.ends_with(".rs")
            || name.contains("tests")
        {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            forbidden_dependency(&source),
            None,
            "forbidden gateway dependency in {}",
            path.display()
        );
        audited.push(name.to_owned());
    }
    // Discovery includes future gateway and concrete owner modules automatically.
    assert!(
        audited
            .iter()
            .any(|name| name == "runtime_host_rel_execution.rs")
    );
    assert!(
        audited
            .iter()
            .any(|name| name == "runtime_host_phy_execution.rs")
    );
    assert!(audited.iter().any(|name| name == "runtime_host_gateway.rs"));
    assert!(
        audited
            .iter()
            .any(|name| name == "runtime_host_gateway_mount.rs")
    );
    assert!(
        audited
            .iter()
            .any(|name| name == "runtime_host_gateway_conversation.rs")
    );
}

#[test]
fn guard_distinguishes_instance_selection_from_legacy_authority() {
    assert_eq!(
        forbidden_dependency("self.active_rel_id(instance)?; ReliquaryRuntimeHostError"),
        None
    );
    for source in [
        "ReliquaryRuntimeHost::new()",
        "use crate::ReliquaryRuntimeHost as Host;",
        "host.active_execution()",
        "OwnerExecution::new()",
        "RuntimeMemoryProfiles::default()",
        "PerceptionOwner::User",
        "host.phylactery_owner_id()",
        "host.active_rel_id(\n )",
    ] {
        assert!(forbidden_dependency(source).is_some(), "{source}");
    }
}

#[test]
fn runtime_sources_have_no_ambient_host_selector() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runtime_inference");
    let mut directories = vec![root];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                directories.push(path);
                continue;
            }
            if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            for identifier in source.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_') {
                assert!(
                    !matches!(
                        identifier,
                        "ReliquaryRuntimeHost"
                            | "LegacyManagedSessionState"
                            | "active_key"
                            | "managed_sessions"
                            | "active_execution"
                            | "active_insomnia_enabled"
                            | "OwnerExecution"
                            | "RuntimeMemoryProfiles"
                            | "PerceptionOwner"
                            | "PerceptionQueue"
                            | "phylactery_enabled"
                            | "acquire_legacy_rel"
                            | "acquire_legacy_phy"
                            | "legacy_container"
                    ),
                    "removed ambient authority {identifier} in {}",
                    path.display()
                );
            }
        }
    }
}
