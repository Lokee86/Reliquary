import json, pathlib
OUT=pathlib.Path(r"..\Reliquary\fixtures\local\calibration\entity-disambiguation-adversarial-v1")
G=[
("g01","Phoenix","Project Phoenix is the deployment modernization project for billing.","Phoenix is the destination city for next month's expo.","The Phoenix project cannot deploy until the CI failure is fixed.","resolve_existing",0),
("g02","Jordan","Jordan Lee is a backend developer who reviews authentication pull requests.","Jordan is the country being evaluated for regional distribution.","Jordan approved the authentication pull request.","resolve_existing",0),
("g03","Helix","Helix is the code editor used for Rust work on this machine.","Helix is the internal telemetry ingestion service.","Helix keybindings should map save to Ctrl-S.","resolve_existing",0),
("g04","Mercury","Mercury is the production PostgreSQL cluster with read replicas.","Mercury is the warehouse temperature sensor at loading bay three.","Mercury replica lag exceeded thirty seconds.","resolve_existing",0),
("g05","Atlas","Atlas is the electric forklift assigned to warehouse aisle four.","Atlas is the internal REST API service with a /health endpoint.","Atlas /health started returning 503 after the deploy.","resolve_existing",1),
("g06","config.json","Warlock stores its local provider configuration in config.json.","Space Rocks stores game server defaults in config.json.","Space Rocks needs config.json to enable the new lobby timeout.","resolve_existing",1),
("g07","src/main.rs","Reliquary's CLI entry point is src/main.rs.","Archivist's Rust binary also has an src/main.rs entry point.","Reliquary should move logging initialization earlier in src/main.rs.","resolve_existing",0),
("g08","API server","The Space Rocks API server is the Rails service backing account and lobby APIs.","Warlock's API server exposes local agent and workspace endpoints.","Warlock's API server needs a new /agents endpoint.","resolve_existing",1),
("g09","Beacon","Beacon is the Bluetooth tracking tag attached to pallet 42.","Beacon is the notification delivery service used by the web application.","Beacon should retry failed push deliveries three times.","resolve_existing",1),
("g10","Sarah","Sarah Chen is the product designer for the mobile app.","Sarah Patel is the payroll accountant.","Sarah approved the payroll adjustment for September.","resolve_existing",1),
("g11","Orion","Orion is the customer migration project scheduled for Q4.","Orion is the astronomy dataset used by the research demo.","The warehouse robot named Orion needs its left drive motor replaced before the warehouse shift.","create_new",-1),
("g12","Delta","Delta Ltd is the concrete supplier for the Richmond project.","Delta is the geometry-diff module in the drawing pipeline.","The conference room named Delta needs a new projector installed before Monday's meeting.","create_new",-1),
("g13","Nimbus","Nimbus is the cloud backup service used by the office.","Nimbus is the web design system used by the marketing site.","The forklift named Nimbus is due for its 500-hour maintenance inspection.","create_new",-1),
("g14","Apollo","Apollo is the mission-tracking application used by operations.","Apollo is the infrastructure team responsible for Kubernetes.","The PostgreSQL cluster named Apollo is reporting high write latency after the index rebuild.","create_new",-1),
("g15","Echo","Echo is the customer-support chatbot.","Echo is the build cache service used by CI.","Echo was updated yesterday.","unresolved",-1),
("g16","Nova","Nova is the Android application for field technicians.","Nova is the Linux build server used by the release pipeline.","Nova is down.","unresolved",-1),
("g17","Quarry","Quarry is the estimating application for concrete takeoffs.","Quarry is the staging database for material pricing.","Quarry changed configuration this morning.","unresolved",-1),
]
R=[]; C=[]; gold={}
for gid,s,a,b,q,d,t in G:
    owner="owner-"+gid
    for suf,content,role in [("a",a,"anchor"),("b",b,"anchor"),("q",q,"query")]:
        mid=f"{gid}-{suf}"; start=content.lower().find(s.lower())
        mention={"field":"content","start_byte":start,"end_byte":start+len(s),"text":content[start:start+len(s)]}
        C.append({"memory_id":mid,"owner_id":owner,"owner_kind":"rel","title":content.split(".")[0],"content":content})
        R.append({"memory_id":mid,"owner_kind":"rel","actual_entity_mentions":[mention],"expected_entity_mentions":[mention],"tags":[role,gid]})
    gold[f"{gid}-q"]={"surface":s,"expected_decision":d,"expected_target_candidate_index":t}
OUT.mkdir(parents=True,exist_ok=True)
(OUT/"results.jsonl").write_text("\n".join(json.dumps(x) for x in R)+"\n",encoding="utf-8")
(OUT/"candidates.jsonl").write_text("\n".join(json.dumps(x) for x in C)+"\n",encoding="utf-8")
(OUT/"gold.json").write_text(json.dumps(gold,indent=2),encoding="utf-8")
print(f"groups={len(G)} rows={len(R)} queries={len(gold)}")
