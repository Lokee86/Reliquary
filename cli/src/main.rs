use reliquary_memory::{
    CompatibilityProfileId, EntityId, MemoryHistoryLookup, MemoryHistoryStatus, MemoryId,
    OwnerScope, Phylactery, ProductError, ProductErrorKind, Rel, RelationshipId, RewritePolicy,
    VerifiedCopyPlan, execute_profile_repair, execute_rewrite, plan_profile_repair,
    prune_archive_copy, prune_entity_descriptions_copy, prune_memory_copy,
    prune_relationship_descriptions_copy, reclaim_storage_copy, reconcile_rel,
};
use serde_json::{Value, json};
use std::{env, fs, path::Path, process::ExitCode};

const HELP: &str = r#"reliquary 0.1.0
Supported Reliquary 2.1 operator CLI

USAGE:
    reliquary rel <COMMAND> [OPTIONS]
    reliquary phy <COMMAND> [OPTIONS]
    reliquary config check <FILE> [--online [--route <ROUTE>]] [--master-key <FILE>]

REL COMMANDS:
    create <PATH> [--scope <unspecified|organization|project|connection>]
    info <PATH>
    verify <PATH>
    import <PATH> --source <FILE> --format <chatgpt|normalized> [--start <TIME> --end <TIME>]
    process <PATH> --config <FILE> --until <idle|ready> [--import <ID>] [--phy <PHY>]
    work <status|resume|stop> <PATH> [--import <ID>]
    vectors <status|build> <PATH> ...
    <insomnia|dream|entity-resolution|freshness> <status|run> <PATH> ...
    search <PATH> --query <TEXT> --mode <lexical|hybrid|semantic> [--config <FILE>]
    query <PATH> --query <TEXT> --config <FILE>
    interact <PATH> [--config <FILE>] [--commands <JSONL>]
    memory list <PATH> [--cursor <JSON>] [--limit <N>]
    memory <get|provenance|history> <PATH> <MEMORY_ID>
    memory add <PATH> --title <TEXT> --content <TEXT> --category <CATEGORY> --type <TYPE>
    archive conversations <PATH> [--cursor <JSON>] [--limit <N>]
    archive branches <PATH> <CONVERSATION_ID> [--limit <N>] [--cursor <JSON>]
    archive show <PATH> <CONVERSATION_ID> <LEAF_NODE_ID> [--cursor <JSON>] [--limit <N>]
    archive turn <PATH> <CONVERSATION_ID> <NODE_ID>
    graph <neighbors|edge> <PATH> ...
    <entity|relationship|community> <list|get> <PATH> ...
    conversation <feed|status|finalize|cancel|checkpoint|stream|episode> <PATH> ...
    context <inspect|plan|checkpoint> <PATH> <CONVERSATION_ID> <LEAF_NODE_ID> ...
    source status <PATH> --import <ID>
    history memory <PATH> <MEMORY_ID> [--cut <VERSION>]
    history archive <PATH> [--offset <N>] [--limit <N>]
    history proof <PATH>
    rewrite <SOURCE> <DESTINATION> --policy <FILE> [--dry-run]
    prune memory <SOURCE> <DESTINATION> <MEMORY_ID>...
    prune archive <SOURCE> <DESTINATION> <CONVERSATION_ID>...
    prune entity <SOURCE> <DESTINATION> <ENTITY_ID>...
    prune relationship <SOURCE> <DESTINATION> <RELATIONSHIP_ID>...
    repair <SOURCE> <DESTINATION> --keep-profile <PROFILE_ID>...
    reclaim <SOURCE> <DESTINATION>
    reconcile <LEFT> <RIGHT> <DESTINATION>

PHY COMMANDS:
    create <PATH>
    info <PATH>
    verify <PATH>
    memory history <PATH> <MEMORY_ID> [--cut <VERSION>]

All successful command results are JSON. Copy operations never replace an
existing destination. There is deliberately no cva alias.
"#;

