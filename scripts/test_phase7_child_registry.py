import copy
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_phase7_child_registry import child_ids, validate


class ChildRegistryTests(unittest.TestCase):
    def setUp(self):
        self.register = {'prior_registered_children': [{'id': 'P7.1.12a-OLD'}],
                         'new_children': [{'id': 'P7.1.09b-NEW'}],
                         'denominator_reconciliation': {
                             'unique_registered_children_found': 1, 'children_added': 1,
                             'named_children_after': 2, 'parent_tickets': 31,
                             'tracked_obligations_after': 33, 'gates': 4}}
        self.ids = {'P7.1.12a-OLD', 'P7.1.09b-NEW'}

    def test_identifiers_deduplicate_addenda_and_ignore_parents_witnesses(self):
        self.assertEqual(child_ids({'ticket': 'P7.1.12', 'child': 'P7.1.12a-OLD',
                                    'rows': [{'id': 'P7.1.12a-OLD'}, {'id': 'RG-PUB-01'}]}),
                         {'P7.1.12a-OLD'})

    def test_complete_registry(self):
        self.assertFalse(validate(self.register, self.ids))

    def test_omitted_receipt_is_not_silently_accepted(self):
        errors = validate(self.register, self.ids | {'P7.1.14-OMITTED'})
        self.assertTrue(any('unregistered' in error for error in errors))

    def test_historical_thirteen_count_cannot_override_registered_ids(self):
        register = copy.deepcopy(self.register)
        register['denominator_reconciliation']['named_children_after'] = 13
        self.assertTrue(validate(register, self.ids))

    def test_phase7_current_denominator_reconciles_to_65(self):
        register = {
            'prior_registered_children': [{'id': f'P7.1.12a-OLD-{i}'} for i in range(27)],
            'new_children': [{'id': f'P7.1.14g-NEW-{i}'} for i in range(7)],
            'denominator_reconciliation': {
                'unique_registered_children_found': 27, 'children_added': 7,
                'named_children_after': 34, 'parent_tickets': 31,
                'tracked_obligations_after': 65, 'gates': 4,
            },
        }
        discovered = {row['id'] for row in register['prior_registered_children']}
        discovered.update(row['id'] for row in register['new_children'])
        self.assertFalse(validate(register, discovered))
        self.assertEqual(31 + len(discovered), 65)

    def test_duplicate_child_is_not_a_new_obligation(self):
        self.register['new_children'].append({'id': 'P7.1.12a-OLD'})
        self.assertTrue(any('duplicate' in error for error in validate(self.register, self.ids)))


if __name__ == '__main__':
    unittest.main()
