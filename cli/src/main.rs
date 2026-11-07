use reliquary_memory::{
    CompatibilityProfileId, EntityId, MemoryHistoryLookup, MemoryHistoryStatus, MemoryId,
    OwnerScope, Rel, RelationshipId, RewritePolicy, VerifiedCopyPlan, execute_profile_repair,
    execute_rewrite, plan_profile_repair, prune_archive_copy, prune_entity_descriptions_copy,
    prune_memory_copy, prune_relationship_descriptions_copy, reclaim_storage_copy, reconcile_rel,
};
use serde_json::{Value, json};
use std::{env, fs, path::Path, process::ExitCode};

const HELP: &str = r#"reliquary 0.1.0
Supported Reliquary 2.1 operator CLI

USAGE:
    reliquary rel <COMMAND> [OPTIONS]

REL COMMANDS:
    create <PATH> [--scope <unspecified|organization|project|connection>]
    info <PATH>
    verify <PATH>
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

All successful command results are JSON. Copy operations never replace an
existing destination. There is deliberately no cva alias or phy CLI command.
"#;

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args == ["--help"] || args == ["-h"] {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    match run(args) {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string(&value).expect("JSON value serializes")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", serde_json::to_string(&json!({"error":{"kind":format!("{:?}", error.kind()).to_lowercase(),"message":error.to_string()}})).unwrap());
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<Value, reliquary_memory::ProductError> {
    if args == ["--version"] || args == ["-V"] {
        return Ok(json!({"name":"reliquary","version":env!("CARGO_PKG_VERSION")}));
    }
    if args.first().map(String::as_str) != Some("rel") {
        return Err(reliquary_memory::ProductError::new(
            reliquary_memory::ProductErrorKind::Unsupported,
            "only the supported rel command family is available",
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

#[path = "commands.rs"]
mod commands;
use commands::{create, history, info, prune, reclaim, reconcile, repair, rewrite};
#[path = "parse.rs"]
mod parse;
use parse::*;
