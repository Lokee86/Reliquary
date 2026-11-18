use super::*;
use crate::local_host::{LocalHost, RelAccess};
use reliquary_memory::MAX_REL_DEPENDENCIES;

pub(super) fn run(args: &[String]) -> Result<Value, ProductError> {
    match args.first().map(String::as_str) {
        Some("list") => {
            validate_options(args, 2, &[])?;
            let host = LocalHost::open(&args[1], None, RelAccess::View)?;
            let declaration = host
                .instance
                .workspace_management(host.rel)?
                .rel_dependencies()?;
            Ok(json!({"operation":"dependency_list","owner_id":host.rel,"declaration":declaration}))
        }
        Some("resolve") => {
            validate_options(args, 2, &[("--mount-rel", true)])?;
            let paths = repeated_options(args, "--mount-rel");
            let (host, _) =
                LocalHost::open_with_rel_mounts(&args[1], None, RelAccess::View, &paths)?;
            let order = host
                .instance
                .workspace_management(host.rel)?
                .resolve_rel_dependencies()?;
            Ok(json!({"operation":"dependency_resolve","owner_id":host.rel,"order":order}))
        }
        Some("replace") => {
            validate_options(
                args,
                2,
                &[
                    ("--expected-generation", false),
                    ("--dependency", true),
                    ("--mount-rel", true),
                ],
            )?;
            let generation = number_option(args, "--expected-generation")?
                .ok_or_else(|| invalid("--expected-generation is required"))?;
            let mut paths = repeated_options(args, "--dependency");
            let direct_count = paths.len();
            if direct_count > MAX_REL_DEPENDENCIES {
                return Err(invalid("REL dependency count exceeds limit"));
            }
            paths.extend(repeated_options(args, "--mount-rel"));
            let (host, mounted) =
                LocalHost::open_with_rel_mounts(&args[1], None, RelAccess::Manage, &paths)?;
            let result = host
                .instance
                .workspace_management(host.rel)?
                .replace_rel_dependencies(generation, &mounted[..direct_count])?;
            Ok(json!({"operation":"dependency_replace","owner_id":host.rel,"result":result}))
        }
        _ => Err(invalid("dependency requires list, resolve, or replace")),
    }
}
