import json, re
from pathlib import Path

SRC = Path(r"C:\!bin\workspace\Reliquary\fixtures\local\calibration\insomnia-entity-v2")
OUT = Path(r"C:\!bin\workspace\Reliquary\fixtures\local\calibration\entity-store-resolution-frozen-rel-v1")
OWNER = "proj-73eff05f-e65e-4fb4-b430-f9af2237b48b"
MEM = {r["memory_id"]: r for r in map(json.loads, (SRC/"candidates.jsonl").read_text(encoding="utf-8").splitlines())}
SHADOW = {r["memory_id"]: r for r in map(json.loads, (SRC/"runs"/"shadow-sol-low-entity-reusable-v2-2026-09-17"/"results.jsonl").read_text(encoding="utf-8").splitlines())}

Q = [
("be03db12d6e17d559bc4dfb8b647570c06032ea651615abba2625c36001ef93d","Go","resolve_existing","go-language","technology"),
("f05fa488483cefa39fa76a4f0359bb2e271dbb94dc5999961507958bb0c3c592","Go","resolve_existing","go-language","technology"),
("35a41e477a1c0779559569c18518f8b48e7ed4a1ef0e5f260194dd25f96ccd71","Go","resolve_existing","go-language","technology"),
("7250be66e4bc47021debf7e4353a09cc47c5102e2e98cba7a82667f55e01bbda","Go","resolve_existing","go-language","technology"),
("4eb92992ede1fc391faca9dacd92f13859595747f41c0a2aafa17c5efdde6851","Go","resolve_existing","go-language","technology"),
("fa40814fc9a3066621f4c3e198b6fb5d597fc55e80b26435fc7124da2e91ebd3","Go","resolve_existing","go-language","technology"),
("0a2a8b90529181fcc3a2abb47ca7da86fe80660f8b7c955f0625748a3277b73f","Python","resolve_existing","python-language","technology"),
("35a41e477a1c0779559569c18518f8b48e7ed4a1ef0e5f260194dd25f96ccd71","TOML","resolve_existing","toml-format","technology"),
("3b5bb1902ced84b1cd919b387b1e2a586783ba4bdc22f6d5032aafe6346d4d62","PlayerSync","resolve_existing","playersync","stable_component"),
("f3fe6dd4e9d2f695f70db0a52a2ada712b05e3a9024db78e9de27687580f325b","PlayerSync","resolve_existing","playersync","stable_component"),
("c29a90c27f141d9e71f27ff04a48097b9a699ba7cc166362641d4e323d6efcee","PlayerSync","resolve_existing","playersync","stable_component"),
("6dcd5e5cba15e9984ee46277da70da505d1b4af4d5ffd41bbbb44b76ca7e0469","Devtools","resolve_existing","devtools-subsystem","stable_component"),
("aad0537cdf2c4ccf60ae6f9a7f1c062fb8bdea97b0ebe23c53bf47e6de62c21d","devtools","resolve_existing","devtools-subsystem","stable_component"),
("efe2aabb2f3898334923a96ee9a8b0ebe7ae8b369e46c8bdfbab0aaa359c1e62","devtools","resolve_existing","devtools-subsystem","stable_component"),
("98cbb49987a3ca33fc6459e874b0fcd233160a390623829451b1e7059c57ca41","devtools","resolve_existing","devtools-subsystem","stable_component"),
("cb7845a776f3a8106f798b88be80ab58cb627e1e44c3a29c2f05b61228bf7551","RoomID","resolve_existing","room-id","code_symbol"),
("0e5c73d18580f6efe8936a2d739f15ac37b3e8a4a626712e61c6beb2e44d93fc","RoomID","resolve_existing","room-id","code_symbol"),
("54e3cba8738bb4d2dd04fdae0311793c5dd128bf6277703fdce855e2e3b2bf3c","DefaultRoomID","resolve_existing","default-room-id","code_symbol"),
("750e11764377e88d4574bdb09fcf4abec5d1101944ed7f342e4ca05a36c2255a","game server","resolve_existing","game-server","stable_component"),
("754455122410e9a4ddf599eb2cbad9d4ab2108f6e3c495eae5b588f143a4406d","game server","resolve_existing","game-server","stable_component"),
("c9a6e1323ef9991fa92215a1957dfa3ee4b430f42a59112165b39fa741bb826a","MCP server","resolve_existing","space-rocks-mcp-read","identity_collision"),
("b49a5935793aaae9776fe216158560769c7c64f919e5007df38ba386cc7f6e34","client/scripts","resolve_existing","client-scripts-dir","path"),
("b49a5935793aaae9776fe216158560769c7c64f919e5007df38ba386cc7f6e34","client/scenes","resolve_existing","client-scenes-dir","path"),
("f7eb826fffd36212c335e3b93071adf12a19c9512ab0c3369954c934baf1a58c","Effects","resolve_existing","effects-component","stable_component"),
("93f70d08a358c8174747516d95eb29645d93f7001e7853e94cd2bbef655a3a3f","OpenMenu","resolve_existing","open-menu-action","code_symbol"),
("237fc0b9807e482ae9fbe50be55908d8df6f6845c9c55a5c6e017f5f52a41553","OpenMenu","resolve_existing","open-menu-action","code_symbol"),
("b9ebb64e08a58fc598433afb6d7b639a765f16c6fdac882ecc7f967d2bf3605f","ShipState","resolve_existing","ship-state","code_symbol"),
("e863f00924510e2f1e95adaf6588e689d4d5f0d31261d33a2680ba039f4e8273","NetworkClient","resolve_existing","network-client","stable_component"),
("82a284bd19fd1472740eef6474ea651915e2b2561007d97465044b58901ee789","API server","resolve_existing","api-server","continuity"),
("630585c3531a782808b81b6234fcc5e8862520e147dec76e77d18cafd6752cba","GameplayShellFlow","resolve_existing","gameplay-shell-flow","stable_component"),
("0a2a8b90529181fcc3a2abb47ca7da86fe80660f8b7c955f0625748a3277b73f","data-sync","resolve_existing","data-sync-tool","tool"),
("718904666a9631b9e98ac6ab071c5bc05d8bddc442ec527b02d033b05d80b9c3","SpaceRocks repository","resolve_existing","space-rocks-repo","repository"),
("cc9fea94c2fe7d380bc92d021e95bf36ca5bacb0e9b1c39d732e56a3b2ad67ac","WorldSync","resolve_existing","world-sync","stable_component"),
("63953ddbf7a5c51049a629eab3244f321240185c69b0ae0c25ae58b7f1dbb457","Player","resolve_existing","player-node","real_collision"),
("c2af4155bd8e678640602eef44ba9ff301fa812ff7238d3ea3a8e9120ea92bd5","Game","resolve_existing","game-struct","real_collision"),
("aa3bd8795cbf19c8943bc3c36b50beb45bfbad99e127df9741f61824b361476d","Room","resolve_existing","room-struct","real_collision"),
("14975f19d77eee82bad4263e1a732c749778f68392fff43e498e405c96d6841c","Lobby","resolve_existing","lobby-state","real_collision"),
("73443b972e71fa584d778cbe92a8894146bd2fc5a76d3e86e82cc29d09c9d5fa","main.go","unresolved",None,"real_ambiguous"),
("4dab4804610a019a","MCP server","create_new",None,"real_create_new"),
("f66ea98358029d71504444cf48e39fbf7ee111f869c68222c3f5eb9082209da7","devtool","reject",None,"upstream_extraction_split"),
("11282fa93a6216f838435353c089287250dac8e87b488e1f89f7cb6a9e5c1bfe","Deployment","reject",None,"real_reject"),
]

