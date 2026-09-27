import json
from pathlib import Path

OUT = Path("..") / "Reliquary" / "fixtures" / "local" / "calibration" / "entity-store-resolution-large-v1"

COUNTS = {
    "same_name_same_type": 30,
    "alias_resolution": 20,
    "path_namespace": 20,
    "renamed_entity": 15,
    "stale_current": 15,
    "multi_candidate": 15,
    "create_new_close": 15,
    "unresolved_ambiguous": 10,
    "reject_non_entity": 5,
    "owner_scope": 5,
}


def mention(text, surface):
    start = text.index(surface)
    return {"field": "content", "start_byte": start, "end_byte": start + len(surface), "text": surface}


def entity(eid, owner, canonical, aliases, kind, summary, evidence):
    return {
        "entity_id": eid, "owner_id": owner, "canonical_name": canonical,
        "aliases": aliases, "kind": kind, "summary": summary,
        "evidence": [{"memory_id": f"{eid}-evidence", "text": evidence}],
    }


def add_case(entities, queries, gold, number, category, surface, content, candidates,
             decision, target_index=None, owner=None, title=None):
    qid = f"q{number:04d}"
    owner = owner or f"owner-{qid}"
    entities.extend(candidates)
    queries.append({
        "memory_id": qid, "owner_id": owner,
        "title": title or content.split(".")[0], "content": content,
        "mentions": [mention(content, surface)], "category": category,
    })
    target = candidates[target_index]["entity_id"] if target_index is not None else None
    gold[qid] = {"expected_decision": decision, "expected_entity_id": target, "category": category}


