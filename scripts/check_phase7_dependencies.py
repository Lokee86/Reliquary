#!/usr/bin/env python3
"""Build/check the bounded Phase 7 import disposition register.

The register preserves every unapproved source-import row. It recognizes only
exact existing owner value contracts as approved; all other entries remain
unapproved, classified conservatively as capability evidence, value-shaped
but unproven, or unresolved. This is not a compiler-resolved call graph.
"""
from __future__ import annotations
import argparse, hashlib, json, re
from pathlib import Path
import subprocess
import sys
from functools import lru_cache
ROOT = Path(__file__).resolve().parents[1]
BASELINE = ROOT / "benchmarks/phase7-baseline.json"
POLICY = ROOT / "tools/pitlord/owner-boundaries.json"
OUTPUT = ROOT / "benchmarks/phase7-edge-dispositions.json"
sys.path.insert(0, str(ROOT / "scripts"))
from check_owner_boundaries import module_tree, split_use_tree, strip_inactive_cfg  # noqa: E402
from check_multiplexing_phase4_coverage import masked  # noqa: E402
EXTERNAL_PACKED_ROOT = ROOT.parent / "lodestone"
EXTERNAL_PACKED_REV = "7b4b3e02b2bd3f3e474fef1a31c8632f8bbea49f"

LOCAL_VALUE_CONTRACTS = {
    ("dream", "retrieval", "MemoryRetrievalIndex"): {
        "kind": "read-only-index-value",
        "reason": "Dream receives only &MemoryRetrievalIndex and reads its selected IDs/vectors. Retrieval remains the builder; the imported value has no store/session capability and no mutation is exposed at this call boundary.",
        "consumer_source": "src/runtime_inference/dream_community_naming_context.rs",
        "consumer_lines": [31, 98],
        "owner_source": "src/retrieval/memory_retrieval_model.rs",
        "owner_lines": [41, 54],
    },
    ("dream", "retrieval", "MemoryRetrievalError"): {
        "kind": "error-value",
        "reason": "Dream wraps the retrieval failure as an enum variant; the import carries error data only and exposes no retrieval operation.",
        "consumer_source": "src/runtime_inference/dream_community_naming_error.rs",
        "consumer_lines": [1, 9],
        "owner_source": "src/retrieval/memory_retrieval_error.rs",
        "owner_lines": [5, 16],
    },
    ("external-sources", "vector-store", "CompatibilityProfile"): {
        "kind": "read-only-profile-value",
        "reason": "External indexing reads profile identity, dimensions, normalization, and references; it receives/stores the profile value and invokes no vector-store operation through it.",
        "consumer_source": "src/external_source/index.rs",
        "consumer_lines": [5, 12, 17, 38],
        "owner_source": "src/retrieval/compatibility_profile_model.rs",
        "owner_lines": [18, 25],
    },
    ("external-sources", "switchboard", "EmbeddingMode"): {
        "kind": "immutable-protocol-value",
        "reason": "Copy enum used as the mode argument to the embedding provider protocol; it has no state or callback behavior.",
        "consumer_source": "src/external_source/index.rs",
        "consumer_lines": [5, 269, 353],
        "owner_source": "src/runtime_inference/embedding_endpoint.rs",
        "owner_lines": [6, 9],
    },
}

EXTERNAL_PACKED_TYPES = {
    "PackedVectors": "Owned packed byte matrix; constructors validate layout and accessors expose read-only rows/bytes or consume the owned bytes. No storage, provider, or shared-state capability.",
    "ScalarType": "Copy enum defining the scalar tag and width vocabulary. No provider or store capability.",
    "VectorSchema": "Copy value containing dimensions and ScalarType; constructors and row-width calculation are pure validation/value operations.",
}

