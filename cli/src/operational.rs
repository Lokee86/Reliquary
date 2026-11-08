use super::*;
use crate::local_host::{LocalHost, RelAccess};
use reliquary_memory::{
    ConfiguredRuntime, ConfiguredRuntimeError, ImportStage, LiveProcessingSelection,
    ProcessOptions, ProcessingState,
};

const PROCESS_OPTIONS: &[(&str, bool)] = &[
    ("--import", false),
    ("--until", false),
    ("--max-episodes", false),
    ("--phy", false),
    ("--config", false),
];

pub(crate) const DEFAULT_CONFIG: &str = "reliquary.cfg";

pub(crate) fn config_path(args: &[String]) -> &str {
    option(args, "--config").unwrap_or(DEFAULT_CONFIG)
}

pub(crate) fn config_error(error: ConfiguredRuntimeError) -> ProductError {
    let kind = match error {
        ConfiguredRuntimeError::Io(_) | ConfiguredRuntimeError::Write(_) => ProductErrorKind::Io,
        ConfiguredRuntimeError::Invalid { .. } => ProductErrorKind::InvalidInput,
        ConfiguredRuntimeError::Credential { .. } => ProductErrorKind::Unauthorized,
        _ => ProductErrorKind::Unavailable,
    };
    ProductError::new(kind, error.to_string())
}

pub(crate) fn runtime(path: &str) -> Result<ConfiguredRuntime, ProductError> {
    ConfiguredRuntime::from_path(path).map_err(config_error)
}

pub(crate) fn optional_runtime(args: &[String]) -> Result<Option<ConfiguredRuntime>, ProductError> {
    match option(args, "--config") {
        Some(path) => runtime(path).map(Some),
        None => match ConfiguredRuntime::from_path(DEFAULT_CONFIG) {
            Ok(runtime) => Ok(Some(runtime)),
            Err(ConfiguredRuntimeError::Io(std::io::ErrorKind::NotFound)) => Ok(None),
            Err(error) => Err(config_error(error)),
        },
    }
}

pub(crate) fn process(args: &[String]) -> Result<Value, ProductError> {
    validate_options(args, 1, PROCESS_OPTIONS)?;
    process_from_args(&args[0], args, None, None)
}

pub(crate) fn work(args: &[String]) -> Result<Value, ProductError> {
    match args.first().map(String::as_str) {
        Some("status") => {
            validate_options(args, 2, &[("--import", false)])?;
            let host = LocalHost::open(&args[1], None, RelAccess::View)?;
            let status = host
                .instance
                .processing_status(host.rel, option(args, "--import"))?;
            Ok(
                json!({"operation":"work_status","import_id":option(args,"--import"),"insomnia":status}),
            )
        }
        Some("resume") => {
            let options = [PROCESS_OPTIONS, &[("--episode", false)]].concat();
            validate_options(args, 2, &options)?;
            let target = if option(args, "--import").is_some() {
                "ready"
            } else {
                "idle"
            };
            process_from_args(&args[1], args, Some(target), None)
        }
        Some("stop") => {
            validate_options(args, 2, &[("--import", false)])?;
            Err(ProductError::new(
                ProductErrorKind::Unsupported,
                "this local invocation has no control channel for another process; stop the owning process",
            ))
        }
        _ => Err(invalid("work requires status, resume, or stop")),
    }
}

