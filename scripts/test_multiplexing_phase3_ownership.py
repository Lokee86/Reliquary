"""Negative source-policy fixtures for the P3 hard cut."""
import unittest
from check_multiplexing_phase3_ownership import inventory, production_source

BASE = {
    "runtime_host_gateway.rs": "fn gateway() { ConversationRegistry::new(path); }",
    "runtime_host_gateway_conversation.rs": """
        struct ConversationRegistry {}
        fn mount_rel() { RelJournal::open(path, rel); }
        fn coordinator() { ConversationCoordinator::new(); }
        fn process_reserved() { journal.claim_finalize(key, revision, capture); }
    """,
    "runtime_host_gateway_conversation_continuation.rs": "struct ConversationCoordinator {}",
    "runtime_host_gateway_conversation_finalization.rs": "fn decide() { ConversationPolicy::VisibleAutoBranch; }",
}


class OwnershipTests(unittest.TestCase):
    def violations(self, file, addition):
        sources = dict(BASE)
        sources[file] = sources.get(file, "") + addition
        return inventory(sources)["violations"]

    def test_current_owner_shape_passes(self):
        self.assertEqual(inventory(BASE)["violations"], [])

    def test_duplicate_owner_and_writer_fail(self):
        for addition in ["struct ConversationCoordinator {}", "fn x() { RelJournal::open(path, rel); }",
                         "fn x() { ConversationRegistry::new(path); }"]:
            self.assertTrue(self.violations("runtime_host_gateway_extra.rs", addition))

    def test_alternate_policy_and_finalize_fail(self):
        for addition in ["fn x() { ConversationPolicy::VisibleAutoBranch; }",
                         "fn x() { journal.claim_finalize(key, revision, capture); }"]:
            self.assertTrue(self.violations("runtime_host_gateway_extra.rs", addition))

    def test_production_exact_processing_fails(self):
        for name in ["process_accepted_conversation_submission", "acquire_exact_for_test",
                     "process_conversation_submission", "FreshForTest"]:
            self.assertTrue(self.violations("runtime_host_gateway_extra.rs", "fn x() { " + name + "(); }"))

    def test_only_explicit_test_items_are_exempt(self):
        addition = '#[cfg(test)]\nfn process_accepted_conversation_submission() { acquire_exact_for_test(); }'
        self.assertFalse(self.violations("runtime_host_gateway_extra.rs", addition))
        self.assertTrue(self.violations("runtime_host_gateway_extra.rs", addition + "\nfn x() { acquire_exact_for_test(); }"))

    def test_test_fields_and_variants_preserve_following_production(self):
        text = """struct Owner {
            #[cfg(test)]
            hook: Option<Arc<dyn Fn(A, B) -> Result<(), E>>>,
            coordinator: ConversationCoordinator,
        }
        enum Acquisition { Durable, #[cfg(test)] FreshForTest, }
        fn gateway() {
            let x = Owner {
                #[cfg(test)]
                hook: Mutex::new(None),
                coordinator: ConversationRegistry::new(path),
            };
        }
        """
        masked = production_source(text)
        self.assertNotIn("FreshForTest", masked)
        self.assertNotIn("hook:", masked)
        self.assertIn("ConversationRegistry::new", masked)
        self.assertIn("coordinator: ConversationCoordinator", masked)

    def test_comments_strings_and_test_uses_do_not_become_authority(self):
        text = '''// RelJournal::open(fake)
        fn harmless() { let s = r#"struct ConversationCoordinator { }"#; }
        #[cfg(test)]
        use super::{process_accepted_conversation_submission, ConversationCoordinator};
        #[cfg(test)]
        mod fixture { fn x() { RelJournal::open(fake); } }
        '''
        self.assertFalse(self.violations("runtime_host_gateway_extra.rs", text))
        self.assertIn("fn harmless", production_source(text))


if __name__ == "__main__":
    unittest.main()