REGISTERED_CHILDREN = [{
    "id": "P7.1.11-EXTERNAL-PROVIDER",
    "parent_ticket": "P7.1.11",
    "scope": ["src/external_source/index.rs", "src/external_source/processing.rs", "src/runtime_inference/runtime_host_gateway_external.rs", "src/runtime_inference/runtime_vector_step.rs"],
    "witnesses": [
        {"path": "src/external_source/index.rs", "lines": [5, 17, 269, 353], "symbol": "EmbeddingEndpoint", "capability": "provider method calls embed, dimensions, normalization; moved to the Vectors-owned ExternalEmbeddingService"},
        {"path": "src/external_source/processing.rs", "lines": [5, 82, 124], "symbol": "GeneralEndpoint", "capability": "generic type bound and Insomnia/temporal inference orchestration; moved to Runtime Gateway"},
    ],
    "preserving_contracts": ["public GatewayRuntime method signatures", "authorization/read/install order", "external batch and vector bounds", "existing ExternalSourceError mapping", "prepared source state and publication receipts"],
    "status": "implemented-source-cut-awaiting-verification",
}, {
    "id": "P7.1.09a-IMPORT-INSOMNIA-REPLAY",
    "parent_ticket": "P7.1.09",
    "scope": ["src/imports/reconciliation_interval.rs", "src/owner_lifecycle/insomnia_import_replay.rs"],
    "witnesses": [
        {"path": "src/imports/reconciliation_interval.rs", "lines": [481, 497], "symbol": "InsomniaStore::apply_work / InsomniaStore::apply_attempt", "capability": "Import decoded records and appended physical payload, then directly mutated the Insomnia owner projection; moved the complete ordered replay operation into owner lifecycle"}
    ],
    "preserving_contracts": ["physical append precedes projection update", "Work and Attempt accepted order", "exact payload bytes", "existing retry, sync, reopen and error mapping"],
    "preserving_tests": ["valid_insomnia_journal_history_is_replayed_with_completed_import", "imports sequential completion and reconciliation suites"],
    "status": "implemented-source-cut-awaiting-verification",
}]