def rid(x):
    if len(x)==64: return x
    hits=[k for k in MEM if k.startswith(x)]
    if len(hits)!=1: raise RuntimeError((x,len(hits)))
    return hits[0]

Q=[(rid(m),s,d,e,c) for m,s,d,e,c in Q]
source_query_ids={m for m,_,_,_,_ in Q}

def span(m,s):
    if m["memory_id"] in SHADOW:
        hits=[x for x in SHADOW[m["memory_id"]].get("actual_entity_mentions",[]) if x["text"].strip().lower()==s.lower()]
        if hits: return hits[0]
    for f in ("content","title"):
        z=re.search(re.escape(s),m[f],re.I)
        if z: return {"field":f,"start_byte":z.start(),"end_byte":z.end(),"text":m[f][z.start():z.end()]}
    raise RuntimeError((m["memory_id"],s))

def by_title(t):
    hits=[m for m in MEM.values() if m["owner_id"]==OWNER and t.lower() in m["title"].lower()]
    if not hits: raise RuntimeError(t)
    return hits[0]

def surface(s,n=2,words=()):
    p=re.compile(r"(?<![A-Za-z0-9_])"+re.escape(s)+r"(?![A-Za-z0-9_])",re.I); out=[]
    for m in MEM.values():
        if m["owner_id"]!=OWNER or m["memory_id"] in source_query_ids: continue
        h=m["title"]+"\n"+m["content"]
        if p.search(h) and (not words or any(w.lower() in h.lower() for w in words)): out.append(m)
    out.sort(key=lambda m:(m["title"].lower(),m["memory_id"]))
    if len(out)<n and words: return surface(s,n,())
    if len(out)<n: raise RuntimeError(("evidence",s,n,len(out)))
    return out[:n]

