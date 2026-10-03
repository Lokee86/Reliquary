//! Std-only Phase 0B G2 reference model.
//! This is an adversarial executable contract, not production behavior or a proof
//! that Reliquary's current storage/runtime satisfies G2.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Principal(&'static str);
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Phy(&'static str);
#[derive(Clone, Debug, Eq, PartialEq)]
enum Role {
    User,
    Agent,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Turn {
    id: &'static str,
    role: Role,
    actor_principal: Option<Principal>,
    generation_id: Option<&'static str>,
    generation_initiator: Option<Principal>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Provenance {
    nodes: Vec<&'static str>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
enum Scope {
    Personal,
    Rel,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Candidate {
    mutation_id: &'static str,
    proposition: &'static str,
    scope: Scope,
    provenance: Provenance,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Route {
    destination: Phy,
    principal: Principal,
    candidate: Candidate,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Memory {
    principal: Principal,
    mutation_id: &'static str,
    proposition: &'static str,
    source_nodes: Vec<&'static str>,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ReceiptRef {
    phy: Phy,
    mutation_id: &'static str,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Receipt {
    refs: Vec<ReceiptRef>,
    rel_facts: Vec<&'static str>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
enum Outcome {
    Quarantined(&'static str),
    Pending,
    Complete(Receipt),
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct EpisodePlan {
    routes: Vec<Route>,
    rel_facts: Vec<&'static str>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Journal {
    frozen_plan: Option<EpisodePlan>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Stores {
    phys: BTreeMap<Phy, BTreeMap<&'static str, Memory>>,
    rel_facts: BTreeSet<&'static str>,
    completion: Option<Receipt>,
}

fn source_principal<'a>(id: &str, turns: &'a BTreeMap<&str, Turn>) -> Option<&'a Principal> {
    let turn = turns.get(id)?;
    match turn.role {
        Role::User => turn.actor_principal.as_ref(),
        Role::Agent => {
            // Initiator is durable authorship context, never personal-fact evidence by itself.
            turn.generation_id?;
            turn.generation_initiator.as_ref()
        }
    }
}

fn plan(
    candidate: &Candidate,
    turns: &BTreeMap<&str, Turn>,
    principal_phy: &BTreeMap<Principal, Phy>,
) -> Result<Option<Route>, &'static str> {
    if candidate.scope == Scope::Rel {
        return Ok(None);
    }
    let mut principals = BTreeSet::new();
    let mut has_principal_authored_evidence = false;
    for node in &candidate.provenance.nodes {
        let turn = turns.get(node).ok_or("unknown provenance source")?;
        if turn.role == Role::User {
            has_principal_authored_evidence = true;
        }
        principals.insert(
            source_principal(node, turns)
                .cloned()
                .ok_or("unknown source attribution")?,
        );
    }
    if !has_principal_authored_evidence {
        return Err("personal fact lacks principal-authored evidence");
    }
    if principals.len() != 1 {
        return Err("ambiguous personal provenance");
    }
    let principal = principals.into_iter().next().unwrap();
    let destination = principal_phy
        .get(&principal)
        .cloned()
        .ok_or("principal has no trusted PHY mapping")?;
    if destination.0 != principal.0 {
        return Err("PHY principal identity mismatch");
    }
    Ok(Some(Route {
        destination,
        principal,
        candidate: candidate.clone(),
    }))
}

fn build_plan(
    candidates: &[Candidate],
    turns: &BTreeMap<&str, Turn>,
    principal_phy: &BTreeMap<Principal, Phy>,
) -> Result<EpisodePlan, &'static str> {
    let mut routes = Vec::new();
    let mut rel_facts = Vec::new();
    for candidate in candidates {
        match plan(candidate, turns, principal_phy)? {
            Some(route) => routes.push(route),
            None => rel_facts.push(candidate.proposition),
        }
    }
    routes.sort_by_key(|route| route.candidate.mutation_id);
    rel_facts.sort_unstable();
    Ok(EpisodePlan { routes, rel_facts })
}

fn publish_one(stores: &mut Stores, route: &Route) -> Result<(), &'static str> {
    let owner = stores.phys.entry(route.destination.clone()).or_default();
    match owner.get(route.candidate.mutation_id) {
        Some(old)
            if old.principal == route.principal
                && old.mutation_id == route.candidate.mutation_id
                && old.proposition == route.candidate.proposition
                && old.source_nodes == route.candidate.provenance.nodes =>
        {
            Ok(())
        }
        Some(_) => Err("same mutation ID has different semantics"),
        None => {
            owner.insert(
                route.candidate.mutation_id,
                Memory {
                    principal: route.principal.clone(),
                    mutation_id: route.candidate.mutation_id,
                    proposition: route.candidate.proposition,
                    source_nodes: route.candidate.provenance.nodes.clone(),
                },
            );
            Ok(())
        }
    }
}

/// Execute/recover one complete Episode. The full semantic plan and all owner
/// destinations freeze before the first PHY write. Retries never call `build_plan`
/// while a frozen plan exists. `fail_after` models a crash after N writes.
fn execute_episode(
    candidates_if_new: &[Candidate],
    turns: &BTreeMap<&str, Turn>,
    current_principal_phy: &BTreeMap<Principal, Phy>,
    journal: &mut Journal,
    stores: &mut Stores,
    fail_after: Option<usize>,
) -> Outcome {
    if let Some(receipt) = &stores.completion {
        return Outcome::Complete(receipt.clone());
    }
    if journal.frozen_plan.is_none() {
        match build_plan(candidates_if_new, turns, current_principal_phy) {
            Ok(plan) => journal.frozen_plan = Some(plan),
            Err(reason) => return Outcome::Quarantined(reason),
        }
    }
    let frozen = journal.frozen_plan.clone().unwrap();
    // Revalidate the fixed grant/owner binding; never substitute a newly mapped PHY.
    for route in &frozen.routes {
        if current_principal_phy.get(&route.principal) != Some(&route.destination) {
            return Outcome::Pending;
        }
    }
    let mut writes = 0;
    for route in &frozen.routes {
        if publish_one(stores, route).is_err() {
            return Outcome::Pending;
        }
        writes += 1;
        if fail_after == Some(writes) {
            return Outcome::Pending;
        }
    }
    // The REL-visible facts and external refs appear only with the final receipt.
    stores.rel_facts.extend(frozen.rel_facts.iter().copied());
    let receipt = Receipt {
        refs: frozen
            .routes
            .iter()
            .map(|route| ReceiptRef {
                phy: route.destination.clone(),
                mutation_id: route.candidate.mutation_id,
            })
            .collect(),
        rel_facts: frozen.rel_facts,
    };
    stores.completion = Some(receipt.clone());
    Outcome::Complete(receipt)
}

fn turns() -> BTreeMap<&'static str, Turn> {
    let a = Principal("phy-a");
    let b = Principal("phy-b");
    BTreeMap::from([
        (
            "u-a",
            Turn {
                id: "u-a",
                role: Role::User,
                actor_principal: Some(a.clone()),
                generation_id: None,
                generation_initiator: None,
            },
        ),
        (
            "u-b",
            Turn {
                id: "u-b",
                role: Role::User,
                actor_principal: Some(b.clone()),
                generation_id: None,
                generation_initiator: None,
            },
        ),
        // Parent is u-a, but generation was explicitly initiated by B.
        (
            "agent-b",
            Turn {
                id: "agent-b",
                role: Role::Agent,
                actor_principal: None,
                generation_id: Some("gen-b"),
                generation_initiator: Some(b),
            },
        ),
        (
            "legacy-agent",
            Turn {
                id: "legacy-agent",
                role: Role::Agent,
                actor_principal: None,
                generation_id: None,
                generation_initiator: None,
            },
        ),
    ])
}
fn mappings() -> BTreeMap<Principal, Phy> {
    BTreeMap::from([
        (Principal("phy-a"), Phy("phy-a")),
        (Principal("phy-b"), Phy("phy-b")),
    ])
}
fn personal(id: &'static str, text: &'static str, nodes: Vec<&'static str>) -> Candidate {
    Candidate {
        mutation_id: id,
        proposition: text,
        scope: Scope::Personal,
        provenance: Provenance { nodes },
    }
}
fn rel_fact(id: &'static str, text: &'static str) -> Candidate {
    Candidate {
        mutation_id: id,
        proposition: text,
        scope: Scope::Rel,
        provenance: Provenance { nodes: vec![] },
    }
}

#[test]
fn generation_initiator_is_distinct_from_actor_and_never_evidence_alone() {
    let t = turns();
    assert_eq!(source_principal("agent-b", &t), Some(&Principal("phy-b")));
    assert_eq!(source_principal("legacy-agent", &t), None);
    let candidate = personal("agent-only", "B likes X", vec!["agent-b"]);
    assert_eq!(
        plan(&candidate, &t, &mappings()),
        Err("personal fact lacks principal-authored evidence")
    );
    let conflicting = personal("conflict", "A likes X", vec!["u-a", "agent-b"]);
    assert_eq!(
        plan(&conflicting, &t, &mappings()),
        Err("ambiguous personal provenance")
    );
}

#[test]
fn mixed_episode_routes_personal_facts_by_provenance_and_keeps_rel_fact_rel_local() {
    let t = turns();
    let m = mappings();
    let mut stores = Stores::default();
    let mut journal = Journal::default();
    let candidates = vec![
        personal("m-a", "A prefers concise answers", vec!["u-a"]),
        personal("m-b", "B prefers detailed answers", vec!["u-b"]),
        rel_fact("r-1", "Project uses Rust"),
    ];
    // Current active selection of B's PHY is intentionally not part of the API contract.
    let current_selected_phy = Phy("phy-b");
    let result = execute_episode(&candidates, &t, &m, &mut journal, &mut stores, None);
    assert!(matches!(result, Outcome::Complete(_)));
    assert_eq!(current_selected_phy, Phy("phy-b"));
    assert!(stores.phys[&Phy("phy-a")].contains_key("m-a"));
    assert!(!stores.phys[&Phy("phy-a")].contains_key("m-b"));
    assert!(stores.phys[&Phy("phy-b")].contains_key("m-b"));
    assert!(
        !stores
            .phys
            .get(&Phy("phy-b"))
            .is_some_and(|owner| owner.contains_key("m-a"))
    );
    assert!(stores.rel_facts.contains("Project uses Rust"));
    assert_eq!(stores.completion.as_ref().unwrap().refs.len(), 2);
}

#[test]
fn ambiguous_or_legacy_personal_provenance_is_quarantined_without_rel_fallback() {
    let t = turns();
    let m = mappings();
    let mut stores = Stores::default();
    let mut journal = Journal::default();
    let mixed = personal(
        "m-mixed",
        "A and B share a private fact",
        vec!["u-a", "u-b"],
    );
    assert_eq!(
        execute_episode(&[mixed], &t, &m, &mut journal, &mut stores, None),
        Outcome::Quarantined("ambiguous personal provenance")
    );
    let legacy = personal("m-legacy", "unknown old assertion", vec!["legacy-agent"]);
    assert_eq!(
        execute_episode(&[legacy], &t, &m, &mut journal, &mut stores, None),
        Outcome::Quarantined("unknown source attribution")
    );
    assert!(stores.phys.is_empty());
    assert!(
        stores.rel_facts.is_empty(),
        "personal ambiguity must not fall back into shared REL state"
    );
}

#[test]
fn phy_identity_mismatch_fails_closed() {
    let t = turns();
    let mut m = mappings();
    m.insert(Principal("phy-a"), Phy("phy-b"));
    let mut stores = Stores::default();
    let mut journal = Journal::default();
    let c = personal("m-a", "A preference", vec!["u-a"]);
    assert_eq!(
        execute_episode(&[c], &t, &m, &mut journal, &mut stores, None),
        Outcome::Quarantined("PHY principal identity mismatch")
    );
    assert!(stores.phys.is_empty());
}

#[test]
fn multi_phy_partial_write_recovers_from_frozen_plan_after_restart() {
    let t = turns();
    let m = mappings();
    let candidates = vec![
        personal("m-a", "A prefers concise answers", vec!["u-a"]),
        personal("m-b", "B prefers detailed answers", vec!["u-b"]),
    ];
    let mut journal = Journal::default();
    let mut stores = Stores::default();
    assert!(matches!(
        execute_episode(&candidates, &t, &m, &mut journal, &mut stores, Some(1)),
        Outcome::Pending
    ));
    assert_eq!(
        journal.frozen_plan.as_ref().unwrap().routes.len(),
        2,
        "complete route plan is durable before first write"
    );
    assert!(stores.completion.is_none());
    assert_eq!(stores.phys.values().map(BTreeMap::len).sum::<usize>(), 1);
    // Simulate crash/restart and a now-missing/changed classifier result: saved plan wins.
    let changed_candidates = vec![personal("m-new", "model redrafted this", vec!["u-b"])];
    let mut journal = journal.clone();
    let mut stores = stores.clone();
    let result = execute_episode(&changed_candidates, &t, &m, &mut journal, &mut stores, None);
    assert!(matches!(result, Outcome::Complete(_)));
    assert!(stores.phys[&Phy("phy-a")].contains_key("m-a"));
    assert!(stores.phys[&Phy("phy-b")].contains_key("m-b"));
    assert!(!stores.phys[&Phy("phy-b")].contains_key("m-new"));
    assert_eq!(stores.phys.values().map(BTreeMap::len).sum::<usize>(), 2);
}

#[test]
fn changed_mapping_holds_frozen_intent_without_retarget_or_receipt() {
    let t = turns();
    let m = mappings();
    let candidates = vec![personal("m-a", "A preference", vec!["u-a"])];
    let mut journal = Journal::default();
    let mut stores = Stores::default();
    assert!(matches!(
        execute_episode(&candidates, &t, &m, &mut journal, &mut stores, Some(1)),
        Outcome::Pending
    ));
    let mut changed = m.clone();
    changed.insert(Principal("phy-a"), Phy("phy-b"));
    assert_eq!(
        execute_episode(&[], &t, &changed, &mut journal, &mut stores, None),
        Outcome::Pending
    );
    assert_eq!(
        journal.frozen_plan.as_ref().unwrap().routes[0].destination,
        Phy("phy-a")
    );
    assert!(stores.completion.is_none());
    assert!(
        !stores
            .phys
            .get(&Phy("phy-b"))
            .is_some_and(|owner| owner.contains_key("m-a"))
    );
}

#[test]
fn completion_receipt_replay_survives_crash_before_intent_cleanup() {
    let t = turns();
    let m = mappings();
    let candidates = vec![personal("m-a", "A preference", vec!["u-a"])];
    let mut journal = Journal::default();
    let mut stores = Stores::default();
    let first = execute_episode(&candidates, &t, &m, &mut journal, &mut stores, None);
    // Model a crash after the REL semantic receipt but before sidecar cleanup.
    let replay = execute_episode(&[], &t, &BTreeMap::new(), &mut journal, &mut stores, None);
    assert_eq!(replay, first);
    assert_eq!(stores.phys[&Phy("phy-a")].len(), 1);
}

#[test]
fn shared_phy_owner_is_idempotent_for_two_instances_of_same_principal() {
    let t = turns();
    let m = mappings();
    let candidates = vec![personal("shared-1", "A preference", vec!["u-a"])];
    let mut journal = Journal::default();
    let mut stores = Stores::default();
    let first = execute_episode(&candidates, &t, &m, &mut journal, &mut stores, None);
    let second = execute_episode(&candidates, &t, &m, &mut journal, &mut stores, None);
    assert_eq!(first, second);
    assert_eq!(stores.phys[&Phy("phy-a")].len(), 1);
}