# Seam status is deliberately evidence-based and conditional. Only rows with
# a matching repository guard and no relevant current source import are no-op.
SEAMS = [
 ("P7.1.01", "Container to semantic backedges", ["container"], ["scripts/check_physical_boundary.py"], "verified-no-op", "Container sources are checked for nonphysical crate imports, callbacks and handle escape; no Container-to-semantic unapproved pair is present in the pinned baseline."),
 ("P7.1.02", "Owner-store to orchestrator backedges", ["archive-store","memory-store","graph-store","perception-store","freshness-store","dream-store","insomnia-store","vector-store","ego-store","import-store","maintenance-store","external-index-store"], ["scripts/check_owner_boundaries.py"], "open", "No source-store-origin unapproved pair appears in this import inventory, but the owner checker is not an exhaustive symbol-call proof; retain conditional until B7 call evidence is bounded."),
 ("P7.1.03", "Archive and Memory implementation cycle", ["archive-store","memory-store"], ["scripts/check_owner_boundaries.py"], "verified-no-op", "No unapproved direct source-import pair exists in either direction; exact accepted identity contracts are handled by the owner boundary policy."),
 ("P7.1.04", "Graph and Perception implementation cycle", ["graph-store","perception-store"], ["scripts/check_owner_boundaries.py"], "verified-no-op", "No unapproved direct source-import pair exists in either direction; owner policy carries exact immutable cross-owner contracts."),
 ("P7.1.05", "Freshness and Dream owner cycle", ["freshness-store","dream-store","freshness","dream"], ["scripts/check_owner_boundaries.py"], "verified-no-op", "No unapproved direct source-import pair exists between these components in the pinned inventory."),
 ("P7.1.06", "Insomnia and vectors or owner lifecycle", ["insomnia","insomnia-store","vector-store","owner-lifecycle"], ["scripts/check_owner_boundaries.py"], "open", "Insomnia imports Container in evidence preparation and has owner lifecycle references; assess the concrete evidence path before declaring a no-op."),
 ("P7.1.07", "Ego or remaining semantic-owner backedges", ["ego-store","dream","retrieval","vector-store","container"], ["scripts/check_owner_boundaries.py"], "open", "Dream imports Container, Retrieval result types and VectorStore capabilities; named source cuts require ownership/call trace before implementation."),
 ("P7.1.08", "History to Import service callback", ["history","import","import-store"], ["scripts/check_history_boundaries.py"], "verified-no-op", "No unapproved direct source-import pair links History to Import in either direction; History has an explicit read-only capability guard."),
 ("P7.1.09", "Import to Maintenance and reconciliation callback", ["import","import-store","maintenance","maintenance-store"], ["scripts/check_owner_boundaries.py"], "open", "P7.1.09a moves Insomnia replay application behind owner lifecycle; compiler-call and source evidence still show other Import-to-owner-store replay operations that require their own bounded disposition."),
 ("P7.1.10", "Project Environment to runtime or facade", ["project-environment","runtime-gateway","public-consumers"], ["scripts/check_facade_boundaries.py"], "verified-no-op", "No unapproved direct source-import pair originates in Project Environment toward runtime or public-consumers."),
 ("P7.1.11", "External Source to Gateway or domain", ["external-sources","runtime-gateway","switchboard","vector-store"], [], "open", "External Sources imports endpoint and vector profile contracts; capability versus immutable value use remains to be proven."),
 ("P7.1.12", "Runtime and domain execution backedges", ["runtime-gateway","public-consumers","graph-store","perception-store","vector-store","insomnia-store","external-index-store"], ["scripts/check_facade_boundaries.py"], "open", "Runtime Gateway has direct imports from domain stores and five public aggregate imports; classify each import/use and separate concrete capability edges from accepted values."),
 ("P7.1.13", "Provider or configured-runtime backedges", ["switchboard","config-security","runtime-gateway"], ["scripts/check_runtime_boundaries.py"], "open", "Provider/runtime direction requires call/use review; the baseline includes source dependencies but cannot close this category alone."),
 ("P7.1.14", "Public facade to lower callback cycles", ["public-consumers","archive-store","chronos","config-security","container","context-engine","dream","dream-store","ego-store","external-index-store","freshness","freshness-store","graph-store","import-store","insomnia","insomnia-store","maintenance-store","memory-store","owner-lifecycle","perception","perception-store","pure-codec","retrieval","review-clocks","switchboard","vector-store","vectors"], ["scripts/check_facade_boundaries.py"], "open", "Direct public-consumers imports span 26 lower components; distinguish contract types from executable capability and preserve only exact immutable values."),
 ("P7.1.15", "Context Engine and Gateway backedges", ["context-engine","runtime-gateway"], ["scripts/check_facade_boundaries.py"], "verified-no-op", "No unapproved direct source-import pair connects Context Engine and Gateway in either direction."),
 ("P7.1.16", "Obsolete internal adapters after cuts", [], [], "open", "Import edges cannot prove private forwarding/adapters are absent; exact consumer/symbol inventory remains required after cuts."),
]


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

@lru_cache(maxsize=None)
def snapshot_text(path: str, root: Path = ROOT, commit: str | None = None) -> str:
    if commit:
        result=subprocess.run(["git","show",f"{commit}:{path}"],cwd=root,capture_output=True,text=True)
        if result.returncode==0:
            return result.stdout
    file=root/path
    return file.read_text(encoding="utf-8") if file.is_file() else ""

def snapshot_sha(path: str, root: Path = ROOT, commit: str | None = None) -> str | None:
    text=snapshot_text(path,root,commit)
    return hashlib.sha256(text.encode("utf-8")).hexdigest() if text else None

def source_line(path: str, line: int, root: Path = ROOT, commit: str | None = None) -> str:
    rows=snapshot_text(path,root,commit).splitlines()
    return rows[line-1].strip() if 0 < line <= len(rows) else ""

