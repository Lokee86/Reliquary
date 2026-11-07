//! Shared test-only replay of the authored G4 native 2.1 conversation cohort.

#[cfg(test)]
use super::*;
use serde_json::Value;
use std::path::Path;

pub(crate) struct ManifestReplay {
    pub(crate) _lantern_queue: ConversationQueue,
    pub(crate) _cascade_queue: ConversationQueue,
}

pub(crate) fn replay_authored_manifest(
    instance: &Instance,
    root: &Path,
    lantern: OwnerId,
    cascade: OwnerId,
    principal: &str,
) -> ManifestReplay {
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/g4-native-rel21-v1.json"
    ))
    .unwrap();
    assert_eq!(manifest["fixture_seed"], "g4-rel21-v1");
    let mut lantern_queue = instance
        .open_conversation_queue(root.join("manifest-lantern.queue"), lantern)
        .unwrap();
    let mut cascade_queue = instance
        .open_conversation_queue(root.join("manifest-cascade.queue"), cascade)
        .unwrap();
    let mut timestamp = 1_i64;
    for script in manifest["conversation_scripts"].as_array().unwrap() {
        let conversation = script["conversation_id"].as_str().unwrap();
        let turns = script["turns"].as_array().unwrap();
        let branch = script["branches"]
            .as_array()
            .and_then(|items| items.first());
        let queue = if script["owner"] == "rel-lantern" {
            &mut lantern_queue
        } else {
            &mut cascade_queue
        };
        for (index, turn) in turns.iter().enumerate() {
            let node = turn["id"].as_str().unwrap();
            let submission = SubmissionId::new(node).unwrap();
            let id = ConversationSubmissionId::new(conversation, submission.clone()).unwrap();
            let revision = queue.conversation_revision(conversation);
            instance
                .submit_conversation(
                    queue,
                    conversation,
                    submission,
                    turn["content"].as_str().unwrap(),
                    revision,
                )
                .unwrap();
            let revision = queue.conversation_revision(conversation);
            let (branch_id, parent_node_id) = manifest_branch(turns, branch, index, node);
            instance
                .finalize_conversation_submission(
                    queue,
                    &id,
                    revision,
                    FinalizeCapture {
                        branch_id,
                        parent_node_id,
                        committed_timestamp_ns: timestamp,
                        verified_principal_id: principal.to_owned(),
                    },
                )
                .unwrap();
            timestamp += 1;
        }
    }
    assert_eq!(lantern_queue.conversation_revision("conv-l01"), 12);
    assert_eq!(cascade_queue.conversation_revision("conv-c04"), 12);
    assert_eq!(
        instance
            .with_writable_reliquary(lantern, |rel| rel.accepted_turn_sources().len())
            .unwrap(),
        32
    );
    assert_eq!(
        instance
            .with_writable_reliquary(cascade, |rel| rel.accepted_turn_sources().len())
            .unwrap(),
        16
    );
    ManifestReplay {
        _lantern_queue: lantern_queue,
        _cascade_queue: cascade_queue,
    }
}

fn manifest_branch(
    turns: &[Value],
    branch: Option<&Value>,
    turn_index: usize,
    node: &str,
) -> (String, Option<String>) {
    let Some(branch) = branch else {
        return (
            "main".into(),
            turn_index
                .checked_sub(1)
                .map(|index| turns[index]["id"].as_str().unwrap().to_owned()),
        );
    };
    let children = branch["child_turn_ids"].as_array().unwrap();
    if children.iter().any(|child| child.as_str() == Some(node)) {
        let canonical = branch["canonical_child_id"].as_str().unwrap();
        return (
            if node == canonical {
                "main".to_owned()
            } else {
                format!("{}-{node}", branch["id"].as_str().unwrap())
            },
            Some(branch["parent_turn_id"].as_str().unwrap().to_owned()),
        );
    }
    (
        "main".into(),
        turn_index
            .checked_sub(1)
            .map(|index| turns[index]["id"].as_str().unwrap().to_owned()),
    )
}
