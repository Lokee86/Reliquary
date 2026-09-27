import json
from pathlib import Path

OUT = Path(r"..\Reliquary\fixtures\local\calibration\entity-store-resolution-adversarial-v1")
CASES = [
("Phoenix","project","city","Phoenix project deployment is blocked by a CI failure.","resolve_existing","a"),
("Jordan","person","country","Jordan approved the authentication pull request.","resolve_existing","a"),
("Helix","editor","service","Helix keybindings should map save to Ctrl-S.","resolve_existing","a"),
("Mercury","cluster","sensor","Mercury replica lag exceeded thirty seconds.","resolve_existing","a"),
("Atlas","forklift","service","Atlas /health started returning 503 after the deploy.","resolve_existing","b"),
("config.json","file","file","Space Rocks needs config.json to enable the new lobby timeout.","resolve_existing","b"),
("src/main.rs","file","file","Reliquary should move logging initialization earlier in src/main.rs.","resolve_existing","a"),
("API server","service","service","Warlock's API server needs a new /agents endpoint.","resolve_existing","b"),
("Beacon","tag","service","Beacon should retry failed push deliveries three times.","resolve_existing","b"),
("Sarah","person","person","Sarah approved the payroll adjustment for September.","resolve_existing","b"),
("Orion","project","dataset","The warehouse robot named Orion needs its left drive motor replaced before the warehouse shift.","create_new",None),
("Delta","supplier","module","The conference room named Delta needs a new projector installed before Monday's meeting.","create_new",None),
("Nimbus","service","design-system","The forklift named Nimbus is due for its 500-hour maintenance inspection.","create_new",None),
("Apollo","application","team","The PostgreSQL cluster named Apollo is reporting high write latency after the index rebuild.","create_new",None),
("Echo","chatbot","service","Echo was updated yesterday.","unresolved",None),
("Nova","application","server","Nova is down.","unresolved",None),
("Quarry","application","database","Quarry changed configuration this morning.","unresolved",None),
]
ANCHORS = {
"Phoenix":("Project Phoenix is the deployment modernization project for billing.","The Phoenix project cannot deploy until the CI failure is fixed."),
"Jordan":("Jordan Lee is a backend developer who reviews authentication pull requests.","Jordan is the country being evaluated for regional distribution."),
"Helix":("Helix is the code editor used for Rust work on this machine.","Helix is the internal telemetry ingestion service."),
"Mercury":("Mercury is the production PostgreSQL cluster with read replicas.","Mercury is the warehouse temperature sensor at loading bay three."),
"Atlas":("Atlas is the electric forklift assigned to warehouse aisle four.","Atlas is the internal REST API service with a /health endpoint."),
"config.json":("Warlock stores its local provider configuration in config.json.","Space Rocks stores game server defaults in config.json."),
"src/main.rs":("Reliquary's CLI entry point is src/main.rs.","Archivist's Rust binary also has an src/main.rs entry point."),
"API server":("The Space Rocks API server is the Rails service backing account and lobby APIs.","Warlock's API server exposes local agent and workspace endpoints."),
"Beacon":("Beacon is the Bluetooth tracking tag attached to pallet 42.","Beacon is the notification delivery service used by the web application."),
"Sarah":("Sarah Chen is the product designer for the mobile app.","Sarah Patel is the payroll accountant."),
"Orion":("Orion is the customer migration project scheduled for Q4.","Orion is the astronomy dataset used by the research demo."),
"Delta":("Delta Ltd is the concrete supplier for the Richmond project.","Delta is the geometry-diff module in the drawing pipeline."),
"Nimbus":("Nimbus is the cloud backup service used by the office.","Nimbus is the web design system used by the marketing site."),
"Apollo":("Apollo is the mission-tracking application used by operations.","Apollo is the infrastructure team responsible for Kubernetes."),
"Echo":("Echo is the customer-support chatbot.","Echo is the build cache service used by CI."),
"Nova":("Nova is the Android application for field technicians.","Nova is the Linux build server used by the release pipeline."),
"Quarry":("Quarry is the estimating application for concrete takeoffs.","Quarry is the staging database for material pricing."),}

def span(text, surface):
    start = text.index(surface)
    return {"field":"content","start_byte":start,"end_byte":start+len(surface),"text":surface}
entities=[]; queries=[]; gold={}
for i,(surface,ka,kb,query,decision,target) in enumerate(CASES,1):
    gid=f"g{i:02d}"; owner=f"owner-{gid}"
    for suffix,kind,summary in [("a",ka,ANCHORS[surface][0]),("b",kb,ANCHORS[surface][1])]:
        eid=f"{gid}-entity-{suffix}"
        entities.append({"entity_id":eid,"owner_id":owner,"canonical_name":surface,"aliases":[surface],"kind":kind,"summary":summary,"evidence":[{"memory_id":f"{gid}-evidence-{suffix}","text":summary}]})
    mid=f"{gid}-q"; mention=span(query,surface)
    queries.append({"memory_id":mid,"owner_id":owner,"title":query.split(".")[0],"content":query,"mentions":[mention]})
    gold[mid]={"expected_decision":decision,"expected_entity_id":None if target is None else f"{gid}-entity-{target}"}
OUT.mkdir(parents=True,exist_ok=True)
for name,rows in [("entities.jsonl",entities),("queries.jsonl",queries)]:
    (OUT/name).write_text("\n".join(json.dumps(x) for x in rows)+"\n",encoding="utf-8")
(OUT/"gold.json").write_text(json.dumps(gold,indent=2)+"\n",encoding="utf-8")
(OUT/"README.md").write_text("Gold is scorer-only; inference receives entities and queries, never gold.\n",encoding="utf-8")
print(f"groups={len(CASES)} entities={len(entities)} queries={len(queries)}")