def is_lib_pub_use(edge: dict, root: Path = ROOT, commit: str | None = None) -> bool:
    return edge["source"]["path"] == "src/lib.rs" and re.match(r"pub\s+use\b", source_line(edge["source"]["path"], edge["source"]["line"], root, commit)) is not None

def exact_local_contract(edge: dict) -> dict | None:
    return LOCAL_VALUE_CONTRACTS.get((edge["source_component"], edge["target_component"], edge["target"]["name"]))

def classify(edge: dict, contracts: set[tuple[str,str,str]], root: Path = ROOT, commit: str | None = None) -> tuple[str,str]:
    key=(edge["source_component"],edge["target_component"],edge["target"]["name"])
    if is_lib_pub_use(edge, root, commit):
        return "supported-public-api-binding", "Exact src/lib.rs pub-use line exposes a supported public binding; no implementation body/call exists at this edge endpoint."
    if key in contracts:
        return "exact-registered-immutable-value", "Existing owner-boundaries contract matches exact consumer, owner, symbol."
    local=exact_local_contract(edge)
    if local:
        return local["kind"], local["reason"]
    name=edge["target"]["name"]
    target=edge.get("target",{}).get("path")
    source_text=snapshot_text(edge["source"]["path"],root,commit)
    target_text=snapshot_text(target,root,commit) if target else ""
    # Imported function calls and trait bounds/methods are capability evidence,
    # even when the identifier itself is not invoked as Name(...).
    if re.search(r"\b(?:pub\s+)?trait\s+"+re.escape(name)+r"\b", target_text):
        if re.search(r"\b(?:impl|dyn)\s+"+re.escape(name)+r"\b|:\s*"+re.escape(name)+r"\b", source_text):
            return "protocol-capability-evidence", "Imported trait is used as a bound/object and represents callable behavior; not an immutable value."
    if re.search(r"\b"+re.escape(name)+r"\s*\(", source_text):
        return "callable-capability-evidence", "Imported symbol is invoked in its source module; still unapproved unless target DAG permits the edge."
    if re.search(r"\b(?:[A-Za-z0-9_]*Store|[A-Za-z0-9_]*Runtime|[A-Za-z0-9_]*Processor|[A-Za-z0-9_]*Engine|Container|Cva|Phylactery)\b", name):
        return "owner-capability-evidence", "Imported symbol names an owner/runtime/product capability; still unapproved unless target DAG permits the edge."
    if name in {"*", "self"}:
        return "unresolved-import-surface", "Wildcard or module-self import requires its exact module surface disposition."
    return "value-shaped-unproven", "Type/value-shaped import has no exact immutable-value contract; retained as unapproved."

def surface_symbols(path: str, root: Path = ROOT, commit: str | None = None) -> list[str]:
    """Return a hashed, bounded lexical list of declarations/imports in a wildcard target."""
    text=snapshot_text(path,root,commit)
    found=set(re.findall(r"\bpub(?:\([^)]*\))?\s+(?:struct|enum|trait|type|const|static|fn|mod)\s+([A-Za-z_]\w*)", text))
    # Include local imports and re-exports, including multiline brace trees.
    use_start=re.compile(r"(?m)\b(?:pub(?:\([^)]*\))?\s+)?use\s+")
    for match in use_start.finditer(text):
        semi=text.find(";",match.end())
        if semi<0:
            continue
        tree=text[match.end():semi].strip()
        try:
            leaves=split_use_tree(tree)
        except (ValueError, IndexError):
            continue
        for path, alias in leaves:
            leaf=alias or path.rstrip(":*").split("::")[-1]
            if leaf and leaf!="*":
                found.add(leaf)
    # Private parent names are visible to child modules using super::*.
    found.update(re.findall(r"(?m)^\s*(?:(?:pub(?:\([^)]*\))?|crate)\s+)?(?:struct|enum|trait|type|const|static|fn|mod)\s+([A-Za-z_]\w*)", text))
    return sorted(found)

