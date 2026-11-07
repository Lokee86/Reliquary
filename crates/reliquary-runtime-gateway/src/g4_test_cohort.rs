//! Shared test-only replay of the authored G4 native 2.1 conversation cohort.

#[cfg(test)]
use super::*;
use serde_json::Value;
use std::path::Path;

pub(crate) struct ManifestReplay {
    pub(crate) _lantern_queue: ConversationQueue,
    pub(crate) _cascade_queue: ConversationQueue,
    pub(crate) historical: Vec<(
        String,
        OwnerId,
        reliquary_owner_session::RelHistoricalCut,
        reliquary_owner_session::HistoricalOwnerProof,
    )>,
}

pub(crate) fn replay_authored_manifest(
    instance: &Instance,
    root: &Path,
    lantern: OwnerId,
    cascade: OwnerId,
    principal: &str,
) -> ManifestReplay {
    assert_eq!(instance.principal(), principal);
    replay_scoped_authored_manifest(instance, instance, root, lantern, cascade)
}

pub(crate) fn replay_scoped_authored_manifest(
    lantern_instance: &Instance,
    cascade_instance: &Instance,
    root: &Path,
    lantern: OwnerId,
    cascade: OwnerId,
) -> ManifestReplay {
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/g4-native-rel21-v1.json"
    ))
    .unwrap();
    assert_eq!(manifest["fixture_seed"], "g4-rel21-v1");
    let mut lantern_queue = lantern_instance
        .open_conversation_queue(root.join("manifest-lantern.queue"), lantern)
        .unwrap();
    let mut cascade_queue = cascade_instance
        .open_conversation_queue(root.join("manifest-cascade.queue"), cascade)
        .unwrap();
    let mut timestamp = 1_i64;
    let mut historical = Vec::new();
    for script in manifest["conversation_scripts"].as_array().unwrap() {
        let conversation = script["conversation_id"].as_str().unwrap();
        let turns = script["turns"].as_array().unwrap();
        let branch = script["branches"]
            .as_array()
            .and_then(|items| items.first());
        let (queue, instance) = if script["owner"] == "rel-lantern" {
            (&mut lantern_queue, lantern_instance)
        } else {
            (&mut cascade_queue, cascade_instance)
        };
        for (index, turn) in turns.iter().enumerate() {
            let node = turn["id"].as_str().unwrap();
            let owner = if script["owner"] == "rel-lantern" {
                lantern
            } else {
                cascade
            };
            for cut in manifest["logical_history_cuts"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|cut| cut["before_turn"].as_str() == Some(node))
            {
                historical.push(capture_cut(instance, owner, cut["id"].as_str().unwrap()));
            }
            let (branch_id, parent_node_id) = manifest_branch(turns, branch, index, node);
            let fork_child = branch.is_some_and(|fork| {
                fork["child_turn_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|child| child.as_str() == Some(node))
            });
            if fork_child {
                // G4-T02 permits accepted owner-session fixture entry points.
                // This proves authored reply persistence, not live host output.
                instance
                    .with_rel_writer(owner, |writer| {
                        writer.accept_turn_and_advance_branch(
                            reliquary_owner_session::model::TurnInput {
                                id: node.into(),
                                conversation_id: conversation.into(),
                                parent_id: parent_node_id,
                                role: turn["speaker"].as_str().unwrap().into(),
                                principal_id: Some(instance.principal().to_owned()),
                                timestamp_ns: timestamp,
                                content: turn["content"].as_str().unwrap().into(),
                                attachments: vec![],
                                project_attachments: vec![],
                            },
                            reliquary_owner_session::model::Branch {
                                canonical: branch_id == "main",
                                id: branch_id,
                                conversation_id: conversation.into(),
                                leaf_node_id: node.into(),
                            },
                        )
                    })
                    .unwrap()
                    .unwrap();
            } else {
                assert_eq!(turn["speaker"], "user");
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
                instance
                    .finalize_conversation_submission(
                        queue,
                        &id,
                        revision,
                        FinalizeCapture {
                            branch_id,
                            parent_node_id,
                            committed_timestamp_ns: timestamp,
                            verified_principal_id: instance.principal().to_owned(),
                        },
                    )
                    .unwrap();
            }
            timestamp += 1;
            for cut in manifest["logical_history_cuts"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|cut| cut["after_turn"].as_str() == Some(node))
            {
                historical.push(capture_cut(instance, owner, cut["id"].as_str().unwrap()));
            }
        }
    }
    assert_eq!(lantern_queue.conversation_revision("conv-l01"), 12);
    assert_eq!(cascade_queue.conversation_revision("conv-c04"), 6);
    assert_eq!(
        lantern_instance
            .with_writable_reliquary(lantern, |rel| rel.accepted_turn_sources().len())
            .unwrap(),
        32
    );
    assert_eq!(
        cascade_instance
            .with_writable_reliquary(cascade, |rel| rel.accepted_turn_sources().len())
            .unwrap(),
        16
    );
    ManifestReplay {
        _lantern_queue: lantern_queue,
        _cascade_queue: cascade_queue,
        historical,
    }
}

fn capture_cut(
    instance: &Instance,
    owner: OwnerId,
    name: &str,
) -> (
    String,
    OwnerId,
    reliquary_owner_session::RelHistoricalCut,
    reliquary_owner_session::HistoricalOwnerProof,
) {
    instance
        .with_writable_reliquary(owner, |rel| {
            let cut = rel.current_historical_cut();
            let proof = rel.historical_proof(owner.as_bytes(), &cut).unwrap();
            (name.to_owned(), owner, cut, proof)
        })
        .unwrap()
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
