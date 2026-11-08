use super::*;
use crate::local_host::{LocalHost, RelAccess, now_ns};
use reliquary_memory::{CreateMemoryRequest, InspectionQuery};

const PAGE: &[(&str, bool)] = &[("--cursor", false), ("--limit", false)];

pub(crate) fn memory(args: &[String]) -> Result<Value, ProductError> {
    let command = args.first().map(String::as_str).unwrap_or_default();
    match command {
        "history" => commands::memory_history(args, false),
        "retire" => semantic_inspection::retire(args),
        "list" => {
            validate_options(args, 2, PAGE)?;
            semantic_inspection::rel_page(&args[1], args, InspectionQuery::Memories, "memory_list")
        }
        "get" | "provenance" => {
            validate_options(args, 3, &[("--revision", false)])?;
            let id = memory_id(&args[2])?;
            let revision = number_option(args, "--revision")?;
            let host = LocalHost::open(&args[1], None, RelAccess::View)?;
            let management = host.instance.workspace_management(host.rel)?;
            if command == "get" && revision.is_none() {
                let page =
                    management.inspect(&InspectionQuery::Memory { id, revision }, None, 1)?;
                let item = page.items.first();
                let memory = match item {
                    Some(reliquary_memory::InspectionItem::Memory(memory)) => json!(memory),
                    _ => Value::Null,
                };
                Ok(json!({"operation":"memory_get","memory":memory,"page":page}))
            } else if command == "get" {
                semantic_inspection::inspect(
                    &management,
                    &InspectionQuery::Memory { id, revision },
                    None,
                    1,
                    "memory_get",
                )
            } else {
                semantic_inspection::inspect(
                    &management,
                    &InspectionQuery::Provenance { id, revision },
                    None,
                    1,
                    "memory_provenance",
                )
            }
        }
        "add" => {
            validate_options(
                args,
                2,
                &[
                    ("--title", false),
                    ("--content", false),
                    ("--category", false),
                    ("--type", false),
                    ("--request-id", false),
                    ("--created-at-ns", false),
                ],
            )?;
            let title = required_option(args, "--title")?;
            let content = required_option(args, "--content")?;
            let category = required_option(args, "--category")?;
            let memory_type = required_option(args, "--type")?;
            let request_id = option(args, "--request-id");
            let timestamp = number_option(args, "--created-at-ns")?;
            if request_id.is_some() && timestamp.is_none() {
                return Err(invalid("--request-id requires a stable --created-at-ns"));
            }
            let host = LocalHost::open(&args[1], None, RelAccess::Manage)?;
            let management = host.instance.workspace_management(host.rel)?;
            let timestamp = timestamp.map_or_else(now_ns, Ok)?;
            let item = if let Some(id) = request_id {
                management.create_memory_with_request(CreateMemoryRequest {
                    request_id: id.into(),
                    title: title.into(),
                    content: content.into(),
                    category: category.into(),
                    memory_type: memory_type.into(),
                    created_at_ns: timestamp,
                })?
            } else {
                management.create_memory(
                    title.into(),
                    content.into(),
                    category.into(),
                    memory_type.into(),
                    timestamp,
                )?
            };
            Ok(json!({"operation":"memory_add","memory":item}))
        }
        _ => Err(invalid(
            "memory requires list, get, provenance, history, or add",
        )),
    }
}

pub(crate) fn archive(args: &[String]) -> Result<Value, ProductError> {
    match args.first().map(String::as_str) {
        Some("branches") => semantic_inspection::branches(args),
        Some("conversations") => {
            validate_options(args, 2, PAGE)?;
            semantic_inspection::rel_page(
                &args[1],
                args,
                InspectionQuery::Conversations,
                "archive_conversations",
            )
        }
        Some("show") => {
            validate_options(args, 4, PAGE)?;
            semantic_inspection::rel_page(
                &args[1],
                args,
                InspectionQuery::Transcript {
                    conversation: args[2].clone(),
                    leaf: args[3].clone(),
                },
                "archive_show",
            )
        }
        Some("turn") => {
            validate_options(args, 4, &[])?;
            semantic_inspection::rel_page(
                &args[1],
                args,
                InspectionQuery::SourceTurn {
                    conversation: args[2].clone(),
                    node: args[3].clone(),
                },
                "archive_turn",
            )
        }
        _ => Err(invalid(
            "archive requires conversations, branches, show, or turn",
        )),
    }
}

pub(crate) fn memory_id(value: &str) -> Result<MemoryId, ProductError> {
    parse_hex(value, "Memory ID").map(MemoryId)
}