def uncertainty_disposition(row: dict, file_components: dict[str,str], allowed: dict[str,set[str]], root: Path, commit: str) -> dict:
    source_path=row["source_path"]
    source_component=file_components.get(source_path)
    target_path=row.get("target_path")
    target_component=file_components.get(target_path) if target_path else None
    base={"source":source_path,"line":row["line"],"import":row["import"],"canonical":row["canonical"],"target_symbol":row["target_symbol"],"source_component":source_component,"target_path":target_path,"target_component":target_component,"source_file_sha256":snapshot_sha(source_path,root,commit),"target_file_sha256":snapshot_sha(target_path,root,commit) if target_path else None}
    symbol=row["target_symbol"]
    if symbol in EXTERNAL_PACKED_TYPES and row["canonical"].startswith("lodestone_packed::"):
        return {**base,"classification":"exact-pinned-external-protocol-value","disposition":"value-contract","contract":EXTERNAL_PACKED_TYPES[symbol],"public_reexport":{"path":"src/lib.rs","line":1358,"sha256":snapshot_sha("src/lib.rs",root,commit)},"dependency_pin":{"manifest":"Cargo.toml","manifest_sha256":sha(root/"Cargo.toml"),"lock":"Cargo.lock","lock_sha256":sha(root/"Cargo.lock"),"revision":EXTERNAL_PACKED_REV},"external_source_sha256":sha(EXTERNAL_PACKED_ROOT/"crates/packed/src/lib.rs") if (EXTERNAL_PACKED_ROOT/"crates/packed/src/lib.rs").is_file() else None,"external_definition_line":{"PackedVectors":75,"ScalarType":5,"VectorSchema":54}[symbol]}
    if row.get("reason")=="wildcard import" and target_path:
        if source_component==target_component:
            kind="same-component-module-wildcard"
            disposition="intra-component"
        elif target_component in allowed.get(source_component,set()):
            kind="target-dag-permitted-module-surface"
            disposition="permitted-component-dependency"
        else:
            kind="cross-component-wildcard-needs-disposition"
            disposition="material-unknown"
        return {**base,"classification":kind,"disposition":disposition,"target_module_surface":surface_symbols(target_path,root,commit),"surface_note":"Lexical declarations/import names are bounded by exact target source hash; macro-generated exports and alternate cfg profiles are outside this receipt."}
    if target_path:
        if source_component==target_component:
            return {**base,"classification":"same-component-unresolved-symbol","disposition":"intra-component","surface_note":"Resolved to a defining module in the same mapped component; no cross-component edge is introduced."}
        if target_component in allowed.get(source_component,set()):
            return {**base,"classification":"permitted-target-component-unresolved-symbol","disposition":"permitted-component-dependency","surface_note":"Resolved target component is an explicit target-DAG dependency; exact symbol was retained as unresolved rather than treated as value."}
    return {**base,"classification":"unresolved-external-or-reexport","disposition":"material-unknown"}

def validate_external_value_contracts(entries: dict[str,str], uncertainties: list[dict]) -> list[str]:
    used={row["target_symbol"] for row in uncertainties if row.get("classification")=="exact-pinned-external-protocol-value"}
    return [f"stale-external-contract:{name}" for name in sorted(set(entries)-used)]

def validate_exception_entries(entries: dict[tuple[str,str,str],dict], edges: list[dict], uncertainties: list[dict]) -> list[str]:
    seen={(e["source_component"],e["target_component"],e["target"]["name"]) for e in edges}
    # Exact local contracts used by unresolved imports are separately grounded
    # by their source locations in the uncertainty registry.
    for row in uncertainties:
        if row.get("contract_key"):
            seen.add(tuple(row["contract_key"]))
    return [f"stale-exception:{key[0]}->{key[1]}:{key[2]}" for key in sorted(set(entries)-seen)]


