use super::*;
use crate::interaction_input::{self, Command};
use crate::local_host::{LocalHost, RelAccess};
use reliquary_memory::{
    ArchiveSearchOptions, ConfiguredRuntime, GraphNode, InspectionQuery, RetrievalMode, SearchMode,
    SearchOptions,
};
use std::io::{BufRead, BufReader, Write};

pub(crate) fn search(args: &[String]) -> Result<Value, ProductError> {
    validate_options(
        args,
        1,
        &[
            ("--query", false),
            ("--mode", false),
            ("--scope", false),
            ("--limit", false),
            ("--config", false),
            ("--embedding-route", false),
        ],
    )?;
    let query = required_option(args, "--query")?;
    let mode = match required_option(args, "--mode")? {
        "lexical" => SearchMode::Lexical,
        "hybrid" => SearchMode::Hybrid,
        "semantic" => SearchMode::Semantic,
        _ => return Err(invalid("--mode must be lexical, hybrid, or semantic")),
    };
    let archive = match option(args, "--scope") {
        None | Some("memory") => false,
        Some("archive") => true,
        Some(_) => return Err(invalid("--scope must be memory or archive")),
    };
    let limit = number_option(args, "--limit")?;
    if archive {
        if mode != SearchMode::Hybrid {
            return Err(invalid("--scope archive requires --mode hybrid"));
        }
        let runtime = operational::runtime(operational::config_path(args))?;
        let host = LocalHost::open(&args[0], None, RelAccess::View)?;
        let result = host.instance.search_archive(
            host.rel,
            query,
            ArchiveSearchOptions {
                limit,
                embedding_route: option(args, "--embedding-route").map(str::to_owned),
            },
            &runtime,
        )?;
        let hits: Vec<_> = result
            .hits
            .iter()
            .map(|hit| {
                json!({
                    "fragment_id": hex(&hit.fragment_id),
                    "ordinal": hit.ordinal,
                    "conversation_id": hit.conversation_id,
                    "start_node_id": hit.start_node_id,
                    "end_node_id": hit.end_node_id,
                    "lexical_score": hit.lexical_score,
                    "semantic_score": hit.semantic_score,
                    "score": hit.score,
                })
            })
            .collect();
        return Ok(json!({
            "operation": "search",
            "result": {
                "scope": "archive",
                "mode": "hybrid",
                "profile_id": hex(&result.profile_id.0),
                "generation_id": hex(&result.generation_id),
                "source_archive_version": result.source_archive_version,
                "fragments_scored": result.fragments_scored,
                "fragments_with_text": result.fragments_with_text,
                "truncated": result.truncated,
                "hits": hits,
            }
        }));
    }
    let configured = if mode == SearchMode::Lexical {
        option(args, "--config")
            .map(operational::runtime)
            .transpose()?
    } else {
        Some(operational::runtime(operational::config_path(args))?)
    };
    let options = SearchOptions {
        mode,
        limit,
        embedding_route: option(args, "--embedding-route").map(str::to_owned),
    };
    let host = LocalHost::open(&args[0], None, RelAccess::View)?;
    let result = host
        .instance
        .search(host.rel, query, options, configured.as_ref())?;
    Ok(json!({"operation":"search","result":result}))
}

pub(crate) fn query(args: &[String]) -> Result<Value, ProductError> {
    validate_options(
        args,
        1,
        &[
            ("--query", false),
            ("--config", false),
            ("--embedding-route", false),
        ],
    )?;
    let text = required_option(args, "--query")?;
    let configured = operational::runtime(operational::config_path(args))?;
    let host = LocalHost::open(&args[0], None, RelAccess::View)?;
    let session = host.instance.begin_retrieval_with_embedding_route(
        host.rel,
        "cli-query",
        RetrievalMode::Inspect,
        option(args, "--embedding-route"),
    )?;
    let result = host.instance.memory_query(&session, text, &configured)?;
    Ok(json!({"operation":"query","result":result}))
}

pub(crate) fn interact(
    args: &[String],
    input: impl BufRead,
    output: impl Write,
) -> Result<bool, ProductError> {
    validate_options(
        args,
        1,
        &[
            ("--config", false),
            ("--commands", false),
            ("--embedding-route", false),
        ],
    )?;
    let configured = operational::optional_runtime(args)?;
    let route = option(args, "--embedding-route");
    let commands = option(args, "--commands").map(fs::File::open).transpose()?;
    let host = LocalHost::open(&args[0], None, RelAccess::View)?;
    let instance = &host.instance;
    let management = instance.workspace_management(host.rel)?;
    let begin = |turn| {
        instance.begin_retrieval_with_embedding_route(
            host.rel,
            &format!("cli-interact-{turn}"),
            RetrievalMode::Inspect,
            route,
        )
    };
    let mut turn = 1u64;
    let mut session = begin(turn)?;
    let mut active = false;
    let mut execute = |command| {
        let value = match command {
            Command::Query(text) => {
                let outcome =
                    instance.memory_query(&session, &text, configured_runtime(&configured)?)?;
                active = true;
                json!({"operation":"query","outcome":outcome})
            }
            Command::Refine(text) => json!({"operation":"refine","outcome":
                instance.refine_query(&session, &text, configured_runtime(&configured)?)?}),
            Command::Traverse(ids) => {
                let ids = ids
                    .iter()
                    .map(|id| inspection::memory_id(id))
                    .collect::<Result<Vec<_>, _>>()?;
                let outcome =
                    instance.traverse_graph(&session, &ids, configured_runtime(&configured)?)?;
                json!({"operation":"traverse","outcome":outcome})
            }
            Command::Show(id) => semantic_inspection::inspect(
                &management,
                &InspectionQuery::Memory {
                    id: inspection::memory_id(&id)?,
                    revision: None,
                },
                None,
                1,
                "show",
            )?,
            Command::Source(id) => semantic_inspection::inspect(
                &management,
                &InspectionQuery::Provenance {
                    id: inspection::memory_id(&id)?,
                    revision: None,
                },
                None,
                1,
                "source",
            )?,
            Command::Neighbors(id) => semantic_inspection::inspect(
                &management,
                &InspectionQuery::Neighbors {
                    anchor: GraphNode::Memory(inspection::memory_id(&id)?.0),
                },
                None,
                reliquary_memory::MAX_MANAGEMENT_PAGE_ITEMS,
                "neighbors",
            )?,
            Command::Status => {
                json!({"operation":"status","mode":"inspect","turn":turn,"active_query":active})
            }
            Command::NewTurn => {
                let next = turn
                    .checked_add(1)
                    .ok_or_else(|| invalid("interaction turn counter exhausted"))?;
                let next_session = begin(next)?;
                instance.cancel_retrieval(&session)?;
                session = next_session;
                turn = next;
                active = false;
                json!({"operation":"new_turn","turn":turn})
            }
            Command::Quit => unreachable!("input driver handles quit"),
        };
        Ok(value)
    };
    match commands {
        Some(file) => interaction_input::run(BufReader::new(file), output, true, &mut execute),
        None => interaction_input::run(input, output, false, &mut execute),
    }
}

fn configured_runtime(
    configured: &Option<ConfiguredRuntime>,
) -> Result<&ConfiguredRuntime, ProductError> {
    configured.as_ref().ok_or_else(|| {
        ProductError::new(
            ProductErrorKind::Unavailable,
            "query, refine, and traverse require --config",
        )
    })
}