fn main() -> ExitCode {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args == ["--help"] || args == ["-h"] {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    let json_flags = args.iter().filter(|arg| *arg == "--json").count();
    if json_flags > 1 {
        eprintln!(
            "{}",
            json!({"error":error_json(&invalid("duplicate CLI option: --json"))})
        );
        return ExitCode::FAILURE;
    }
    args.retain(|arg| arg != "--json");
    if args.starts_with(&["rel".to_owned(), "interact".to_owned()]) {
        return match retrieval_command::interact(&args[2..]) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(error) => {
                eprintln!("{}", json!({"error":error_json(&error)}));
                ExitCode::FAILURE
            }
        };
    }
    match run(args) {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string(&value).expect("JSON value serializes")
            );
            if value["ok"] == false {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("{}", json!({"error":error_json(&error)}));
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<Value, reliquary_memory::ProductError> {
    if args == ["--version"] || args == ["-V"] {
        return Ok(json!({"name":"reliquary","version":env!("CARGO_PKG_VERSION")}));
    }
    let family = args.first().map(String::as_str);
    if family == Some("config") {
        return operational::config(&args[1..]);
    }
    if family == Some("phy") {
        return run_phy(&args[1..]);
    }
    if family != Some("rel") {
        return Err(reliquary_memory::ProductError::new(
            reliquary_memory::ProductErrorKind::Unsupported,
            "unsupported command family",
        ));
    }
    let command = args
        .get(1)
        .map(String::as_str)
        .ok_or_else(|| invalid("missing REL command"))?;
    match command {
        "create" => create(&args[2..]),
        "info" => info(&args[2..], false),
        "verify" => info(&args[2..], true),
        "history" => history(&args[2..]),
        "import" => import_command::run(&args[2..]),
        "process" => operational::process(&args[2..]),
        "work" => operational::work(&args[2..]),
        "vectors" | "insomnia" | "dream" | "entity-resolution" | "freshness" => {
            operational::stage(command, &args[2..])
        }
        "search" => retrieval_command::search(&args[2..]),
        "query" => retrieval_command::query(&args[2..]),
        "memory" => inspection::memory(&args[2..]),
        "archive" => inspection::archive(&args[2..]),
        "graph" => semantic_inspection::graph(&args[2..]),
        "entity" | "relationship" | "community" => {
            semantic_inspection::semantic(command, &args[2..])
        }
        "conversation" => conversation::run(&args[2..]),
        "context" => context_source::context(&args[2..]),
        "source" => context_source::source(&args[2..]),
        "rewrite" => rewrite(&args[2..]),
        "prune" => prune(&args[2..]),
        "repair" => repair(&args[2..]),
        "reclaim" => reclaim(&args[2..]),
        "reconcile" => reconcile(&args[2..]),
        _ => Err(reliquary_memory::ProductError::new(
            reliquary_memory::ProductErrorKind::Unsupported,
            format!("unsupported REL command: {command}"),
        )),
    }
}

fn run_phy(args: &[String]) -> Result<Value, reliquary_memory::ProductError> {
    let command = args
        .first()
        .map(String::as_str)
        .ok_or_else(|| invalid("missing PHY command"))?;
    match command {
        "create" | "info" | "verify" => {
            validate_options(args, 2, &[])?;
            let path = &args[1];
            let info = match command {
                "create" => Phylactery::create(path)?.info()?,
                "info" => Phylactery::open(path)?.info()?,
                _ => Phylactery::verify(path)?,
            };
            Ok(json!({"operation":command,"verified":command=="verify","owner":info}))
        }
        "memory" if args.get(1).is_some_and(|value| value == "history") => {
            commands::memory_history(&args[1..], true)
        }
        "memory" => semantic_inspection::phy_memory(&args[1..]),
        "history" => commands::memory_history(&args[1..], true),
        _ => Err(reliquary_memory::ProductError::new(
            reliquary_memory::ProductErrorKind::Unsupported,
            format!("unsupported PHY command: {command}"),
        )),
    }
}

fn error_json(error: &ProductError) -> Value {
    json!({"kind":error.kind(),"message":error.message()})
}

#[path = "commands.rs"]
mod commands;
mod context_source;
mod conversation;
mod import_command;
mod inspection;
mod interaction_input;
mod interruption;
mod local_host;
mod operational;
mod retrieval_command;
mod semantic_inspection;
use commands::{create, history, info, prune, reclaim, reconcile, repair, rewrite};
#[path = "parse.rs"]
mod parse;
use parse::*;