def ent(i,name,aliases,kind,summary,ev):
    if not ev or any(m["owner_id"]!=OWNER for m in ev): raise RuntimeError(("non-REL entity evidence",i,[m["memory_id"] for m in ev]))
    return {"entity_id":i,"owner_id":OWNER,"canonical_name":name,"aliases":aliases,"kind":kind,"summary":summary,
            "evidence":[{"memory_id":m["memory_id"],"text":m["content"]} for m in ev[:3]]}

E=[
ent("go-language","Go",["Go"],"programming_language","The Go programming language used by the Space Rocks server and tooling.",surface("Go")),
ent("python-language","Python",["Python"],"programming_language","The Python programming language used by project tooling.",surface("Python")),
ent("toml-format","TOML",["TOML"],"data_format","TOML used for source-of-truth configuration and generated constants.",surface("TOML")),
ent("playersync","PlayerSync",["PlayerSync"],"code_component","The quarantined legacy PlayerSync client component behind an API wall.",surface("PlayerSync",3)),
ent("devtools-subsystem","Devtools",["Devtools"],"subsystem","The Space Rocks devtools subsystem: telemetry, debug controls, and wire protocol.",surface("devtools",3)),
ent("room-id","RoomID",["RoomID"],"code_symbol","The server's internal RoomID identifier, distinct from player-facing targeting.",surface("RoomID",2)),
ent("default-room-id","DefaultRoomID",["DefaultRoomID"],"code_symbol","The DefaultRoomID constant used by default-room access and normalization.",surface("DefaultRoomID",1)),
ent("game-server","Space Rocks game server",["game server","game-server"],"service","The Go Space Rocks game server and its runtime/test surface.",surface("game server",3)),
ent("space-rocks-mcp-read","Space Rocks MCP server",["MCP server"],"tool_service","The existing read-only Space Rocks MCP server used for planning and diagnosis.",
    [by_title("Space Rocks MCP server tools"),by_title("Read-only Space Rocks MCP server"),by_title("Planning and diagnostic MCP server")]),
ent("client-scripts-dir","client/scripts",["client/scripts"],"directory","The Space Rocks client/scripts source directory.",surface("client/scripts")),
ent("client-scenes-dir","client/scenes",["client/scenes"],"directory","The Space Rocks client/scenes scene directory.",surface("client/scenes")),
ent("effects-component","Effects",["Effects"],"code_component","The gameplay Effects component.",surface("Effects")),
ent("open-menu-action","OpenMenu",["OpenMenu"],"input_action","The OpenMenu gameplay input action used for pause/menu routing.",surface("OpenMenu",3)),
ent("ship-state","ShipState",["ShipState"],"code_type","The ShipState gameplay packet/state struct, distinct from Ship.",surface("ShipState",3)),
ent("network-client","NetworkClient",["NetworkClient"],"code_component","The client NetworkClient transport component.",surface("NetworkClient",2)),
ent("api-server","Space Rocks API server",["API server"],"service","The Space Rocks web/auth API server; implementation details changed while the service identity persisted.",
    [by_title("Rails API server"),by_title("API server will serve the entire website"),by_title("API environment loaded by direnv")]),
ent("gameplay-shell-flow","GameplayShellFlow",["GameplayShellFlow"],"code_component","The client gameplay composition-root flow.",surface("GameplayShellFlow",3)),
ent("data-sync-tool","data-sync",["data-sync"],"tool","The Space Rocks data-sync synchronization/generation tool.",surface("data-sync",3)),
ent("space-rocks-repo","SpaceRocks repository",["SpaceRocks repository","@SpaceRocks repository"],"repository","The Space Rocks source repository.",
    [MEM[rid("a715c9721dcda289")],MEM[rid("721eb879424f91f7")]]),
ent("world-sync","WorldSync",["WorldSync"],"code_component","The client WorldSync synchronization component.",surface("WorldSync",3)),
ent("player-node","Player",["Player"],"code_component","The client Player node/component used for avatar/camera presentation.",[MEM[rid("07207f8c539c82c8")]]),
ent("player-participant","Player",["Player"],"domain_entity","A gameplay player/participant with durable session state.",[MEM[rid("2c792af65311b951")]]),
ent("game-struct","Game",["Game"],"code_type","The server Game type that owns gameplay orchestration such as spawn-position selection.",surface("Game",2,("Game retains","game."))),
ent("space-rocks-game","Game",["Game"],"application","The Space Rocks game/application as a whole.",[MEM[rid("fbc8c3566e7a7c60")]]),
ent("room-struct","Room",["Room"],"code_type","The server Room code type being split/refactored.",surface("Room",2,("split","struct"))),
ent("runtime-room","Room",["Room"],"runtime_entity","A runtime multiplayer room with players and room state.",surface("Room",2,("players","state"))),
ent("lobby-state","Lobby",["Lobby"],"enum_value","The Lobby room-state wire value paired with InGame.",surface("Lobby",2,("wire","state"))),
ent("multiplayer-lobby","Lobby",["Lobby"],"ui_component","The multiplayer lobby UI/flow before gameplay.",surface("multiplayer lobby",2)),
ent("game-server-main-go","main.go",["main.go"],"file","The game-server main.go at services/game-server/cmd/game-server.",[by_title("Game server entry point")]),
ent("local-server-main-go","main.go",["main.go"],"file","The planned local-server main.go entry point.",[by_title("Separate Server Entrypoints")]),
ent("online-server-main-go","main.go",["main.go"],"file","The planned online-server main.go entry point.",[by_title("Separate Server Entrypoints")]),
]

