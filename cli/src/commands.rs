use super::*;

pub(super) fn create(args: &[String]) -> Result<Value, reliquary_memory::ProductError> {
    validate_options(args, 1, &[("--scope", false)])?;
    let path = one_or_option_path(args)?;
    let scope = option(args, "--scope")
        .map(parse_scope)
        .transpose()?
        .unwrap_or(OwnerScope::Project);
    let rel = Rel::create(path, scope)?;
    Ok(json!({"operation":"create","owner":rel.info()?}))
}
pub(super) fn info(args: &[String], verify: bool) -> Result<Value, reliquary_memory::ProductError> {
    if args.len() != 1 {
        return Err(invalid("info/verify requires exactly one path"));
    }
    let info = if verify {
        Rel::verify(&args[0])?
    } else {
        Rel::open(&args[0])?.info()?
    };
    Ok(json!({"operation":if verify {"verify"} else {"info"},"verified":verify,"owner":info}))
}
pub(super) fn history(args: &[String]) -> Result<Value, reliquary_memory::ProductError> {
    if args.first().map(String::as_str) == Some("archive") {
        validate_options(args, 2, &[("--offset", false), ("--limit", false)])?;
        let offset = option(args, "--offset")
            .unwrap_or("0")
            .parse()
            .map_err(|_| invalid("offset must be an unsigned integer"))?;
        let limit = option(args, "--limit")
            .unwrap_or("128")
            .parse()
            .map_err(|_| invalid("limit must be an unsigned integer"))?;
        return Ok(
            json!({"operation":"history_archive","page":Rel::open(&args[1])?.conversations(offset, limit)?}),
        );
    }
    if args.first().map(String::as_str) == Some("proof") {
        if args.len() != 2 {
            return Err(invalid("history proof requires PATH"));
        }
        return Ok(
            json!({"operation":"history_proof","proof":Rel::open(&args[1])?.current_historical_proof()?}),
        );
    }
    if args.len() < 3 || args[0] != "memory" {
        return Err(invalid("history requires memory, archive, or proof"));
    }
    validate_options(args, 3, &[("--cut", false)])?;
    let rel = Rel::open(&args[1])?;
    let id = MemoryId(parse_hex::<32>(&args[2], "Memory ID")?);
    let lookup = match option(args, "--cut") {
        Some(value) => rel.memory_history_at_cut(
            id,
            value
                .parse()
                .map_err(|_| invalid("history cut must be an unsigned integer"))?,
        )?,
        None => rel.memory_history(id)?,
    };
    let result = match lookup {
        MemoryHistoryLookup::Unavailable { .. } => {
            json!({"status":"unavailable","memory_id":args[2]})
        }
        MemoryHistoryLookup::Found(read) => json!({
            "status":match read.status { MemoryHistoryStatus::Available => "available", MemoryHistoryStatus::Tombstoned(_) => "tombstoned" },
            "memory_id":args[2], "revisions":read.history.revisions.len(),
            "payloads_available":read.history.revisions.iter().filter(|revision| matches!(revision.payload, reliquary_memory::PayloadState::Available(_))).count()
        }),
    };
    Ok(json!({"operation":"history_memory","result":result}))
}
pub(super) fn rewrite(args: &[String]) -> Result<Value, reliquary_memory::ProductError> {
    validate_options(
        args,
        2,
        &[
            ("--policy", false),
            ("--dry-run", false),
            ("--profile", true),
            ("--include-memory", true),
            ("--exclude-memory", true),
        ],
    )?;
    let policy_path =
        option(args, "--policy").ok_or_else(|| invalid("rewrite requires --policy FILE"))?;
    let mut policy: RewritePolicy = serde_json::from_slice(&fs::read(policy_path)?)
        .map_err(|error| invalid(format!("invalid rewrite policy: {error}")))?;
    let profiles = repeated_options(args, "--profile");
    if !profiles.is_empty() {
        policy.profile_ids = profiles
            .iter()
            .map(|id| parse_hex::<32>(id, "profile ID"))
            .collect::<Result<_, _>>()?;
    }
    let included = repeated_options(args, "--include-memory");
    if !included.is_empty() {
        policy.memories.include = included
            .iter()
            .map(|id| parse_hex::<32>(id, "Memory ID"))
            .collect::<Result<_, _>>()?;
    }
    let excluded = repeated_options(args, "--exclude-memory");
    if !excluded.is_empty() {
        policy.memories.exclude = excluded
            .iter()
            .map(|id| parse_hex::<32>(id, "Memory ID"))
            .collect::<Result<_, _>>()?;
    }
    let rel = Rel::open(&args[0])?;
    let plan = rel.plan_rewrite(policy)?;
    if args.iter().any(|arg| arg == "--dry-run") {
        return Ok(json!({"operation":"rewrite_plan","dry_run":true,"plan":plan}));
    }
    let report = execute_rewrite(&plan, &args[0], &args[1])?;
    Ok(
        json!({"operation":"rewrite","source_owner_id":hex(&report.source_owner_id),"destination_owner_id":hex(&report.destination_owner_id),"retained_memories":report.retained_memories,"retained_revisions":report.retained_revisions,"retained_tombstones":report.retained_tombstones,"output_bytes":report.output_bytes}),
    )
}
pub(super) fn prune(args: &[String]) -> Result<Value, reliquary_memory::ProductError> {
    if args.len() < 4 || args[3..].iter().any(|item| item.starts_with('-')) {
        return Err(invalid(
            "prune requires a domain, source, destination, and non-option targets",
        ));
    }
    let plan = VerifiedCopyPlan::prepare(&args[1])?;
    match args[0].as_str() {
        "memory" => {
            let ids = args[3..]
                .iter()
                .map(|id| parse_hex::<32>(id, "Memory ID").map(MemoryId))
                .collect::<Result<Vec<_>, _>>()?;
            let report = prune_memory_copy(&plan, &args[1], &args[2], &ids)?;
            Ok(
                json!({"operation":"prune_memory","owner_id":hex(&report.owner_id),"pruned_memories":report.pruned_memories,"source_bytes":report.source_bytes,"transformed_bytes":report.transformed_bytes}),
            )
        }
        "archive" => {
            let report = prune_archive_copy(&plan, &args[1], &args[2], &args[3..])?;
            Ok(
                json!({"operation":"prune_archive","owner_id":hex(&report.owner_id),"pruned_conversations":report.pruned_conversations,"pruned_turns":report.pruned_turns,"source_bytes":report.source_bytes,"transformed_bytes":report.transformed_bytes}),
            )
        }
        "entity" => {
            let ids = args[3..]
                .iter()
                .map(|id| parse_hex::<32>(id, "Entity ID").map(EntityId))
                .collect::<Result<Vec<_>, _>>()?;
            let report = prune_entity_descriptions_copy(&plan, &args[1], &args[2], &ids)?;
            Ok(
                json!({"operation":"prune_entity","owner_id":hex(&report.owner_id),"pruned_entities":report.pruned_entities,"pruned_descriptions":report.pruned_descriptions,"source_bytes":report.source_bytes,"transformed_bytes":report.transformed_bytes}),
            )
        }
        "relationship" => {
            let ids = args[3..]
                .iter()
                .map(|id| parse_hex::<32>(id, "Relationship ID").map(RelationshipId))
                .collect::<Result<Vec<_>, _>>()?;
            let report = prune_relationship_descriptions_copy(&plan, &args[1], &args[2], &ids)?;
            Ok(
                json!({"operation":"prune_relationship","owner_id":hex(&report.owner_id),"pruned_relationships":report.pruned_relationships,"pruned_descriptions":report.pruned_descriptions,"source_bytes":report.source_bytes,"transformed_bytes":report.transformed_bytes}),
            )
        }
        _ => Err(invalid(
            "supported prune domains are memory, archive, entity and relationship",
        )),
    }
}
pub(super) fn repair(args: &[String]) -> Result<Value, reliquary_memory::ProductError> {
    validate_options(args, 2, &[("--keep-profile", true)])?;
    let profiles = repeated_options(args, "--keep-profile")
        .into_iter()
        .map(|id| parse_hex::<32>(id, "profile ID").map(CompatibilityProfileId))
        .collect::<Result<Vec<_>, _>>()?;
    if profiles.is_empty() {
        return Err(invalid("repair requires at least one --keep-profile ID"));
    }
    let plan = plan_profile_repair(&args[0], &profiles)?;
    let report = execute_profile_repair(&plan, &args[0], &args[1])?;
    Ok(
        json!({"operation":"repair","owner_id":hex(&report.owner_id),"retired_profiles":report.retired.iter().map(|id|hex(&id.0)).collect::<Vec<_>>(),"evidence_digest":hex(&report.evidence_digest)}),
    )
}
pub(super) fn reclaim(args: &[String]) -> Result<Value, reliquary_memory::ProductError> {
    if args.len() != 2 {
        return Err(invalid("reclaim requires SOURCE DESTINATION"));
    }
    let plan = VerifiedCopyPlan::prepare(&args[0])?;
    let report = reclaim_storage_copy(&plan, &args[0], &args[1])?;
    Ok(
        json!({"operation":"reclaim","removed_frames":report.removed_frames,"source_bytes":report.source_bytes,"candidate_bytes":report.candidate_bytes}),
    )
}
pub(super) fn reconcile(args: &[String]) -> Result<Value, reliquary_memory::ProductError> {
    if args.len() != 3 {
        return Err(invalid("reconcile requires LEFT RIGHT DESTINATION"));
    }
    Ok(json!({"operation":"reconcile","report":reconcile_rel(&args[0], &args[1], &args[2])?}))
}
