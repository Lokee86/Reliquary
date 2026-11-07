"""Independent regression tests for the provisional synthetic PG-C01 contract."""
import copy
import json
import pathlib
import sys
import unittest

import jsonschema

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import corpus_evidence_v1 as contract


class CorpusEvidenceV1Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.groups, cls.records, cls.carriers = contract.load_registries(ROOT)
        cls.domains = contract.domain_ids_from_registry(cls.records)
        cls.rows = contract.parse_jsonl((ROOT / "fixtures" / "source-rows.v1.jsonl").read_bytes())

    def test_hand_authored_rows_validate_and_schema_matches(self):
        encoded = contract.canonical_rows(self.rows, self.groups, self.records, self.domains)
        self.assertEqual(len(encoded), 8)
        self.assertTrue(contract.canonical_jsonl(encoded).endswith(b"\n"))
        schema = json.loads((ROOT / "schemas" / "semantic-row-v1.schema.json").read_bytes())
        validator = jsonschema.Draft202012Validator(schema)
        validator.check_schema(schema)
        for row in self.rows:
            validator.validate(row)

    def test_strict_json_rejects_ambiguous_or_nonportable_inputs(self):
        bad = [
            b"\xef\xbb\xbf{}",
            b'{"a":1}',
            b'{"a":NaN}',
            b'{"a":"x","\\u0061":"y"}',
            b'{"a":"\\ud800"}',
        ]
        for raw in bad:
            with self.subTest(raw=raw):
                with self.assertRaises(contract.ContractError):
                    contract.strict_json_loads(raw)

    def test_jcs_uses_utf16_code_unit_key_order(self):
        value = {"\ue000": "bmp", "\U00010000": "nonbmp", "a": "M\u00fcnchen e\u0301"}
        expected = b'{"a":"M\xc3\xbcnchen e\xcc\x81","\xf0\x90\x80\x80":"nonbmp","\xee\x80\x80":"bmp"}'
        self.assertEqual(contract.canonical_json_bytes(value), expected)

    def test_independent_golden_vectors_and_alternate_target_encoding(self):
        golden = json.loads((ROOT / "fixtures" / "golden-vectors-v1.json").read_bytes())["vectors"]
        target = contract.parse_jsonl((ROOT / "fixtures" / "target-rows.v1.jsonl").read_bytes())
        source = next(row for row in self.rows if row["family"] == "fixture.memory_revision.v1" and row["revision_or_event"] == "revision-2")
        self.assertEqual(len(target), 4)
        target_current = next(row for row in target if row["family"] == "fixture.memory_revision.v1" and row["revision_or_event"] == "revision-2")
        self.assertEqual(contract.canonical_json_bytes(source), contract.canonical_json_bytes(target_current))
        source_memory = [row for row in self.rows if row["family"] == "fixture.memory_revision.v1"]
        target_memory = [row for row in target if row["family"] == "fixture.memory_revision.v1"]
        self.assertEqual(len(source_memory), 3)
        self.assertEqual(len(target_memory), 3)
        self.assertEqual(next(row for row in source_memory if row["revision_or_event"] == "revision-2"), target_current)
        source_coverage = self._family_fixture_coverage("fixture.memory_revision.v1","memory",source_memory)
        target_coverage = self._family_fixture_coverage("fixture.memory_revision.v1","memory",target_memory)
        owner_cut_ids={entry["cut_id"] for entry in source_coverage["cuts"]}
        source_history_digest = contract.family_digest(source["owner"], source_coverage, source_memory, self.groups, self.records, self.domains, owner_cut_ids)[1]
        target_history_digest = contract.family_digest(source["owner"], target_coverage, target_memory, self.groups, self.records, self.domains, owner_cut_ids)[1]
        self.assertNotEqual(source_history_digest, target_history_digest)
        source_vector = next(row for row in self.rows if row["family"] == "fixture.vector_probe.v1")
        target_vector = next(row for row in target if row["family"] == "fixture.vector_probe.v1")
        vector_cov = self._family_fixture_coverage("fixture.vector_probe.v1","vectors",[source_vector])
        vector_cut_ids={entry["cut_id"] for entry in vector_cov["cuts"]}
        self.assertNotEqual(
            contract.family_digest(source_vector["owner"], vector_cov, [source_vector], self.groups, self.records, self.domains, vector_cut_ids)[1],
            contract.family_digest(target_vector["owner"], vector_cov, [target_vector], self.groups, self.records, self.domains, vector_cut_ids)[1],
        )
        jcs = golden["jcs_utf16"]
        value = json.loads(jcs["input_json_escaped"])
        self.assertEqual(contract.canonical_json_bytes(value).hex(), jcs["canonical_utf8_hex"])
        self.assertEqual([contract.u64be(int(n)).hex() for n in golden["u64be"]["decimal_values"]], golden["u64be"]["bytes_hex"])
        coordinates = {row["coordinate"]["global_version"]: row["coordinate"] for row in self.rows}
        owner_cuts=json.loads((ROOT/"fixtures"/"owner-cuts.v1.json").read_bytes())["cuts"]
        coordinates.update({cut["coordinate"]["global_version"]:cut["coordinate"] for cut in owner_cuts})
        for version, expected in (("10", golden["cut_ids"]["global_version_10"]), ("12", golden["cut_ids"]["global_version_12"]), ("13", golden["cut_ids"]["global_version_13"])):
            coordinate = coordinates[version]
            self.assertEqual(contract.cut_id_for(coordinate), expected)
            contract.validate_cut({"cut_id":expected,"coordinate":coordinate,"availability":"present","completeness":"exhaustive","reason":None}, self.domains)
        self.assertEqual(coordinates["12"]["transaction_time"]["value"], "9007199254740995")
        self.assertEqual(contract.lp(golden["length_prefix"]["payload_utf8"].encode()).hex(), golden["length_prefix"]["encoded_hex"])
        family = golden["empty_family_digest"]
        preimage, digest = contract.family_digest(family["owner"], family["coverage"], [], self.groups, self.records, self.domains, set())
        self.assertEqual(preimage.hex(), family["preimage_hex"])
        self.assertEqual(digest, family["sha256"])

    def test_ordinal_sort_is_numeric_and_duplicate_events_are_retained(self):
        events = [row for row in self.rows if row["family"] == "fixture.freshness_event.v1" and row["availability"] == "present"]
        self.assertEqual(len(events), 2)
        by_ordinal = {row["ordinal"]: row for row in events}
        self.assertLess(contract.row_sort_key(by_ordinal["2"]), contract.row_sort_key(by_ordinal["10"]))
        encoded = contract.canonical_rows(events, self.groups, self.records, self.domains)
        self.assertEqual(len(encoded), 2)
        self.assertNotEqual(encoded[0], encoded[1])
        with self.assertRaisesRegex(contract.ContractError,"duplicate semantic row key"):
            contract.canonical_rows([events[0],copy.deepcopy(events[0])],self.groups,self.records,self.domains)

    def test_empty_stream_and_supported_empty_are_distinct_from_absent(self):
        empty_path = ROOT / "fixtures" / "empty-ego-rows.v1.jsonl"
        self.assertEqual(empty_path.read_bytes(), b"")
        self.assertEqual(contract.parse_jsonl(empty_path.read_bytes()), [])
        self.assertEqual(contract.canonical_jsonl([]), b"")
        supported = {"family":"fixture.ego_anchor.v1","group_id":"ego","presence":"supported","completeness":"exhaustive","row_count":"0","cuts":[],"row_cut_memberships":[]}
        absent = copy.deepcopy(supported)
        absent["presence"] = "absent"
        unsupported = copy.deepcopy(supported)
        unsupported["presence"] = "unsupported"
        unsupported["completeness"] = "incomplete"
        contract.validate_family_coverage(supported, self.records, set())
        contract.validate_family_coverage(absent, self.records, set())
        contract.validate_family_coverage(unsupported, self.records, set())
        self.assertNotEqual(contract.family_digest({"owner_uuid":"00112233445566778899aabbccddeeff","owner_kind":"REL","scope":"project"}, supported, [], self.groups, self.records, self.domains, set())[1],
                            contract.family_digest({"owner_uuid":"00112233445566778899aabbccddeeff","owner_kind":"REL","scope":"project"}, absent, [], self.groups, self.records, self.domains, set())[1])
        invalid_absent = copy.deepcopy(absent)
        invalid_absent["row_count"] = "1"
        with self.assertRaises(contract.ContractError):
            contract.validate_family_coverage(invalid_absent, self.records, set())

    def test_registry_required_logical_provenance_is_enforced(self):
        archive = next(row for row in self.rows if row["family"] == "fixture.archive_turn_revision.v1")
        omitted = copy.deepcopy(archive)
        omitted["source_binding"] = {"kind": "optional", "state": "absent"}
        with self.assertRaisesRegex(contract.ContractError, "requires logical source provenance"):
            contract.validate_semantic_row(omitted, self.groups, self.records, self.domains)

    def test_cross_field_unavailability_and_source_span_rules_fail_closed(self):
        row=next(row for row in self.rows if row["family"]=="fixture.freshness_event.v1" and row["availability"]=="unavailable")
        wrong_family=copy.deepcopy(row)
        wrong_family["family"]="fixture.memory_revision.v1"
        wrong_family["group_id"]="memory"
        wrong_family["value"]["schema_id"]="fixture.memory_revision.v1"
        with self.assertRaisesRegex(contract.ContractError,"Freshness-only"):
            contract.validate_semantic_row(wrong_family,self.groups,self.records,self.domains)
        archive=copy.deepcopy(next(row for row in self.rows if row["family"]=="fixture.archive_turn_revision.v1"))
        archive["source_binding"]["value"]["fields"]["source_body_span_start"]["decimal"]="20"
        with self.assertRaisesRegex(contract.ContractError,"half-open byte range"):
            contract.validate_semantic_row(archive,self.groups,self.records,self.domains)

    def test_unavailable_body_is_typed_and_not_coerced_to_empty(self):
        row = next(row for row in self.rows if row["family"] == "fixture.memory_revision.v1" and row["availability"] == "pruned")
        self.assertEqual(row["value"]["kind"], "unavailable")
        self.assertEqual(row["value"]["reason_code"], "pruned")
        self.assertEqual(row["source_binding"]["state"], "present")

    def test_unknown_schema_and_extra_record_fields_fail_closed(self):
        row = copy.deepcopy(self.rows[0])
        row["family"] = "fixture.unreviewed.v1"
        with self.assertRaises(contract.ContractError):
            contract.validate_semantic_row(row, self.groups, self.records, self.domains)
        row = copy.deepcopy(next(r for r in self.rows if r["availability"] == "present"))
        row["value"]["fields"]["unreviewed"] = {"kind": "text", "value": "ignored"}
        with self.assertRaisesRegex(contract.ContractError, "exactly match"):
            contract.validate_semantic_row(row, self.groups, self.records, self.domains)

    def test_intermediate_history_and_logical_provenance_affect_semantic_digest(self):
        memories = [row for row in self.rows if row["family"] == "fixture.memory_revision.v1"]
        coverage = self._family_fixture_coverage("fixture.memory_revision.v1","memory",memories)
        owner_cut_ids={entry["cut_id"] for entry in coverage["cuts"]}
        owner = memories[0]["owner"]
        original_digest = contract.family_digest(owner, coverage, memories, self.groups, self.records, self.domains, owner_cut_ids)[1]
        mutated = copy.deepcopy(memories)
        old = next(row for row in mutated if row["revision_or_event"] == "revision-1")
        old["value"]["fields"]["body"]["value"]["value"] = "intermediate body changed"
        mutated_digest = contract.family_digest(
            owner, coverage, mutated, self.groups, self.records, self.domains, owner_cut_ids
        )[1]
        self.assertNotEqual(original_digest, mutated_digest)
        self.assertEqual(
            next(row for row in memories if row["revision_or_event"] == "revision-2"),
            next(row for row in mutated if row["revision_or_event"] == "revision-2"),
        )
        provenance_changed = copy.deepcopy(memories)
        provenance_changed[0]["source_binding"]["value"]["fields"]["source_body_sha256"]["hex"] = "c" * 64
        provenance_digest = contract.family_digest(
            owner, coverage, provenance_changed, self.groups, self.records, self.domains, owner_cut_ids
        )[1]
        self.assertNotEqual(original_digest, provenance_digest)

    def test_physical_offsets_change_custody_not_semantic_rows(self):
        custody_a = {
            "source_file_sha256": "a" * 64, "target_file_sha256": "b" * 64,
            "relocations": [{
                "owner_uuid": "00112233445566778899aabbccddeeff",
                "family": "fixture.memory_revision.v1", "source_object_sha256": "1" * 64,
                "source_offset": "100", "source_length": "20",
                "target_object_sha256": "2" * 64, "target_offset": "300",
                "target_length": "20", "payload_identity": "abcd",
                "reference_kind": "body",
            }],
        }
        custody_b = copy.deepcopy(custody_a)
        custody_b["relocations"][0]["target_offset"] = "400"
        self.assertNotEqual(contract.custody_digest(custody_a, self.records)[1], contract.custody_digest(custody_b, self.records)[1])
        second=copy.deepcopy(custody_a["relocations"][0])
        second["source_offset"]="200"
        custody_many=copy.deepcopy(custody_a)
        custody_many["relocations"].append(second)
        reversed_many=copy.deepcopy(custody_many)
        reversed_many["relocations"].reverse()
        self.assertEqual(contract.custody_digest(custody_many,self.records)[1],contract.custody_digest(reversed_many,self.records)[1])
        bad_family=copy.deepcopy(custody_a)
        bad_family["relocations"][0]["family"]="memory"
        with self.assertRaises(contract.ContractError):
            contract.custody_digest(bad_family,self.records)
        duplicate=copy.deepcopy(custody_many)
        duplicate["relocations"].append(copy.deepcopy(duplicate["relocations"][0]))
        with self.assertRaisesRegex(contract.ContractError,"duplicate physical relocation"):
            contract.custody_digest(duplicate,self.records)
        memory_rows = [row for row in self.rows if row["family"] == "fixture.memory_revision.v1"]
        row = memory_rows[0]
        coverage = self._family_fixture_coverage(row["family"],row["group_id"],memory_rows)
        owner_cut_ids={entry["cut_id"] for entry in coverage["cuts"]}
        contract.canonical_rows(memory_rows, self.groups, self.records, self.domains)
        semantic_before = contract.family_digest(row["owner"], coverage, memory_rows, self.groups, self.records, self.domains, owner_cut_ids)[1]
        custody_before = contract.custody_digest(custody_a, self.records)[1]
        custody_after = contract.custody_digest(custody_b, self.records)[1]
        semantic_after = contract.family_digest(row["owner"], coverage, memory_rows, self.groups, self.records, self.domains, owner_cut_ids)[1]
        self.assertNotEqual(custody_before, custody_after)
        self.assertEqual(semantic_before, semantic_after)

    def test_authentic_admission_is_blocked_for_synthetic_registries(self):
        with self.assertRaises(contract.ContractError):
            contract.validate_authentic_admission(self.groups, self.records, self.carriers)

    def _family_fixture_coverage(self,family,group,rows,owner_cut_ids=None):
        catalog=json.loads((ROOT/"fixtures"/"owner-cuts.v1.json").read_bytes())
        cuts=catalog["cuts"]
        all_cut_ids={entry["cut_id"] for entry in cuts}
        if owner_cut_ids is not None and set(owner_cut_ids)!=all_cut_ids:
            raise AssertionError("fixture coverage must use the complete explicit owner cut catalog")
        table=json.loads((ROOT/"fixtures"/"row-cut-memberships.v1.json").read_bytes())
        entries=[entry for entry in table["entries"] if entry["family"]==family]
        memberships=[]
        for entry in entries:
            for cut_id in entry["observation_cut_ids"]:
                memberships.append({"stable_key":entry["stable_key"],"revision_or_event":entry["revision_or_event"],"ordinal":entry["ordinal"],"cut_id":cut_id})
        memberships.sort(key=lambda m:(m["stable_key"].encode("utf-8"),m["revision_or_event"].encode("utf-8"),contract.u64be(int(m["ordinal"])),bytes.fromhex(m["cut_id"])))
        row_keys={(row["stable_key"],row["revision_or_event"],row["ordinal"]) for row in rows}
        covered_keys={(m["stable_key"],m["revision_or_event"],m["ordinal"]) for m in memberships}
        if row_keys!=covered_keys:
            raise AssertionError(f"fixture membership rows differ for {family}")
        counts={cut_id:sum(1 for m in memberships if m["cut_id"]==cut_id) for cut_id in all_cut_ids}
        cut_coverage=[{"cut_id":cut["cut_id"],"presence":"supported","completeness":"exhaustive","row_count":str(counts[cut["cut_id"]]),"reason":None} for cut in cuts]
        return {"family":family,"group_id":group,"presence":"supported","completeness":"exhaustive","row_count":str(len(rows)),"cuts":cut_coverage,"row_cut_memberships":memberships}

    def _owner_fixture(self):
        owner = {"owner_uuid":"00112233445566778899aabbccddeeff","owner_kind":"REL","scope":"project"}
        catalog=json.loads((ROOT/"fixtures"/"owner-cuts.v1.json").read_bytes())
        cuts=catalog["cuts"]
        descriptor = {**owner,"history_generation":"3","lineage":{"kind":"optional","state":"absent"},
                      "version_heads":[{"domain_id":"fixture.activity","version":"12"},{"domain_id":"fixture.archive","version":"6"},
                                       {"domain_id":"fixture.global","version":"13"},{"domain_id":"fixture.memory","version":"4"}],
                      "supported_cuts":cuts}
        cut_ids={entry["cut_id"] for entry in cuts}
        family_rows={}
        family_coverage=[]
        for schema in self.records["records"]:
            if schema["semantic_family"] is not True:
                continue
            family=schema["schema_id"]
            rows=[row for row in self.rows if row["family"]==family]
            family_rows[family]=rows
            family_coverage.append(self._family_fixture_coverage(family,schema["group_id"],rows,cut_ids))
        group_coverage=[]
        for group in self.groups["groups"]:
            count=sum(1 for schema in self.records["records"] if schema["semantic_family"] is True and schema["group_id"]==group["id"])
            if count:
                group_coverage.append({"group_id":group["id"],"presence":"supported","completeness":"exhaustive","family_count":str(count),"reason":None})
            else:
                group_coverage.append({"group_id":group["id"],"presence":"unclassified","completeness":"incomplete","family_count":"0","reason":"no reviewed synthetic fixture schema"})
        return descriptor,group_coverage,family_coverage,family_rows

    def test_owner_and_corpus_digests_bind_complete_catalog_and_cuts(self):
        descriptor,groups,families,rows=self._owner_fixture()
        preimage,digest=contract.owner_digest(descriptor,groups,families,rows,self.groups,self.records,self.domains,self.carriers)
        shuffled_preimage,shuffled_digest=contract.owner_digest(descriptor,list(reversed(groups)),list(reversed(families)),rows,self.groups,self.records,self.domains,self.carriers)
        self.assertEqual(preimage,shuffled_preimage)
        self.assertEqual(digest,shuffled_digest)
        evidence={"descriptor":descriptor,"group_coverage":groups,"family_coverage":families,"family_rows":rows,"claimed_owner_digest":digest}
        corpus_preimage,corpus_hash=contract.corpus_digest([evidence],self.groups,self.records,self.domains,self.carriers)
        self.assertTrue(corpus_preimage.startswith(contract.CORPUS_DOMAIN))
        self.assertEqual(len(corpus_hash),64)
        tampered_claim=copy.deepcopy(evidence)
        tampered_claim["claimed_owner_digest"]="0"*64
        with self.assertRaisesRegex(contract.ContractError,"claimed owner digest does not match"):
            contract.corpus_digest([tampered_claim],self.groups,self.records,self.domains,self.carriers)

        second_descriptor=copy.deepcopy(descriptor)
        second_descriptor["owner_uuid"]="ffeeddccbbaa99887766554433221100"
        second_rows={}
        for family, family_values in rows.items():
            second_rows[family]=[]
            for source_row in family_values:
                row=copy.deepcopy(source_row)
                row["owner"]["owner_uuid"]=second_descriptor["owner_uuid"]
                second_rows[family].append(row)
        _,second_digest=contract.owner_digest(second_descriptor,groups,families,second_rows,self.groups,self.records,self.domains,self.carriers)
        second_evidence={"descriptor":second_descriptor,"group_coverage":groups,"family_coverage":families,"family_rows":second_rows,"claimed_owner_digest":second_digest}
        ordered_hash=contract.corpus_digest([evidence,second_evidence],self.groups,self.records,self.domains,self.carriers)[1]
        reversed_hash=contract.corpus_digest([second_evidence,evidence],self.groups,self.records,self.domains,self.carriers)[1]
        self.assertEqual(ordered_hash,reversed_hash)
        swapped_claims=[copy.deepcopy(evidence),copy.deepcopy(second_evidence)]
        swapped_claims[0]["claimed_owner_digest"]=second_digest
        swapped_claims[1]["claimed_owner_digest"]=digest
        with self.assertRaisesRegex(contract.ContractError,"claimed owner digest does not match"):
            contract.corpus_digest(swapped_claims,self.groups,self.records,self.domains,self.carriers)

        changed=copy.deepcopy(descriptor)
        changed["history_generation"]="4"
        self.assertNotEqual(contract.owner_digest(changed,groups,families,rows,self.groups,self.records,self.domains,self.carriers)[1],digest)
        changed_registry=copy.deepcopy(self.groups)
        changed_registry["groups"][0]["retained_value_obligations"].append("Catalog identity test marker.")
        changed_registry_digest=contract.owner_digest(descriptor,groups,families,rows,changed_registry,self.records,self.domains,self.carriers)[1]
        self.assertNotEqual(changed_registry_digest,digest)
        changed_corpus_hash=contract.corpus_digest([{"descriptor":descriptor,"group_coverage":groups,"family_coverage":families,"family_rows":rows}],changed_registry,self.records,self.domains,self.carriers)[1]
        self.assertNotEqual(changed_corpus_hash,corpus_hash)

        wrong_scope=copy.deepcopy(descriptor)
        wrong_scope["scope"]="organization"
        with self.assertRaisesRegex(contract.ContractError,"outside the declared family or owner"):
            contract.owner_digest(wrong_scope,groups,families,rows,self.groups,self.records,self.domains,self.carriers)
        missing_family_rows=dict(rows)
        missing_family_rows.pop(next(iter(missing_family_rows)))
        with self.assertRaises(contract.ContractError):
            contract.owner_digest(descriptor,groups,families,missing_family_rows,self.groups,self.records,self.domains,self.carriers)
        omitted_cut=copy.deepcopy(families)
        omitted_cut[0]["cuts"].pop()
        with self.assertRaises(contract.ContractError):
            contract.owner_digest(descriptor,groups,omitted_cut,rows,self.groups,self.records,self.domains,self.carriers)
        freshness=next(row for row in families if row["family"]=="fixture.freshness_event.v1")
        event_membership=[entry for entry in freshness["row_cut_memberships"] if entry["revision_or_event"]=="accepted-event:aa" and entry["ordinal"]=="10"]
        self.assertEqual(len(event_membership),2)
        origin=next(row for row in rows["fixture.freshness_event.v1"] if row["revision_or_event"]=="accepted-event:aa" and row["ordinal"]=="10")
        self.assertEqual(origin["coordinate"]["global_version"],"12")
        self.assertIn("84f64a07fb495814b3e2834544557ac5997f5bdf03623ba45440565018264ed7",{entry["cut_id"] for entry in event_membership})
        moved=copy.deepcopy(freshness)
        moved["row_cut_memberships"]=[entry for entry in moved["row_cut_memberships"] if not (entry["revision_or_event"]=="accepted-event:aa" and entry["ordinal"]=="10" and entry["cut_id"]=="84f64a07fb495814b3e2834544557ac5997f5bdf03623ba45440565018264ed7")]
        cut13=next(entry for entry in moved["cuts"] if entry["cut_id"]=="84f64a07fb495814b3e2834544557ac5997f5bdf03623ba45440565018264ed7")
        cut13["row_count"]=str(int(cut13["row_count"])-1)
        owner_key={key:descriptor[key] for key in ("owner_uuid","owner_kind","scope")}
        original_family_digest=contract.family_digest(owner_key,freshness,rows["fixture.freshness_event.v1"],self.groups,self.records,self.domains,{cut["cut_id"] for cut in descriptor["supported_cuts"]})[1]
        moved_family_digest=contract.family_digest(owner_key,moved,rows["fixture.freshness_event.v1"],self.groups,self.records,self.domains,{cut["cut_id"] for cut in descriptor["supported_cuts"]})[1]
        self.assertNotEqual(original_family_digest,moved_family_digest)
        mismatched_group=copy.deepcopy(groups)
        next(row for row in mismatched_group if row["group_id"]=="memory")["presence"]="absent"
        with self.assertRaises(contract.ContractError):
            contract.owner_digest(descriptor,mismatched_group,families,rows,self.groups,self.records,self.domains,self.carriers)

    def test_all_contract_schemas_validate_examples(self):
        descriptor,groups,families,rows=self._owner_fixture()
        report=json.loads((ROOT/"fixtures"/"report-example-incomplete.v1.json").read_bytes())
        custody={"source_file_sha256":"a"*64,"target_file_sha256":"b"*64,"relocations":[{"owner_uuid":"00112233445566778899aabbccddeeff","family":"fixture.memory_revision.v1","source_object_sha256":"1"*64,"source_offset":"100","source_length":"20","target_object_sha256":"2"*64,"target_offset":"300","target_length":"20","payload_identity":"abcd","reference_kind":"body"}]}
        examples={
            "semantic-owner-v1.schema.json":descriptor,
            "group-coverage-v1.schema.json":groups[0],
            "family-coverage-v1.schema.json":families[0],
            "artifact-report-v1.schema.json":report,
            "physical-custody-v1.schema.json":custody,
        }
        for path in (ROOT/"schemas").glob("*.schema.json"):
            schema=json.loads(path.read_bytes())
            validator=jsonschema.Draft202012Validator(schema)
            validator.check_schema(schema)
            if path.name in examples:
                validator.validate(examples[path.name])
        contract.validate_artifact_report(report,ROOT)
        tampered=copy.deepcopy(report)
        tampered["registry_hashes"]["record_schemas"]="0"*64
        with self.assertRaisesRegex(contract.ContractError,"registry hashes"):
            contract.validate_artifact_report(tampered,ROOT)
        incomplete_pass=copy.deepcopy(report)
        incomplete_pass["status"]="PASS"
        with self.assertRaisesRegex(contract.ContractError,"PASS report"):
            contract.validate_artifact_report(incomplete_pass,ROOT)
        authentic=copy.deepcopy(report)
        authentic["evidence_scope"]="authentic_source"
        with self.assertRaises(contract.ContractError):
            contract.validate_artifact_report(authentic,ROOT)
        row_schema=json.loads((ROOT/"schemas"/"semantic-row-v1.schema.json").read_bytes())
        for row in self.rows:
            jsonschema.validate(row,row_schema)

    def test_registry_bundle_is_bound_and_hash_is_stable(self):
        first = contract.registry_hashes(ROOT)
        second = contract.registry_hashes(ROOT)
        self.assertEqual(first, second)
        self.assertEqual(set(first), {"semantic_groups", "record_schemas", "physical_carriers", "contract_bundle"})
        self.assertEqual((ROOT/"fixtures"/"bundle.sha256").read_text(encoding="utf-8").strip(),first["contract_bundle"])
        self.assertEqual(self.carriers["coverage"], "incomplete")
        self.assertEqual(self.carriers["entries"], [])


if __name__ == "__main__":
    unittest.main()