queries=[]; gold={}
for i,(mid,s,d,target,cat) in enumerate(Q,1):
    m=MEM[mid]; qid=f"frozen-{i:03d}"
    queries.append({"memory_id":qid,"source_memory_id":mid,"owner_id":m["owner_id"],"title":m["title"],"content":m["content"],"mentions":[span(m,s)],"category":cat})
    gold[qid]={"expected_decision":d,"expected_entity_id":target,"category":cat,"source_memory_id":mid}

ids={e["entity_id"] for e in E}; byid={e["entity_id"]:e for e in E}
assert len(queries)==len(gold)==41
for q in queries:
    x=q["mentions"][0]; assert q[x["field"]][x["start_byte"]:x["end_byte"]]==x["text"]
    g=gold[q["memory_id"]]
    if g["expected_decision"]=="resolve_existing":
        assert g["expected_entity_id"] in ids
        evidence_ids = {x["memory_id"] for x in byid[g["expected_entity_id"]]["evidence"]}
        if q["source_memory_id"] in evidence_ids:
            raise RuntimeError(("query leaked into target evidence", q["memory_id"], q["source_memory_id"], g["expected_entity_id"]))

OUT.mkdir(parents=True,exist_ok=True)
(OUT/"entities.jsonl").write_text("\n".join(json.dumps(x,ensure_ascii=False) for x in E)+"\n",encoding="utf-8")
(OUT/"queries.jsonl").write_text("\n".join(json.dumps(x,ensure_ascii=False) for x in queries)+"\n",encoding="utf-8")
(OUT/"gold.json").write_text(json.dumps(gold,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
regression_mid=rid("f66ea98358029d71504444cf48e39fbf7ee111f869c68222c3f5eb9082209da7")
regression_memory=MEM[regression_mid]
expected_text="Respawn devtool"
expected_start=regression_memory["title"].index(expected_text)
observed=SHADOW[regression_mid].get("actual_entity_mentions",[])
(OUT/"upstream_extraction_regressions.jsonl").write_text(json.dumps({
    "memory_id": regression_mid,
    "issue": "split_identity_surface",
    "expected_mention": {"field":"title","start_byte":expected_start,"end_byte":expected_start+len(expected_text),"text":expected_text},
    "observed_mentions": observed,
    "note": "Resolution receives bare devtool and must reject it; Pass 1 should extract the complete identifying surface Respawn devtool."
},ensure_ascii=False)+"\n",encoding="utf-8")

(OUT/"README.md").write_text("""# Frozen REL Entity-resolution gold v1

Hand-labelled real-data corpus built from the frozen 28-day Reliquary fixture.
Source REL: fixtures/local/frozen/chatgpt-first28d-2026-08-30/project.prj.rel
Source harvest: calibration/insomnia-entity-v2/candidates.jsonl (3,060 Memories).

Each benchmark row preserves the original Memory ID in source_memory_id. Most mention spans come directly from the Sol-low shadow Entity-extraction run; the two close-new cases are manually selected real Memories from the same REL.

The corpus contains repeated durable identities, real same-surface collisions (Player, Game, Room, Lobby), a three-way main.go ambiguity, a second MCP server that should create a new Entity, and non-durable extracted mentions that Pass 2 should reject.

The Respawn devtool Memory exposed an upstream extraction regression: Pass 1 split the identifying title surface Respawn devtool into separate Respawn and devtool mentions. The resolution benchmark therefore labels bare devtool as reject, while upstream_extraction_regressions.jsonl records the complete span Pass 1 should have emitted.

Resolution gold was curated from frozen REL evidence; no model-generated resolution labels are used.
""",encoding="utf-8")
from collections import Counter
print(json.dumps({"queries":len(queries),"entities":len(E),"categories":Counter(g["category"] for g in gold.values())},default=dict,indent=2))
