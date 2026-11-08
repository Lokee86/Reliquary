use super::*;
use crate::local_host::{LocalHost, RelAccess};
use reliquary_memory::{NativeImportFormat, NativeImportPlan, NativeImportWindow};

pub(crate) fn run(args: &[String]) -> Result<Value, ProductError> {
    validate_options(
        args,
        1,
        &[
            ("--source", false),
            ("--format", false),
            ("--start", false),
            ("--end", false),
            ("--phy", false),
            ("--config", false),
            ("--process", false),
            ("--dry-run", false),
        ],
    )?;
    let format = match required_option(args, "--format")? {
        "chatgpt" => NativeImportFormat::ChatGpt,
        "normalized" => NativeImportFormat::Normalized,
        _ => return Err(invalid("--format must be chatgpt or normalized")),
    };
    let window = match (option(args, "--start"), option(args, "--end")) {
        (Some(start), Some(end)) => Some(
            NativeImportWindow::from_rfc3339(start, end)
                .map_err(|error| invalid(error.to_string()))?,
        ),
        (None, None) => None,
        _ => return Err(invalid("--start and --end must be used together")),
    };
    let process = args.iter().any(|arg| arg == "--process");
    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    if process && dry_run {
        return Err(invalid("--dry-run and --process cannot be combined"));
    }
    let configured = if process {
        Some(operational::runtime(operational::config_path(args))?)
    } else {
        if option(args, "--config").is_some() {
            return Err(invalid("import configuration options require --process"));
        }
        None
    };
    let plan = NativeImportPlan::from_path(required_option(args, "--source")?, format, window)?;
    let coverage = plan.coverage().clone();
    let source_sha256 = hex(&plan.source_sha256());
    if dry_run {
        let rel = Rel::open(&args[0])?;
        if let Some(path) = option(args, "--phy") {
            Phylactery::open(path)?;
        }
        return Ok(
            json!({"operation":"import_plan","import_id":plan.import_id(rel.owner_id()),
            "source_sha256":source_sha256,"coverage":coverage}),
        );
    }
    let host = LocalHost::open(&args[0], option(args, "--phy"), RelAccess::Operate)?;
    eprintln!("{}", json!({"operation":"import_plan","coverage":coverage}));
    let staged = host.instance.stage_native_import(host.rel, plan)?;
    let processing = configured.as_ref().map(|runtime| {
        match operational::run_processing(&host, runtime, Some(&staged.import_id), 128, "ready") {
            Ok(result) => result,
            Err(error) => json!({"ok":false,"error":error_json(&error)}),
        }
    });
    let ok = processing
        .as_ref()
        .is_none_or(|result| result["ok"] == true);
    Ok(json!({
        "operation":"import",
        "ok":ok,
        "import_id":staged.import_id,
        "source_sha256":source_sha256,
        "coverage":coverage,
        "staged":{
            "receipt_digest":hex(&staged.receipt_digest),
            "already_staged":staged.already_staged,
            "already_completed":staged.already_completed,
        },
        "processing":processing
    }))
}