def external_provider_violations(source_files: dict[str,str]) -> list[str]:
    forbidden=re.compile(r"\b(?:EmbeddingEndpoint|EmbeddingMode|GeneralEndpoint|GeneralEndpointError|InsomniaExtractor|TemporalInferencer)\b")
    return [f"{path}: residual provider/inference capability `{match.group(0)}`" for path,text in sorted(source_files.items()) for match in forbidden.finditer(masked(text))]

def current_external_provider_proof(root: Path = ROOT) -> dict:
    mapping=json.loads((root/"tools/pitlord/production-components.json").read_text(encoding="utf-8"))
    profile=mapping["cfg_profile"]
    facts=set(profile["rustc_cfg_facts"]) | {f'feature="{name}"' for name in profile.get("features",[])}
    module_paths={path.replace("\\","/") for path in module_tree(root,facts).values() if path.startswith("src/external_source/")}
    source_files={path:strip_inactive_cfg((root/path).read_text(encoding="utf-8"),facts) for path in sorted(module_paths)}
    violations=external_provider_violations(source_files)
    vector_path="src/runtime_inference/runtime_vector_step.rs"
    processing_path="src/runtime_inference/runtime_host_gateway_external_processing.rs"
    vector_source=(root/vector_path).read_text(encoding="utf-8")
    processing_source=(root/processing_path).read_text(encoding="utf-8")
    if "trait ExternalEmbeddingService" not in vector_source or "impl<E: EmbeddingEndpoint> ExternalEmbeddingService for E" not in vector_source:
        violations.append(f"{vector_path}: Vectors-owned ExternalEmbeddingService implementation missing")
    if "InsomniaExtractor" not in processing_source or "TemporalInferencer" not in processing_source:
        violations.append(f"{processing_path}: upper runtime processing orchestration missing")
    if violations:
        raise ValueError("external provider hard cut failed: "+"; ".join(violations))
    evidence_paths=sorted(module_paths | {vector_path,processing_path,"src/runtime_inference/runtime_host_gateway_external.rs"})
    return {
        "status":"pass",
        "scope":"cfg-reachable external_source production modules plus the Vectors provider adapter and upper Gateway processing adapter",
        "profile_source":"tools/pitlord/production-components.json",
        "profile_facts_count":len(facts),
        "external_source_production_module_count":len(module_paths),
        "forbidden_provider_or_inference_identifiers":[],
        "vectors_provider_contract":{"path":vector_path,"trait":"ExternalEmbeddingService","implementation":"impl<E: EmbeddingEndpoint> ExternalEmbeddingService for E","sha256":sha(root/vector_path)},
        "runtime_processing_contract":{"path":processing_path,"entrypoint":"prepare","insomnia_inference":"InsomniaExtractor + TemporalInferencer","sha256":sha(root/processing_path)},
        "source_hashes_sha256":{path:sha(root/path) for path in evidence_paths},
        "proof_limit":"Bounded source and recorded-cfg check; no Rust compilation or macro-expanded call graph is asserted.",
    }

