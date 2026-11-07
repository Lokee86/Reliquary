use serde_json::Value;
use std::collections::{HashMap, HashSet};

const MANIFEST: &str = include_str!("fixtures/g4-native-rel21-v1.json");

#[test]
fn authored_g4_manifest_has_consistent_fixture_schema() {
    let manifest: Value = serde_json::from_str(MANIFEST).expect("manifest is valid JSON");
    validate_manifest(&manifest).unwrap_or_else(|errors| panic!("{errors:#?}"));
}

#[test]
fn schema_rejects_duplicate_scripts_unclear_grants_and_bad_branch_parents() {
    let manifest: Value = serde_json::from_str(MANIFEST).expect("manifest is valid JSON");

    let mut duplicate = manifest.clone();
    duplicate["conversation_scripts"][1]["id"] = duplicate["conversation_scripts"][0]["id"].clone();
    assert!(
        validate_manifest(&duplicate)
            .unwrap_err()
            .iter()
            .any(|error| error.contains("duplicate script ID"))
    );

    let mut unclear_grant = manifest.clone();
    unclear_grant["grants"][0]["actions"] = serde_json::json!(["all"]);
    assert!(
        validate_manifest(&unclear_grant)
            .unwrap_err()
            .iter()
            .any(|error| error.contains("unclear actions"))
    );

    let mut bad_parent = manifest;
    bad_parent["conversation_scripts"][2]["branches"][0]["parent_turn_id"] =
        serde_json::json!("missing-parent");
    assert!(
        validate_manifest(&bad_parent)
            .unwrap_err()
            .iter()
            .any(|error| error.contains("inconsistent parent/child/canonical"))
    );
}

#[test]
fn schema_rejects_wrong_owner_allocation_duplicate_conversations_and_missing_evidence_sections() {
    let manifest: Value = serde_json::from_str(MANIFEST).expect("manifest is valid JSON");

    let mut wrong_allocation = manifest.clone();
    wrong_allocation["conversation_scripts"][7]["owner"] = serde_json::json!("rel-cascade");
    assert!(
        validate_manifest(&wrong_allocation)
            .unwrap_err()
            .iter()
            .any(|error| error.contains("allocation"))
    );

    let mut duplicate_conversation = manifest.clone();
    duplicate_conversation["conversation_scripts"][1]["conversation_id"] =
        duplicate_conversation["conversation_scripts"][0]["conversation_id"].clone();
    assert!(
        validate_manifest(&duplicate_conversation)
            .unwrap_err()
            .iter()
            .any(|error| error.contains("duplicate/missing conversation"))
    );

    let mut missing_evidence = manifest;
    missing_evidence["adversarial_disruptions"] = serde_json::json!([]);
    assert!(
        validate_manifest(&missing_evidence)
            .unwrap_err()
            .iter()
            .any(|error| error.contains("mandatory routing/state/history/retrieval/adversarial"))
    );
}

