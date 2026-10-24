import tempfile
import unittest
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_phase7_dependencies as phase7


class Phase7DependencyRegisterTests(unittest.TestCase):
    def edge(self, source: str, target: str, symbol: str, *, source_component="external-sources", target_component="switchboard"):
        return {
            "source_component": source_component,
            "target_component": target_component,
            "source": {"path": "src/source.rs", "line": 1, "name": f"crate::{symbol}"},
            "target": {"path": "src/target.rs", "name": symbol},
        }

    def test_public_root_reexport_is_api_binding(self):
        with tempfile.TemporaryDirectory(dir=phase7.ROOT) as directory:
            root=Path(directory)
            (root/"src").mkdir()
            (root/"src/lib.rs").write_text("pub use memory::MemoryId;\n",encoding="utf-8")
            e=self.edge("src/lib.rs","src/memory.rs","MemoryId",source_component="public-consumers",target_component="memory-store")
            e["source"]["path"]="src/lib.rs"
            self.assertTrue(phase7.is_lib_pub_use(e,root))
            kind,_=phase7.classify(e,set(),root)
            self.assertEqual(kind,"supported-public-api-binding")

    def test_trait_bound_remains_capability_not_value(self):
        with tempfile.TemporaryDirectory(dir=phase7.ROOT) as directory:
            root=Path(directory)
            (root/"src").mkdir()
            (root/"src/source.rs").write_text("fn run<E: EmbeddingEndpoint>(e: &E) { e.embed(); }\n",encoding="utf-8")
            (root/"src/target.rs").write_text("pub trait EmbeddingEndpoint { fn embed(&self); }\n",encoding="utf-8")
            e=self.edge("src/source.rs","src/target.rs","EmbeddingEndpoint")
            kind,_=phase7.classify(e,set(),root)
            self.assertEqual(kind,"protocol-capability-evidence")

    def test_invoked_import_is_not_mistaken_for_value(self):
        with tempfile.TemporaryDirectory(dir=phase7.ROOT) as directory:
            root=Path(directory)
            (root/"src").mkdir()
            (root/"src/source.rs").write_text("fn run() { imported_operation(); }\n",encoding="utf-8")
            (root/"src/target.rs").write_text("pub fn imported_operation() {}\n",encoding="utf-8")
            e=self.edge("src/source.rs","src/target.rs","imported_operation")
            kind,_=phase7.classify(e,set(),root)
            self.assertEqual(kind,"callable-capability-evidence")

    def test_provider_cut_rejects_callback_in_external_source(self):
        self.assertEqual(
            phase7.external_provider_violations({"src/external_source/index.rs":"use crate::EmbeddingEndpoint;"}),
            ["src/external_source/index.rs: residual provider/inference capability `EmbeddingEndpoint`"],
        )
        self.assertEqual(
            phase7.external_provider_violations({"src/external_source/index.rs":"use crate::ExternalEmbeddingService;"}),
            [],
        )

    def test_live_provider_cut_has_no_external_callback(self):
        proof=phase7.current_external_provider_proof()
        self.assertEqual(proof["status"],"pass")
        self.assertEqual(proof["forbidden_provider_or_inference_identifiers"],[])

    def test_stale_exact_exception_fails_closed(self):
        exception={("external-sources","switchboard","StaleType"): {"kind":"immutable"}}
        self.assertEqual(
            phase7.validate_exception_entries(exception,[],[]),
            ["stale-exception:external-sources->switchboard:StaleType"],
        )

    def test_stale_external_protocol_contract_fails_closed(self):
        self.assertEqual(
            phase7.validate_external_value_contracts({"StalePackedType":"value"},[]),
            ["stale-external-contract:StalePackedType"],
        )

    def test_unknown_import_receipt_has_full_nonmaterial_coverage(self):
        receipt=json.loads((phase7.ROOT/"benchmarks/phase7-edge-dispositions.json").read_text(encoding="utf-8"))
        rows=receipt["uncertainties"]
        self.assertEqual(len(rows),97)
        self.assertEqual(receipt["counts"]["material_uncertainties"],0)
        self.assertTrue(all(row["source_file_sha256"] for row in rows))
        self.assertTrue(all(row["disposition"]!="material-unknown" for row in rows))

    def test_used_exact_exception_has_no_stale_finding(self):
        key=("external-sources","vector-store","CompatibilityProfile")
        exception={key:{"kind":"read-only-profile-value"}}
        edges=[{"source_component":key[0],"target_component":key[1],"target":{"name":key[2]}}]
        self.assertEqual(phase7.validate_exception_entries(exception,edges,[]),[])


if __name__=="__main__":
    unittest.main()