def build() -> dict:
    baseline=json.loads(BASELINE.read_text(encoding="utf-8")); policy=json.loads(POLICY.read_text(encoding="utf-8"))
    contracts={(r["consumer"],r["owner"],r["symbol"]) for r in policy.get("producer_value_contracts",[])}
    source_commit=baseline["inputs"]["git_head"]
    file_components={r["path"]:r["component"] for r in json.loads((ROOT/"tools/pitlord/production-components.json").read_text(encoding="utf-8"))["files"]}
    dag=json.loads((ROOT/"tools/pitlord/convergence-component-dag.json").read_text(encoding="utf-8"))
    allowed={r["id"]:{e["to"] for e in r.get("depends_on",[])} for r in dag["components"]}
    all_edges=baseline["source_imports"]
    edges=[e for e in all_edges if e["disposition"]=="UNAPPROVED-PAIR"]
    rows=[]
    for i,e in enumerate(edges,1):
        kind,why=classify(e,contracts,ROOT,source_commit)
        if kind=="supported-public-api-binding": disposition="supported-api-binding"
        elif exact_local_contract(e): disposition="exact-local-value-contract"
        elif kind=="protocol-capability-evidence": disposition="named-capability-cut"
        else: disposition="unapproved"
        rows.append({"row_id":f"B7-{i:04d}","source_component":e["source_component"],"target_component":e["target_component"],"source":e["source"],"target":e["target"],"canonical":e["canonical"],"source_file_sha256":snapshot_sha(e["source"]["path"],ROOT,source_commit),"target_file_sha256":snapshot_sha(e["target"]["path"],ROOT,source_commit) if e["target"].get("path") else None,"classification":kind,"disposition":disposition, "basis":why})
    uncertainties=baseline["uncertainties"]["rows"]
    uncertain=[{"row_id":f"U7-{i:03d}",**uncertainty_disposition(r,file_components,allowed,ROOT,source_commit)} for i,r in enumerate(uncertainties,1)]
    stale_exceptions=validate_exception_entries(LOCAL_VALUE_CONTRACTS,edges,uncertain)
    stale_external=validate_external_value_contracts(EXTERNAL_PACKED_TYPES,uncertain)
    if stale_exceptions or stale_external:
        raise ValueError("unused exact contract(s): "+", ".join(stale_exceptions+stale_external))
    external_provider_proof=current_external_provider_proof(ROOT)
    contract_records=[]
    for key,contract in sorted(LOCAL_VALUE_CONTRACTS.items()):
        contract_records.append({"consumer":key[0],"owner":key[1],"symbol":key[2],**contract,"evidence_source_checkpoint":source_commit,"consumer_source_sha256":snapshot_sha(contract["consumer_source"],ROOT,source_commit),"owner_source_sha256":snapshot_sha(contract["owner_source"],ROOT,source_commit)})
    seam_rows=[]
    pair_counts={}
    for edge in edges:
        key=(edge["source_component"],edge["target_component"])
        pair_counts[key]=pair_counts.get(key,0)+1
    seam_pair_scopes={
        "P7.1.01": lambda a,b: a=="container",
        "P7.1.02": lambda a,b: a.endswith("-store") and b in {"runtime-gateway","public-consumers","dream","retrieval","freshness","insomnia","external-sources"},
        "P7.1.03": lambda a,b: {a,b}=={"archive-store","memory-store"},
        "P7.1.04": lambda a,b: {a,b}=={"graph-store","perception-store"},
        "P7.1.05": lambda a,b: {a,b} in ({"freshness-store","dream-store"},{"freshness","dream"},{"freshness-store","dream"},{"dream-store","freshness"}),
        "P7.1.06": lambda a,b: (a.startswith("insomnia") and b in {"vector-store","owner-lifecycle","container"}) or (b.startswith("insomnia") and a in {"vector-store","owner-lifecycle","container"}),
        "P7.1.11": lambda a,b: a=="external-sources" and b in {"runtime-gateway","switchboard","vector-store"},
        "P7.1.12": lambda a,b: a=="runtime-gateway" and b in {"public-consumers","container","external-index-store","graph-store","insomnia-store","perception-store","vector-store"},
        "P7.1.14": lambda a,b: a=="public-consumers" and b!="runtime-gateway",
        "P7.1.08": lambda a,b: {a,b}=={"history","import"} or {a,b}=={"history","import-store"},
        "P7.1.09": lambda a,b: {a,b} in ({"import","maintenance"},{"import-store","maintenance-store"},{"import","maintenance-store"},{"import-store","maintenance"}),
        "P7.1.10": lambda a,b: a=="project-environment" and b in {"runtime-gateway","public-consumers"},
        "P7.1.15": lambda a,b: {a,b}=={"context-engine","runtime-gateway"},
    }
    for ticket,title,scope,guards,status,basis in SEAMS:
        predicate=seam_pair_scopes.get(ticket, lambda a,b: a in scope or b in scope)
        related=sum(n for (a,b),n in pair_counts.items() if predicate(a,b))
        seam_rows.append({"ticket":ticket,"title":title,"status":status,"component_scope":scope,"related_unapproved_import_rows":related,"source_guards":guards,"basis":basis})
    return {"schema":"phase7-edge-dispositions.v1","as_of":"2026-10-24","status":"B7 bounded register; residuals remain open","source_baseline_sha256":sha(BASELINE),"source_checkpoint":source_commit,"owner_policy_sha256":sha(POLICY),"classification_source_sha256":{path:snapshot_sha(path,ROOT,source_commit) for path in sorted({e["source"]["path"] for e in edges})},"counts":{"unapproved_candidate_rows":len(rows),"pairs":len(baseline["unapproved_pairs"]),"public_api_binding_rows":sum(r["classification"]=="supported-public-api-binding" for r in rows),"exact_local_value_contract_rows":sum(r["disposition"]=="exact-local-value-contract" for r in rows),"named_provider_capability_rows":sum(r["disposition"]=="named-capability-cut" for r in rows),"capability_evidence_rows":sum("capability-evidence" in r["classification"] for r in rows),"value_shaped_unproven_rows":sum(r["classification"]=="value-shaped-unproven" for r in rows),"unresolved_import_surface_rows":sum(r["classification"]=="unresolved-import-surface" for r in rows),"uncertainties":len(uncertain),"material_uncertainties":sum(r["disposition"]=="material-unknown" for r in uncertain)},"classification_policy":"src/lib.rs pub-use lines are supported API bindings and are not implementation capabilities. Local immutable contracts are exact consumer/owner/symbol entries with source proof. Protocol traits remain named capabilities. Name heuristics never grant exceptions. All other rows remain unapproved.","local_value_contracts":contract_records,"registered_children":REGISTERED_CHILDREN,"external_provider_cut":external_provider_proof,"seam_tickets":seam_rows,"candidate_edges":rows,"uncertainties":uncertain,"proof_limits":["Source import inventory is bounded to the baseline's recorded Windows x86_64 MSVC cfg profile.","The 97 uncertainty rows are each source-hashed and dispositioned by exact owner/component or pinned external contract; macro-generated exports and alternate cfg profiles remain outside this receipt.","No compiler-resolved call graph is asserted; named provider callbacks remain implementation violations until the registered child removes them.","No DAG edge or blanket source exception is added by this register."]}

