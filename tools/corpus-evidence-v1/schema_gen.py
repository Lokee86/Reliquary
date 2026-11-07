"""Generate the committed JSON Schema resources from the versioned registries."""
from __future__ import annotations
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def tagged_text():
    return {"type":"object","properties":{"kind":{"const":"text"},"value":{"type":"string"}},"required":["kind","value"],"additionalProperties":False}


def tagged_bytes(pattern):
    return {"type":"object","properties":{"kind":{"const":"bytes"},"hex":{"type":"string","pattern":pattern}},"required":["kind","hex"],"additionalProperties":False}


def tagged_integer(signed):
    pattern = "^(?:0|[1-9][0-9]*|-[1-9][0-9]*)$" if signed else "^(?:0|[1-9][0-9]*)$"
    return {"type":"object","properties":{"kind":{"const":"integer"},"signed":{"const":signed},"decimal":{"type":"string","pattern":pattern}},"required":["kind","signed","decimal"],"additionalProperties":False}


def field_schema(kind):
    if kind == "text":
        return tagged_text()
    if kind == "hex":
        return tagged_bytes("^(?:[0-9a-f]{2})+$")
    if kind == "sha256":
        return tagged_bytes("^[0-9a-f]{64}$")
    if kind == "u64":
        return tagged_integer(False)
    if kind == "i64":
        return tagged_integer(True)
    if kind == "float_bits":
        return {"oneOf":[
            {"type":"object","properties":{"kind":{"const":"float_bits"},"scalar":{"const":"f32"},"endianness":{"enum":["little","big"]},"hex":{"type":"string","pattern":"^[0-9a-f]{8}$"}},"required":["kind","scalar","endianness","hex"],"additionalProperties":False},
            {"type":"object","properties":{"kind":{"const":"float_bits"},"scalar":{"const":"f64"},"endianness":{"enum":["little","big"]},"hex":{"type":"string","pattern":"^[0-9a-f]{16}$"}},"required":["kind","scalar","endianness","hex"],"additionalProperties":False}
        ]}
    if kind == "optional_text":
        return {"oneOf":[
            {"type":"object","properties":{"kind":{"const":"optional"},"state":{"const":"absent"}},"required":["kind","state"],"additionalProperties":False},
            {"type":"object","properties":{"kind":{"const":"optional"},"state":{"const":"present"},"value":tagged_text()},"required":["kind","state","value"],"additionalProperties":False}
        ]}
    if kind == "hex_array":
        return {"type":"object","properties":{"kind":{"const":"array"},"ordering":{"const":"ordered"},"items":{"type":"array","items":tagged_bytes("^(?:[0-9a-f]{2})+$")}},"required":["kind","ordering","items"],"additionalProperties":False}
    raise ValueError(f"unknown registered field type: {kind}")