def build():
    entities, queries, gold = [], [], {}
    n = 0

    # Two-candidate families rotate the target position without making the surface informative.
    for category, count in [("same_name_same_type", 30), ("alias_resolution", 20),
                            ("renamed_entity", 15), ("stale_current", 15)]:
        for j in range(count):
            n += 1; qid = f"q{n:04d}"; owner = f"owner-{qid}"; target = j % 2
            if category == "same_name_same_type":
                surface = f"Beacon {j + 1}"; kind = "service"
                facts = [("billing", "invoice reconciliation", "Rails billing worker"),
                         ("telemetry", "sensor ingestion", "Rust telemetry worker")]
                content = f"{surface} handled the {facts[target][1]} alert in the {facts[target][0]} deployment; its {facts[target][2]} logs show the retry fix."
                candidates = [entity(f"{qid}-entity-{i}", owner, surface, [surface], kind,
                    f"{surface} is the {facts[i][1]} service for the {facts[i][0]} domain.",
                    f"The {facts[i][2]} owns {facts[i][1]} in the {facts[i][0]} deployment.") for i in range(2)]
            elif category == "alias_resolution":
                surface = f"Beacon gateway {j + 1}"; canonical = [f"Gateway North {j + 1}", f"Gateway South {j + 1}"]
                domains = [("eu-west", "payments", "Stripe"), ("us-east", "search", "OpenSearch")]
                content = f"The {surface} in {domains[target][0]} serves {domains[target][1]} traffic backed by {domains[target][2]}; rotate its certificate."
                candidates = [entity(f"{qid}-entity-{i}", owner, canonical[i], [surface, f"gateway-{j + 1}"], "service",
                    f"{canonical[i]} is the {domains[i][1]} gateway in {domains[i][0]}.",
                    f"Its production backend is {domains[i][2]} for {domains[i][1]} traffic.") for i in range(2)]
            elif category == "renamed_entity":
                surface = f"Project Aurora {j + 1}"; names = [f"Project Aurora {j + 1}", f"Project Borealis {j + 1}"]
                facts = [("mobile checkout", "Kotlin", "Android"), ("warehouse routing", "Go", "robot fleet")]
                content = f"{surface}, formerly called {('Checkout Dawn' if target == 0 else 'Route Dawn')}, owns the {facts[target][0]} work in {facts[target][1]} for the {facts[target][2]}."
                candidates = [entity(f"{qid}-entity-{i}", owner, names[i], [surface, ('Checkout Dawn' if i == 0 else 'Route Dawn')], "project",
                    f"{names[i]} owns {facts[i][0]} for the {facts[i][2]} team.",
                    f"Historical records call it {('Checkout Dawn' if i == 0 else 'Route Dawn')}; the implementation uses {facts[i][1]}.") for i in range(2)]
            else:
                surface = f"Atlas worker {j + 1}"
                current = target
                roles = [("queue consumer", "Kafka"), ("batch exporter", "S3")]
                content = f"The current {surface} is the {roles[current][0]} using {roles[current][1]}; that current deployment owns today's incident."
                candidates = []
                for i in range(2):
                    status = "current" if i == current else "retired"
                    candidates.append(entity(
                        f"{qid}-entity-{i}", owner, surface, [surface], "worker",
                        f"{surface} is the {roles[i][0]} for {roles[i][1]} ({status} role).",
                        f"Operational history marks this worker as {status}; it handled {roles[i][0]} jobs using {roles[i][1]}."
                    ))
            add_case(entities, queries, gold, n, category, surface, content, candidates, "resolve_existing", target)

    for j in range(20):
        n += 1; qid = f"q{n:04d}"; owner = f"owner-{qid}"; target = j % 2; surface = "settings.toml"
        repos = [("Reliquary", "storage", "retention"), ("Space Rocks", "lobby", "matchmaking")]
        content = f"Update {surface} in the {repos[target][0]} {repos[target][1]} component to change {repos[target][2]} limits."
        candidates = [entity(f"{qid}-entity-{i}", owner, surface, [surface], "file",
            f"{surface} belongs to the {repos[i][1]} component of the {repos[i][0]} repository.",
            f"The {repos[i][0]} {repos[i][1]} component reads this file for {repos[i][2]} settings.") for i in range(2)]
        add_case(entities, queries, gold, n, "path_namespace", surface, content, candidates, "resolve_existing", target)

    for j in range(15):
        n += 1; qid = f"q{n:04d}"; owner = f"owner-{qid}"; surface = f"Orion device {j + 1}"
        candidate_count = 3 + (j % 2)
        locations = ["cold storage", "loading dock", "field office", "repair bay"]
        functions = ["refrigeration", "shipping", "inventory", "maintenance"]
        candidates = [entity(
            f"{qid}-entity-{i}", owner, surface, [surface], "device",
            f"{surface} is assigned to {locations[i]}.",
            f"Its serial family serves {functions[i]} operations at the {locations[i]}."
        ) for i in range(candidate_count)]
        target = j % candidate_count
        content = f"Inspect {surface} at the {locations[target]}; its {functions[target]} sensor reported a fault."
        add_case(entities, queries, gold, n, "multi_candidate", surface, content, candidates, "resolve_existing", target)

    for j in range(15):
        n += 1; qid = f"q{n:04d}"; owner = f"owner-{qid}"; surface = f"Delta rack {j + 1}"
        candidates = [entity(f"{qid}-entity-{i}", owner, surface, [surface], "rack", f"{surface} holds {('API credentials' if i == 0 else 'build artifacts')}.", f"The rack is located in {('security vault' if i == 0 else 'release storage')}.") for i in range(2)]
        new = "prototype batteries" if j % 2 == 0 else "archived paper schematics"
        content = f"Create a separate identity for {surface}: this one holds {new} in the lab, unlike the existing racks."
        add_case(entities, queries, gold, n, "create_new_close", surface, content, candidates, "create_new")

    for j in range(10):
        n += 1; qid = f"q{n:04d}"; owner = f"owner-{qid}"; surface = f"Nova queue {j + 1}"
        candidates = [entity(f"{qid}-entity-{i}", owner, surface, [surface], "queue", f"{surface} serves {('email' if i == 0 else 'billing')} jobs.", "Both queues are active production queues with no distinguishing fact in this memory.") for i in range(2)]
        content = f"{surface} had a delay today. Please investigate it."
        add_case(entities, queries, gold, n, "unresolved_ambiguous", surface, content, candidates, "unresolved")

    for j in range(5):
        n += 1; qid = f"q{n:04d}"; owner = f"owner-{qid}"; surface = ["review item", "next step", "current attempt", "temporary result", "meeting topic"][j]
        content = f"The {surface} was discussed in the meeting, but it was only a sentence-local description rather than a durable named object."
        add_case(entities, queries, gold, n, "reject_non_entity", surface, content, [], "reject")

    for j in range(5):
        n += 1; qid = f"q{n:04d}"; owner = f"owner-{qid}"; surface = f"Core API {j + 1}"; target = j % 2
        candidates = [entity(f"{qid}-same-owner", owner, surface, [surface], "service", "The current owner's API serves internal clients.", "This owner operates the API for the current workspace.")]
        decoy_owner = f"other-owner-{qid}"
        entities.append(entity(f"{qid}-other-owner", decoy_owner, surface, [surface], "service", "A different owner operates a similarly named API.", "This is outside the query owner's scope."))
        if target == 0:
            content = f"The {surface} for this workspace serves the internal admin console; rotate its token."
            decision, idx = "resolve_existing", 0
        else:
            content = f"Create a new {surface} for this workspace's public partner API; the existing internal API is not this service."
            decision, idx = "create_new", None
        add_case(entities, queries, gold, n, "owner_scope", surface, content, candidates, decision, idx)

    assert n == 150
    OUT.mkdir(parents=True, exist_ok=True)
    for name, rows in (("entities.jsonl", entities), ("queries.jsonl", queries)):
        (OUT / name).write_text("\n".join(json.dumps(row, separators=(",", ":")) for row in rows) + "\n", encoding="utf-8")
    (OUT / "gold.json").write_text(json.dumps(gold, indent=2) + "\n", encoding="utf-8")
    (OUT / "README.md").write_text("""# Large Entity-ID resolution calibration corpus\n\nGenerated by `examples/entity_store_resolution_experiment/make_large_fixture.py`.\n\nThis deterministic, scorer-only corpus contains exactly 150 one-mention queries across ten categories. Candidate discovery is lexical and owner-scoped; summaries and evidence provide the semantic and temporal facts needed for the gold decision. Gold is never passed to inference. The corpus intentionally includes alias collisions, namespace collisions, renamed and stale records, bounded multi-candidate permutations, explicit new identities, ambiguity, rejection, and owner-scope cases.\n\nCategory counts: same_name_same_type=30, alias_resolution=20, path_namespace=20, renamed_entity=15, stale_current=15, multi_candidate=15, create_new_close=15, unresolved_ambiguous=10, reject_non_entity=5, owner_scope=5.\n""", encoding="utf-8")
    print(f"queries={len(queries)} entities={len(entities)} categories={COUNTS}")


if __name__ == "__main__":
    build()