def main():
    ap=argparse.ArgumentParser(); ap.add_argument("--check",action="store_true"); ap.add_argument("--accept",action="store_true",help="fail unless the register contains no unresolved implementation obligation; freshness alone is not acceptance"); ap.add_argument("--output",type=Path,default=OUTPUT); args=ap.parse_args()
    report=build(); out=args.output if args.output.is_absolute() else ROOT/args.output
    if args.check:
        if not out.is_file(): raise SystemExit(f"missing register: {out}")
        existing=json.loads(out.read_text(encoding="utf-8"))
        if existing != report: raise SystemExit("Phase 7 edge disposition register is stale; regenerate without broadening any exception")
        print(f"Phase 7 dependency register: current ({report['counts']['unapproved_candidate_rows']} rows, {report['counts']['uncertainties']} uncertainties; B7 open)")
    else:
        out.parent.mkdir(parents=True,exist_ok=True); out.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
        print(f"Wrote {out}: {report['counts']}")
    if args.accept:
        remaining = [row for row in report["candidate_edges"] if row["disposition"] in {"unapproved", "named-capability-cut"}]
        if remaining or report["counts"]["material_uncertainties"]:
            print(f"Phase 7 acceptance OPEN: {len(remaining)} baseline obligations still require exact post-cut implementation evidence")
            return 1
    return 0
if __name__=="__main__": raise SystemExit(main())