def schema_documents():
    groups = json.loads((ROOT/"registry/semantic-groups-v1.json").read_text(encoding="utf-8"))
    registry = json.loads((ROOT/"registry/record-schemas-v1.json").read_text(encoding="utf-8"))
    group_ids = [row["id"] for row in groups["groups"]]
    records = registry["records"]
    family_ids = [row["schema_id"] for row in records if row.get("semantic_family") is True]

    record_variants = []
    for record in records:
        fields = {field["name"]: field_schema(field["type"]) for field in record["fields"]}
        record_variants.append({
            "type":"object",
            "properties":{
                "kind":{"const":"record"},
                "schema_id":{"const":record["schema_id"]},
                "fields":{"type":"object","properties":fields,"required":list(fields),"additionalProperties":False}
            },
            "required":["kind","schema_id","fields"],
            "additionalProperties":False
        })
    defs = {
        "recordValue":{"oneOf":record_variants},
        "decimalU64":{"type":"string","pattern":"^(?:0|[1-9][0-9]*)$"},
        "decimalI64":{"type":"string","pattern":"^(?:0|[1-9][0-9]*|-[1-9][0-9]*)$"},
        "ownerKey":{"type":"object","properties":{"owner_uuid":{"type":"string","pattern":"^[0-9a-f]{32}$"},"owner_kind":{"enum":["REL","PHY"]},"scope":{"type":"string"}},"required":["owner_uuid","owner_kind","scope"],"additionalProperties":False}
    }
    for variant, record in zip(record_variants, records):
        defs[record["schema_id"].replace(".","_")] = variant
    lineage_id = next(row["schema_id"] for row in records if row["schema_id"] == "fixture.owner_lineage.v1")
    provenance_id = next(row["schema_id"] for row in records if row["schema_id"] == "fixture.logical_provenance.v1")
    optional_lineage = {"oneOf":[
        {"type":"object","properties":{"kind":{"const":"optional"},"state":{"const":"absent"}},"required":["kind","state"],"additionalProperties":False},
        {"type":"object","properties":{"kind":{"const":"optional"},"state":{"const":"present"},"value":{"$ref":"#/$defs/"+lineage_id.replace(".","_")}},"required":["kind","state","value"],"additionalProperties":False}
    ]}
    optional_provenance = {"oneOf":[
        {"type":"object","properties":{"kind":{"const":"optional"},"state":{"const":"absent"}},"required":["kind","state"],"additionalProperties":False},
        {"type":"object","properties":{"kind":{"const":"optional"},"state":{"const":"present"},"value":{"$ref":"#/$defs/"+provenance_id.replace(".","_")}},"required":["kind","state","value"],"additionalProperties":False}
    ]}
    defs["optionalLineage"] = optional_lineage
    defs["optionalProvenance"] = optional_provenance
    coordinate = {
        "type":"object",
        "properties":{
            "global_version":{"$ref":"#/$defs/decimalU64"},
            "domain_versions":{"type":"array","items":{"type":"object","properties":{"domain_id":{"type":"string","enum":registry["domain_ids"]},"version":{"$ref":"#/$defs/decimalU64"}},"required":["domain_id","version"],"additionalProperties":False}},
            "transaction_time":{"oneOf":[{"type":"object","properties":{"state":{"const":"absent"}},"required":["state"],"additionalProperties":False},{"type":"object","properties":{"state":{"const":"present"},"value":{"$ref":"#/$defs/decimalI64"}},"required":["state","value"],"additionalProperties":False}]},
            "rel_activity_position":{"oneOf":[{"type":"object","properties":{"state":{"const":"absent"}},"required":["state"],"additionalProperties":False},{"type":"object","properties":{"state":{"const":"present"},"value":{"$ref":"#/$defs/decimalU64"}},"required":["state","value"],"additionalProperties":False}]},
            "accepted_event_boundary":{"oneOf":[{"type":"object","properties":{"state":{"const":"absent"}},"required":["state"],"additionalProperties":False},{"type":"object","properties":{"state":{"const":"present"},"value":{"$ref":"#/$defs/decimalU64"}},"required":["state","value"],"additionalProperties":False}]}
        },
        "required":["global_version","domain_versions","transaction_time","rel_activity_position","accepted_event_boundary"],
        "additionalProperties":False
    }
    defs["coordinate"] = coordinate
    defs["cut"] = {"type":"object","properties":{"cut_id":{"type":"string","pattern":"^[0-9a-f]{64}$"},"coordinate":{"$ref":"#/$defs/coordinate"},"availability":{"enum":["present","missing","tombstoned","pruned","unavailable","unauthorized","corrupt"]},"completeness":{"enum":["exhaustive","sampled","incomplete"]},"reason":{"type":["string","null"]}},"required":["cut_id","coordinate","availability","completeness","reason"],"additionalProperties":False}
    reasons = ["missing","tombstoned","pruned","unavailable","unauthorized","corrupt","intra_settlement_unavailable"]
    defs["unavailableValue"] = {"oneOf":[{"type":"object","properties":{"kind":{"const":"unavailable"},"schema_id":{"const":family},"reason_code":{"enum":reasons},"evidence_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"}},"required":["kind","schema_id","reason_code","evidence_sha256"],"additionalProperties":False} for family in family_ids]}
    source_bound_families = [row["schema_id"] for row in records if row.get("semantic_family") is True and row.get("requires_source_binding") is True]
    row_schema = {"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://reliquary.invalid/corpus-evidence/semantic-row-v1","title":"Semantic row v1","type":"object","$defs":defs,"properties":{
        "family":{"enum":family_ids},"group_id":{"enum":group_ids},"stable_key":{"type":"string"},"revision_or_event":{"type":"string"},"ordinal":{"$ref":"#/$defs/decimalU64"},"owner":{"$ref":"#/$defs/ownerKey"},"coordinate":{"$ref":"#/$defs/coordinate"},"availability":{"enum":["present","missing","tombstoned","pruned","unavailable","unauthorized","corrupt"]},"value":{"oneOf":[{"$ref":"#/$defs/recordValue"},{"$ref":"#/$defs/unavailableValue"}]},"source_binding":{"$ref":"#/$defs/optionalProvenance"}
    },"required":["family","group_id","stable_key","revision_or_event","ordinal","owner","coordinate","availability","value","source_binding"],"allOf":[
        {"if":{"properties":{"availability":{"const":"present"}},"required":["availability"]},"then":{"properties":{"value":{"$ref":"#/$defs/recordValue"}}}},
        {"if":{"properties":{"availability":{"enum":["missing","tombstoned","pruned","unavailable","unauthorized","corrupt"]}},"required":["availability"]},"then":{"properties":{"value":{"$ref":"#/$defs/unavailableValue"}}}},
        {"if":{"properties":{"family":{"enum":source_bound_families}},"required":["family"]},"then":{"properties":{"source_binding":{"properties":{"state":{"const":"present"}},"required":["state"]}}}}
    ],"additionalProperties":False}
    owner_schema = {"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://reliquary.invalid/corpus-evidence/owner-v1","title":"Semantic owner descriptor v1","type":"object","$defs":defs,"properties":{
        "owner_uuid":{"type":"string","pattern":"^[0-9a-f]{32}$"},"owner_kind":{"enum":["REL","PHY"]},"scope":{"type":"string"},"history_generation":{"$ref":"#/$defs/decimalU64"},"lineage":{"$ref":"#/$defs/optionalLineage"},
        "version_heads":{"type":"array","items":{"type":"object","properties":{"domain_id":{"type":"string","enum":registry["domain_ids"]},"version":{"$ref":"#/$defs/decimalU64"}},"required":["domain_id","version"],"additionalProperties":False}},
        "supported_cuts":{"type":"array","items":{"$ref":"#/$defs/cut"}}
    },"required":["owner_uuid","owner_kind","scope","history_generation","lineage","version_heads","supported_cuts"],"additionalProperties":False}
    cut_cov = {"type":"object","properties":{"cut_id":{"type":"string","pattern":"^[0-9a-f]{64}$"},"presence":{"enum":["supported","absent","unsupported","unavailable","corrupt","unaudited","unclassified"]},"completeness":{"enum":["exhaustive","sampled","incomplete"]},"row_count":{"$ref":"#/$defs/decimalU64"},"reason":{"type":["string","null"]}},"required":["cut_id","presence","completeness","row_count","reason"],"additionalProperties":False}
    family_coverage = {"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://reliquary.invalid/corpus-evidence/family-coverage-v1","type":"object","$defs":{"decimalU64":defs["decimalU64"],"cutCoverage":cut_cov},"properties":{
        "family":{"enum":family_ids},"group_id":{"enum":group_ids},"presence":{"enum":["supported","absent","unsupported","unavailable","corrupt","unaudited","unclassified"]},"completeness":{"enum":["exhaustive","sampled","incomplete"]},"row_count":{"$ref":"#/$defs/decimalU64"},"cuts":{"type":"array","items":{"$ref":"#/$defs/cutCoverage"}}
    },"required":["family","group_id","presence","completeness","row_count","cuts"],"additionalProperties":False}
    group_coverage = {"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://reliquary.invalid/corpus-evidence/group-coverage-v1","type":"object","properties":{
        "group_id":{"enum":group_ids},"presence":{"enum":["supported","absent","unsupported","unavailable","corrupt","unaudited","unclassified"]},"completeness":{"enum":["exhaustive","sampled","incomplete"]},"family_count":{"type":"string","pattern":"^(?:0|[1-9][0-9]*)$"},"reason":{"type":["string","null"]}
    },"required":["group_id","presence","completeness","family_count","reason"],"additionalProperties":False}
    hexhash = {"type":"string","pattern":"^[0-9a-f]{64}$"}
    report = {"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://reliquary.invalid/corpus-evidence/artifact-report-v1","type":"object","properties":{
        "schema_id":{"const":"reliquary-corpus-artifact-report-v1"},"contract_id":{"const":"reliquary-corpus-evidence-v1"},"producer_role":{"enum":["source_reader","converter","target_reader","comparator","staging","benchmark"]},"evidence_scope":{"enum":["synthetic_fixture","authentic_source","target_candidate","benchmark"]},"producer_revision":{"type":"string","pattern":"^[0-9a-f]{40}$"},"producer_binary_sha256":hexhash,"producer_code_sha256":hexhash,"validation_patch_sha256":{"oneOf":[hexhash,{"type":"null"}]},"lockfile_sha256":hexhash,"toolchain_id":{"type":"string","minLength":1},
        "registry_hashes":{"type":"object","properties":{"semantic_groups":hexhash,"record_schemas":hexhash,"physical_carriers":hexhash,"contract_bundle":hexhash},"required":["semantic_groups","record_schemas","physical_carriers","contract_bundle"],"additionalProperties":False},
        "status":{"enum":["PASS","FAIL","INCOMPLETE"]},"captured_at_utc":{"type":"string","pattern":"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\\.[0-9]+)?Z$"},"reporting_date_local":{"type":"string","pattern":"^[0-9]{4}-[0-9]{2}-[0-9]{2}$"},"reporting_timezone":{"type":"string","minLength":1},
        "command_argv":{"type":"array","items":{"type":"string"}},"exit_status":{"type":"string","pattern":"^(?:0|[1-9][0-9]*|-[1-9][0-9]*)$"},
        "source_hashes":{"type":"array","items":hexhash},"parent_artifact_hashes":{"type":"array","items":hexhash},
        "coverage":{"type":"object","properties":{"owner_count":{"type":"string","pattern":"^(?:0|[1-9][0-9]*)$"},"family_count":{"type":"string","pattern":"^(?:0|[1-9][0-9]*)$"},"cut_count":{"type":"string","pattern":"^(?:0|[1-9][0-9]*)$"},"completeness":{"enum":["exhaustive","sampled","incomplete"]}},"required":["owner_count","family_count","cut_count","completeness"],"additionalProperties":False},
        "diagnostics":{"type":"object","properties":{"total_count":{"type":"string","pattern":"^(?:0|[1-9][0-9]*)$"},"sample_limit":{"type":"string","pattern":"^(?:0|[1-9][0-9]*)$"},"sample":{"type":"array","items":{"type":"object","properties":{"owner_uuid":{"type":"string","pattern":"^[0-9a-f]{32}$"},"family":{"enum":family_ids},"stable_key":{"type":"string"},"revision_or_event":{"type":"string"},"cut_id":{"type":"string","pattern":"^[0-9a-f]{64}$"},"expected_availability":{"enum":["present","missing","tombstoned","pruned","unavailable","unauthorized","corrupt"]},"observed_availability":{"enum":["present","missing","tombstoned","pruned","unavailable","unauthorized","corrupt"]},"expected_sha256":hexhash,"observed_sha256":hexhash},"required":["owner_uuid","family","stable_key","revision_or_event","cut_id","expected_availability","observed_availability","expected_sha256","observed_sha256"],"additionalProperties":False}}},"required":["total_count","sample_limit","sample"],"additionalProperties":False}
    },"required":["schema_id","contract_id","producer_role","evidence_scope","producer_revision","producer_binary_sha256","producer_code_sha256","validation_patch_sha256","lockfile_sha256","toolchain_id","registry_hashes","status","captured_at_utc","reporting_date_local","reporting_timezone","command_argv","exit_status","source_hashes","parent_artifact_hashes","coverage","diagnostics"],"additionalProperties":False}
    custody = {"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://reliquary.invalid/corpus-evidence/physical-custody-v1","type":"object","properties":{
        "source_file_sha256":hexhash,"target_file_sha256":hexhash,"relocations":{"type":"array","items":{"type":"object","properties":{"owner_uuid":{"type":"string","pattern":"^[0-9a-f]{32}$"},"family":{"enum":family_ids},"source_object_sha256":hexhash,"source_offset":{"type":"string","pattern":"^(?:0|[1-9][0-9]*)$"},"source_length":{"type":"string","pattern":"^(?:0|[1-9][0-9]*)$"},"target_object_sha256":hexhash,"target_offset":{"type":"string","pattern":"^(?:0|[1-9][0-9]*)$"},"target_length":{"type":"string","pattern":"^(?:0|[1-9][0-9]*)$"},"payload_identity":{"type":"string","pattern":"^(?:[0-9a-f]{2})+$"},"reference_kind":{"type":"string","minLength":1}},"required":["owner_uuid","family","source_object_sha256","source_offset","source_length","target_object_sha256","target_offset","target_length","payload_identity","reference_kind"],"additionalProperties":False}}
    },"required":["source_file_sha256","target_file_sha256","relocations"],"additionalProperties":False}
    docs = {
        "semantic-row-v1.schema.json":row_schema,
        "semantic-owner-v1.schema.json":owner_schema,
        "family-coverage-v1.schema.json":family_coverage,
        "group-coverage-v1.schema.json":group_coverage,
        "artifact-report-v1.schema.json":report,
        "physical-custody-v1.schema.json":custody
    }
    return docs


def main():
    folder = ROOT/"schemas"
    folder.mkdir(parents=True, exist_ok=True)
    for name, schema in schema_documents().items():
        (folder/name).write_text(json.dumps(schema, ensure_ascii=False, indent=2, sort_keys=True)+"\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()