fn validate_manifest(manifest: &Value) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    if string(manifest, "manifest_name") != Some("g4-native-rel21-corpus-v1")
        || manifest.get("schema_version").and_then(Value::as_u64) != Some(1)
        || string(manifest, "fixture_seed") != Some("g4-rel21-v1")
    {
        errors.push("unexpected manifest identity, schema, or fixture seed".into());
    }
    let cohort = manifest.get("cohort").unwrap_or(&Value::Null);
    if cohort.get("expected_rel_owners").and_then(Value::as_u64) != Some(2)
        || cohort.get("expected_phy_owners").and_then(Value::as_u64) != Some(2)
        || cohort.get("expected_conversations").and_then(Value::as_u64) != Some(12)
        || cohort
            .get("authored_accepted_native_turns")
            .and_then(Value::as_u64)
            != Some(48)
    {
        errors.push("cohort declaration does not match the frozen 2/2/12/48 contract".into());
    }
    let owners = objects(manifest, "owners");
    let owner_kinds: HashMap<_, _> = owners
        .iter()
        .filter_map(|owner| Some((string(owner, "alias")?, string(owner, "kind")?)))
        .collect();
    let aliases: HashSet<_> = owner_kinds.keys().copied().collect();
    if owner_kinds.len() != 4 || owners.len() != 4 {
        errors.push("expected four uniquely named owners".into());
    }
    if owners
        .iter()
        .filter(|owner| string(owner, "kind") == Some("REL"))
        .count()
        != 2
        || owners
            .iter()
            .filter(|owner| string(owner, "kind") == Some("PHY"))
            .count()
            != 2
    {
        errors.push("cohort must declare two REL and two PHY owners".into());
    }

    let principals: HashSet<_> = objects(manifest, "principals")
        .iter()
        .filter_map(|principal| string(principal, "id"))
        .collect();
    let mut grant_keys = HashSet::new();
    for grant in objects(manifest, "grants") {
        let principal = string(grant, "principal").unwrap_or("");
        let owner = string(grant, "owner").unwrap_or("");
        let kind = string(grant, "owner_kind").unwrap_or("");
        let actions = strings(grant, "actions");
        let key = (principal, owner);
        if !principals.contains(principal) || !aliases.contains(owner) {
            errors.push(format!(
                "grant references unknown principal or owner: {principal}/{owner}"
            ));
        }
        if owner_kinds.get(owner).copied() != Some(kind) {
            errors.push(format!("grant kind does not match owner alias {owner}"));
        }
        if actions.is_empty()
            || actions
                .iter()
                .any(|action| !["view", "manage", "operate"].contains(action))
        {
            errors.push(format!("grant for {principal}/{owner} has unclear actions"));
        }
        if !grant_keys.insert(key) {
            errors.push(format!(
                "duplicate or ambiguous grant for {principal}/{owner}"
            ));
        }
    }

    let scripts = objects(manifest, "conversation_scripts");
    let mut script_ids = HashSet::new();
    let mut conversation_ids = HashSet::new();
    let mut turn_ids = HashMap::new();
    let mut owner_conversations: HashMap<&str, usize> = HashMap::new();
    let mut accepted_turn_count = 0usize;
    let mut fork_count = 0usize;
    for script in &scripts {
        let id = string(script, "id").unwrap_or("");
        if id.is_empty() || !script_ids.insert(id) {
            errors.push(format!("missing or duplicate script ID: {id}"));
        }
        let owner = string(script, "owner").unwrap_or("");
        let conversation = string(script, "conversation_id").unwrap_or("");
        if !aliases.contains(owner)
            || owner_kinds.get(owner).copied() != Some("REL")
            || conversation.is_empty()
            || !conversation_ids.insert(conversation)
        {
            errors.push(format!(
                "script {id} has an invalid owner or duplicate/missing conversation ID"
            ));
        }
        *owner_conversations.entry(owner).or_default() += 1;
        let turns = objects(script, "turns");
        if turns.len() < 4 {
            errors.push(format!("script {id} has fewer than four authored turns"));
        }
        for turn in &turns {
            let turn_id = string(turn, "id").unwrap_or("");
            if turn_id.is_empty() || turn_ids.insert(turn_id, id).is_some() {
                errors.push(format!("missing or duplicate turn ID: {turn_id}"));
            }
            if turn.get("accepted").and_then(Value::as_bool) == Some(true) {
                accepted_turn_count += 1;
            } else {
                errors.push(format!("authored turn {turn_id} must be marked accepted"));
            }
            if string(turn, "content").unwrap_or("").is_empty() {
                errors.push(format!("authored turn {turn_id} has empty content"));
            }
        }
        for branch in objects(script, "branches") {
            fork_count += 1;
            let parent = string(branch, "parent_turn_id").unwrap_or("");
            let children = strings(branch, "child_turn_ids");
            let canonical = string(branch, "canonical_child_id").unwrap_or("");
            let local: HashSet<_> = turns.iter().filter_map(|turn| string(turn, "id")).collect();
            if string(branch, "id").is_none_or(str::is_empty)
                || !local.contains(parent)
                || children.len() < 2
                || children.iter().copied().collect::<HashSet<_>>().len() != children.len()
                || children.contains(&parent)
                || children.iter().any(|child| !local.contains(child))
                || !children.contains(&canonical)
            {
                errors.push(format!(
                    "branch in {id} has inconsistent parent/child/canonical references"
                ));
            }
        }
    }
    if scripts.len() != 12 {
        errors.push(format!("expected 12 scripts, found {}", scripts.len()));
    }
    if owner_conversations.get("rel-lantern") != Some(&8)
        || owner_conversations.get("rel-cascade") != Some(&4)
        || owner_conversations.len() != 2
    {
        errors.push("conversation allocation must be exactly Lantern=8 and Cascade=4".into());
    }
    if accepted_turn_count != 48 {
        errors.push(format!(
            "expected exactly 48 authored accepted turns, found {accepted_turn_count}"
        ));
    }
    if accepted_turn_count < 40 {
        errors.push(format!(
            "expected at least 40 accepted turns, found {accepted_turn_count}"
        ));
    }
    if fork_count < 3 {
        errors.push(format!(
            "expected at least three branch forks, found {fork_count}"
        ));
    }

    let mut source_ids = HashSet::new();
    for source in objects(manifest, "source_references") {
        let source_id = string(source, "id").unwrap_or("");
        let script_id = string(source, "script_id").unwrap_or("");
        let turn_id = string(source, "turn_id").unwrap_or("");
        let span = string(source, "exact_span").unwrap_or("");
        let route = source.get("expected_route").unwrap_or(&Value::Null);
        let route_owner = string(route, "owner").unwrap_or("");
        let script = scripts
            .iter()
            .find(|script| string(script, "id") == Some(script_id));
        let source_turn = script.and_then(|script| {
            objects(script, "turns")
                .into_iter()
                .find(|turn| string(turn, "id") == Some(turn_id))
        });
        if source_id.is_empty()
            || !source_ids.insert(source_id)
            || source_turn.is_none()
            || span.is_empty()
            || !source_turn.is_some_and(|turn| string(turn, "content").unwrap_or("").contains(span))
        {
            errors.push(format!(
                "source reference {source_id} does not match an authored source span"
            ));
        }
        if !aliases.contains(route_owner)
            || owner_kinds.get(route_owner).copied() != string(route, "owner_kind")
        {
            errors.push(format!(
                "source reference {source_id} has invalid routed owner"
            ));
        }
        if string(route, "class") == Some("user") {
            let principal = string(route, "principal").unwrap_or("");
            let grants_match = objects(manifest, "grants").iter().any(|grant| {
                string(grant, "principal") == Some(principal)
                    && string(grant, "owner") == Some(route_owner)
                    && strings(grant, "actions").contains(&"operate")
            });
            if owner_kinds.get(route_owner).copied() != Some("PHY") || !grants_match {
                errors.push(format!(
                    "private source {source_id} lacks a matching scoped PHY grant"
                ));
            }
        }
    }

    if objects(manifest, "logical_history_cuts").is_empty()
        || objects(manifest, "retrieval_acceptance_cases").is_empty()
        || objects(manifest, "adversarial_disruptions").is_empty()
        || manifest
            .get("allowed_owner_routing")
            .and_then(Value::as_object)
            .is_none_or(|routes| routes.is_empty())
        || manifest
            .get("expected_state")
            .and_then(Value::as_object)
            .is_none_or(|state| state.is_empty())
    {
        errors.push(
            "manifest is missing a mandatory routing/state/history/retrieval/adversarial section"
                .into(),
        );
    }

    let observations = objects(manifest, "expected_vs_observed");
    let observed_cases: HashSet<_> = observations
        .iter()
        .filter_map(|item| string(item, "case"))
        .collect();
    for number in 1..=20 {
        let case = format!("G4-{number:02}");
        if !observed_cases.contains(case.as_str()) {
            errors.push(format!("missing expected-vs-observed mapping for {case}"));
        }
    }
    for case in observed_cases {
        let count = observations
            .iter()
            .filter(|item| string(item, "case") == Some(case))
            .count();
        if count != 1 {
            errors.push(format!(
                "expected-vs-observed mapping for {case} is duplicated"
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn objects<'a>(value: &'a Value, key: &str) -> Vec<&'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn strings<'a>(value: &'a Value, key: &str) -> Vec<&'a str> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}
