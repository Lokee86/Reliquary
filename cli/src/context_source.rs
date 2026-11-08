use super::*;
use crate::local_host::{LocalHost, RelAccess};
use reliquary_memory::{
    ContextMessage, ContextRole, ContextTokenCounter, ContextTurn, ContextView,
    DEFAULT_COMPACTION_TRIGGER_PERCENT, MAX_CONTEXT_SOURCE_BYTES, MAX_CONTEXT_SOURCE_TURNS,
    compaction_budget,
};
use std::io::Read;

struct ByteBudget;
impl ContextTokenCounter for ByteBudget {
    fn count_text(&self, text: &str) -> u64 {
        text.len() as u64
    }

    fn count_message(&self, message: &ContextMessage) -> u64 {
        self.count_text(&message.content)
    }
}

pub(crate) fn context(args: &[String]) -> Result<Value, ProductError> {
    let command = args.first().map(String::as_str).unwrap_or_default();
    if !matches!(command, "inspect" | "plan" | "checkpoint") {
        return Err(invalid("context requires inspect, plan, or checkpoint"));
    }
    let flags = if command == "checkpoint" {
        vec![("--byte-budget", false), ("--summary-file", false)]
    } else {
        vec![("--byte-budget", false)]
    };
    validate_options(args, 4, &flags)?;
    let limit = number_option(args, "--byte-budget")?.unwrap_or(64 * 1024);
    let budget = compaction_budget(limit, DEFAULT_COMPACTION_TRIGGER_PERCENT)
        .map_err(|error| invalid(error.to_string()))?;
    let summary = if command == "checkpoint" {
        let file = fs::File::open(required_option(args, "--summary-file")?)?;
        let mut bytes = Vec::new();
        file.take(budget.summary_tokens.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > budget.summary_tokens {
            return Err(invalid("summary exceeds the inspection budget"));
        }
        Some(String::from_utf8(bytes).map_err(|_| invalid("summary is not UTF-8"))?)
    } else {
        None
    };
    let host = LocalHost::open(&args[1], None, RelAccess::View)?;
    let management = host.instance.workspace_management(host.rel)?;
    management.select_conversation(&args[2], &args[3])?;
    let mut view = ContextView::default();
    let mut offset = 0;
    let mut bytes = 0;
    loop {
        let page = management.transcript(offset, 32)?;
        for turn in page.items {
            let role = match turn.role.as_str() {
                "user" => ContextRole::User,
                "assistant" => ContextRole::Assistant,
                _ => continue,
            };
            bytes += turn.content.len();
            if bytes > MAX_CONTEXT_SOURCE_BYTES || view.turns.len() >= MAX_CONTEXT_SOURCE_TURNS {
                return Err(invalid(
                    "context transcript exceeds the supported inspection bound",
                ));
            }
            view.turns.push(ContextTurn {
                message_id: turn.node_id,
                role,
                content: turn.content,
            });
        }
        let Some(next) = page.next_offset else { break };
        offset = next;
    }
    let prepared = host.instance.prepare_context_workflow(
        host.rel,
        &args[2],
        view,
        &[],
        &[],
        &[],
        None,
        bytes as u64,
        budget,
        &ByteBudget,
    )?;
    let result = match command {
        "inspect" => {
            let request = host.instance.context_request(&prepared)?;
            json!({"instructions":request.instructions,"messages":messages(&request.messages)})
        }
        "plan" => host.instance.context_compaction_plan(&prepared)?.map_or(Value::Null, |plan| {
            json!({"through_message_id":plan.through_message_id,"instructions":plan.instructions,
                "messages":messages(&plan.messages),"summary_byte_limit":plan.summary_token_limit})
        }),
        _ => {
            let checkpoint = host.instance.finalize_context_workflow(
                &prepared, summary.expect("checkpoint summary validated"), &ByteBudget,
            )?;
            match checkpoint {
                None => Value::Null,
                Some(checkpoint) => {
                    let checkpoint = host.instance.context_checkpoint(&checkpoint)?;
                    json!({"through_message_id":checkpoint.through_message_id,"summary":checkpoint.summary})
                }
            }
        }
    };
    Ok(json!({"operation":command,"budget_unit":"utf8_bytes","result":result}))
}

fn messages(messages: &[ContextMessage]) -> Vec<Value> {
    messages.iter().map(|message| json!({
        "role":match message.role { ContextRole::User => "user", ContextRole::Assistant => "assistant" },
        "content":message.content
    })).collect()
}

pub(crate) fn source(args: &[String]) -> Result<Value, ProductError> {
    match args.first().map(String::as_str) {
        Some("status") => {
            validate_options(args, 2, &[("--import", false)])?;
            let id = required_option(args, "--import")?;
            let host = LocalHost::open(&args[1], None, RelAccess::View)?;
            Ok(
                json!({"operation":"source_status","status":host.instance.native_import_status(host.rel,id)?}),
            )
        }
        Some("manifest" | "hydrate" | "index" | "publish") => Err(ProductError::new(
            ProductErrorKind::Unsupported,
            "this operation requires a registered trusted external source host",
        )),
        _ => Err(invalid("unsupported source operation")),
    }
}