pub(crate) fn stage(family: &str, args: &[String]) -> Result<Value, ProductError> {
    let operation = args.first().map(String::as_str).unwrap_or_default();
    match (family, operation) {
        ("vectors", "build") if option(args, "--import").is_none() => {
            validate_options(args, 2, &[("--config", false), ("--limit", false)])?;
            let runtime = runtime(config_path(args))?;
            let limit = number_option(args, "--limit")?.unwrap_or(128);
            let host = LocalHost::open(&args[1], None, RelAccess::Operate)?;
            let coverage = host.instance.build_vectors(host.rel, &runtime, limit)?;
            Ok(json!({"operation":"vectors_build","coverage":coverage}))
        }
        ("vectors", "status") if option(args, "--import").is_none() => {
            validate_options(args, 2, &[("--profile", false)])?;
            let profile = CompatibilityProfileId(parse_hex(
                required_option(args, "--profile")?,
                "profile ID",
            )?);
            let host = LocalHost::open(&args[1], None, RelAccess::View)?;
            let coverage = host.instance.vector_status(host.rel, profile)?;
            Ok(json!({"operation":"vectors_status","coverage":coverage}))
        }
        ("freshness", "status") if option(args, "--import").is_none() => {
            validate_options(args, 2, &[("--limit", false)])?;
            let limit = number_option(args, "--limit")?.unwrap_or(32);
            let host = LocalHost::open(&args[1], None, RelAccess::View)?;
            let due = host.instance.freshness_due(host.rel, limit)?;
            let due = due
                .into_iter()
                .map(|row| {
                    json!({
                        "boundary_turn":row.boundary_turn, "memory_id":hex(&row.memory_id.0),
                        "from":format!("{:?}",row.from_state).to_ascii_lowercase(),
                        "to":format!("{:?}",row.to_state).to_ascii_lowercase(),
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({"operation":"freshness_status","due":due}))
        }
        ("dream" | "entity-resolution", "status") if option(args, "--import").is_none() => {
            validate_options(args, 2, &[("--phy", false)])?;
            let host = LocalHost::open(&args[1], option(args, "--phy"), RelAccess::View)?;
            let stage = if family == "dream" {
                ImportStage::Dream
            } else {
                ImportStage::Entity
            };
            let progress = host.instance.live_stage_status(host.rel, host.phy, stage)?;
            Ok(json!({"operation":"stage_status","stage":family,"progress":progress}))
        }
        (_, "status") => {
            validate_options(args, 2, &[("--import", false)])?;
            let id = option(args, "--import");
            let host = LocalHost::open(&args[1], None, RelAccess::View)?;
            if family != "insomnia" && id.is_none() {
                return Err(invalid("stage status requires --import"));
            }
            let progress = host.instance.processing_status(host.rel, id)?;
            let import = id
                .map(|id| host.instance.native_import_status(host.rel, id))
                .transpose()?
                .flatten();
            Ok(
                json!({"operation":"stage_status","stage":family,"insomnia":progress,"import":import}),
            )
        }
        (_, "run" | "build") => {
            validate_options(args, 2, PROCESS_OPTIONS)?;
            let selection = if option(args, "--import").is_some() {
                None
            } else {
                Some(match family {
                    "insomnia" => LiveProcessingSelection::Insomnia,
                    "entity-resolution" => LiveProcessingSelection::Entity,
                    "dream" => LiveProcessingSelection::Dream,
                    _ => return Err(invalid("stage run requires --import")),
                })
            };
            let target = if selection == Some(LiveProcessingSelection::Insomnia) {
                "idle"
            } else {
                "ready"
            };
            process_from_args(&args[1], args, Some(target), selection)
        }
        _ => Err(invalid("unsupported owner processing operation")),
    }
}

fn process_from_args(
    path: &str,
    args: &[String],
    default_target: Option<&str>,
    selection: Option<LiveProcessingSelection>,
) -> Result<Value, ProductError> {
    let target = option(args, "--until").or(default_target).unwrap_or("");
    if !matches!(target, "idle" | "ready") {
        return Err(invalid("--until must be idle or ready"));
    }
    let import_id = option(args, "--import");
    let episode = option(args, "--episode")
        .map(|value| parse_hex(value, "Episode ID").map(reliquary_memory::EpisodeId))
        .transpose()?;
    let max_episodes = number_option(args, "--max-episodes")?.unwrap_or(128);
    if !(1..=reliquary_memory::MAX_PROCESS_EPISODES).contains(&max_episodes) {
        return Err(invalid("--max-episodes is outside the supported bounds"));
    }
    let configured = runtime(config_path(args))?;
    let host = LocalHost::open(path, option(args, "--phy"), RelAccess::Operate)?;
    run_selected_processing(
        &host,
        &configured,
        import_id,
        max_episodes,
        target,
        selection,
        episode,
    )
}

pub(crate) fn run_processing(
    host: &LocalHost,
    configured: &ConfiguredRuntime,
    import_id: Option<&str>,
    max_episodes: usize,
    target: &str,
) -> Result<Value, ProductError> {
    run_selected_processing(
        host,
        configured,
        import_id,
        max_episodes,
        target,
        None,
        None,
    )
}

fn run_selected_processing(
    host: &LocalHost,
    configured: &ConfiguredRuntime,
    import_id: Option<&str>,
    max_episodes: usize,
    target: &str,
    selection: Option<LiveProcessingSelection>,
    retry_episode: Option<reliquary_memory::EpisodeId>,
) -> Result<Value, ProductError> {
    use std::time::{Duration, Instant};
    let interrupt = crate::interruption::InterruptGuard::install()?;
    let options = ProcessOptions {
        import_id: import_id.map(str::to_owned),
        destination: host.phy,
        max_episodes,
    };
    let job = match (selection, retry_episode) {
        (Some(selection), None) => host.instance.start_live_processing_stage(
            host.rel,
            configured,
            host.phy,
            max_episodes,
            selection,
        )?,
        (None, Some(episode)) => host
            .instance
            .resume_held_processing(host.rel, configured, options, episode)?,
        (None, None) => host
            .instance
            .start_processing(host.rel, configured, options)?,
        (Some(_), Some(_)) => {
            return Err(invalid(
                "held Episode retry requires the work resume command",
            ));
        }
    };
    let mut last_report = None;
    let mut next_report = Instant::now();
    while !job.is_finished() {
        if interrupt.interrupted() {
            job.request_stop();
        }
        if Instant::now() >= next_report {
            let progress = json!({"operation":"processing_progress","report":job.poll()?});
            if last_report.as_ref() != Some(&progress) {
                eprintln!("{progress}");
                last_report = Some(progress);
            }
            next_report = Instant::now() + Duration::from_secs(1);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let report = job.join()?;
    let ok = report.state == ProcessingState::Ready
        || target == "idle" && report.state == ProcessingState::Idle;
    Ok(json!({"operation":"process","until":target,"ok":ok,"report":report}))
}
