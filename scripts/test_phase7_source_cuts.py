import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_phase7_source_cuts import violations


class SourceCutTests(unittest.TestCase):
    def test_physical_capability_negative(self):
        self.assertTrue(violations("src/runtime_inference/dream_owner_candidates.rs", "fn read(owner: &mut Container) {}", {"windows"}))

    def test_runtime_callback_negative(self):
        self.assertTrue(violations("src/runtime_inference/dream_owner_lifecycle.rs", "fn apply(gateway: &GatewayRuntime) {}", {"windows"}))

    def test_dream_owner_callback_negative(self):
        self.assertTrue(violations("src/runtime_inference/dream_canonical.rs", "trait DreamOwnerRead { fn same_publication_cohort(&self); }", {"windows"}))

    def test_provider_negative(self):
        self.assertTrue(violations("src/external_source/index.rs", "fn run<E: EmbeddingEndpoint>(e: E) {}", {"windows"}))

    def test_owner_cache_negative(self):
        self.assertTrue(violations("src/owner_lifecycle/reliquary.rs", "struct Owner { cache: LexicalIndex }", {"windows"}))

    def test_concrete_owner_reader_positive(self):
        self.assertFalse(violations("src/runtime_inference/dream_owner_candidates.rs", "fn read(owner: &MemoryReadView) {}", {"windows"}))

    def test_import_cannot_apply_insomnia_records_directly(self):
        self.assertTrue(violations(
            "src/imports/reconciliation_interval.rs",
            "output.insomnia.apply_work(work); output.insomnia.apply_attempt(attempt);",
            {"windows"},
        ))

    def test_insomnia_replay_cannot_restore_decode_forwarder(self):
        self.assertTrue(violations(
            "src/owner_lifecycle/insomnia_import_replay.rs",
            "pub(crate) fn decode_record(payload: &[u8]) {}",
            {"windows"},
        ))

    def test_cfg_inactive_and_comments_positive(self):
        self.assertFalse(violations("src/external_source/index.rs", "// GeneralEndpoint\n#[cfg(test)] fn f(p: &GeneralEndpoint) {}", {"windows"}))


if __name__ == "__main__":
    unittest.main()
