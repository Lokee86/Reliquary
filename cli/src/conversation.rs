use super::*;
use crate::local_host::{LocalHost, RelAccess, now_ns};
use reliquary_memory::{
    ConversationSubmissionId, EnqueueOutcome, EpisodeBoundary, FinalizeCapture, GenerationReplay,
    SubmissionId,
};

pub(crate) fn run(args: &[String]) -> Result<Value, ProductError> {
    let operation = args.first().map(String::as_str).unwrap_or_default();
    if operation == "episode" {
        validate_options(args, 5, &[])?;
        let host = LocalHost::open(&args[1], None, RelAccess::Operate)?;
        let id = host.instance.finalize_conversation_episode(
            host.rel,
            &args[2],
            &args[3],
            &args[4],
            EpisodeBoundary::Explicit,
            now_ns()?,
        )?;
        return Ok(json!({"operation":"conversation_episode","episode_id":hex(&id.0)}));
    }
    if matches!(operation, "dispatch" | "reconcile" | "cancel-host") {
        return Err(ProductError::new(
            ProductErrorKind::Unsupported,
            "this operation requires a registered trusted conversation host",
        ));
    }
    let specific: &[(&str, bool)] = match operation {
        "feed" => &[("--content", false), ("--revision", false)],
        "cancel" => &[("--revision", false)],
        "finalize" => &[
            ("--branch", false),
            ("--parent", false),
            ("--revision", false),
        ],
        "stream" => &[("--after", false), ("--limit", false)],
        "status" | "checkpoint" => &[],
        _ => return Err(invalid("unsupported conversation operation")),
    };
    let flags = [[("--state-dir", false)].as_slice(), specific].concat();
    validate_options(args, 4, &flags)?;
    let state_dir = required_option(args, "--state-dir")?;
    let submission = SubmissionId::new(&args[3]).map_err(|error| invalid(error.to_string()))?;
    let id = ConversationSubmissionId::new(&args[2], submission)
        .map_err(|error| invalid(error.to_string()))?;
    let revision = if matches!(operation, "feed" | "cancel" | "finalize") {
        Some(
            required_option(args, "--revision")?
                .parse()
                .map_err(|_| invalid("invalid conversation revision"))?,
        )
    } else {
        None
    };
    let content = if operation == "feed" {
        Some(required_option(args, "--content")?)
    } else {
        None
    };
    let branch = if operation == "finalize" {
        Some(required_option(args, "--branch")?)
    } else {
        None
    };
    let after = number_option(args, "--after")?.unwrap_or(0);
    let limit = number_option(args, "--limit")?.unwrap_or(16);
    if !(1..=64).contains(&limit) {
        return Err(invalid("stream limit must be between 1 and 64"));
    }
    let read_only = matches!(operation, "status" | "checkpoint" | "stream");
    let access = if read_only {
        RelAccess::View
    } else {
        RelAccess::Operate
    };
    let host = LocalHost::open(&args[1], None, access)?;
    let mut queue = if read_only {
        host.instance
            .open_conversation_queue_read(state_dir, host.rel)?
    } else {
        host.instance.open_conversation_queue(state_dir, host.rel)?
    };
    let instance = &host.instance;
    let result = match operation {
        "feed" => {
            let outcome = instance.submit_conversation(
                &mut queue,
                &id.conversation_id,
                id.submission_id.clone(),
                content.expect("feed content validated"),
                revision.expect("feed revision validated"),
            )?;
            match outcome {
                EnqueueOutcome::Durable(status) => json!({"state":"durable","status":status}),
                EnqueueOutcome::Idempotent(status) => json!({"state":"idempotent","status":status}),
            }
        }
        "finalize" => {
            let branch = branch.expect("branch validated");
            let parent = option(args, "--parent");
            let capture = match instance
                .conversation_submission_status(&queue, &id)?
                .and_then(|status| status.finalize_capture)
            {
                Some(capture)
                    if capture.branch_id == branch
                        && capture.parent_node_id.as_deref() == parent =>
                {
                    capture
                }
                Some(_) => {
                    return Err(ProductError::new(
                        ProductErrorKind::Conflict,
                        "finalization retry differs from the durable branch or parent",
                    ));
                }
                None => FinalizeCapture {
                    branch_id: branch.to_owned(),
                    parent_node_id: parent.map(str::to_owned),
                    committed_timestamp_ns: now_ns()?,
                    verified_principal_id: instance.principal().to_owned(),
                },
            };
            json!(instance.finalize_conversation_submission(
                &mut queue,
                &id,
                revision.expect("finalize revision validated"),
                capture,
            )?)
        }
        "cancel" => json!(instance.cancel_own_pending_submission(
            &mut queue,
            &id,
            revision.expect("cancel revision validated"),
        )?),
        "status" => json!(instance.conversation_submission_status(&queue, &id)?),
        "checkpoint" => json!(instance.generation_checkpoint_snapshot(&queue, &id)?),
        "stream" => match instance.replay_generation_checkpoints(&queue, &id, after, limit)? {
            GenerationReplay::Checkpoints {
                latest_cursor,
                items,
            } => json!({"state":"checkpoints","latest_cursor":latest_cursor,"items":items}),
            GenerationReplay::ResyncRequired { latest_cursor } => {
                json!({"state":"resync_required","latest_cursor":latest_cursor})
            }
        },
        _ => unreachable!(),
    };
    let revision = instance.conversation_revision(&queue, &id.conversation_id)?;
    Ok(json!({"operation":operation,"submission":id,"revision":revision,"result":result}))
}
