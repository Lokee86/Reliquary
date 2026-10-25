#!/usr/bin/env python3
"""Reconcile exact Phase 7 child identifiers without rewriting old receipts."""
from __future__ import annotations

import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
REGISTER = ROOT / 'benchmarks/phase7-indirect-call-corrections-2026-10-24.json'
CHILD = re.compile(r'^P7\.\d\.\d{2}(?:[a-z]|-).+')
IDENTITY_FIELDS = {'ticket', 'child', 'id', 'child_ticket'}


def child_ids(value) -> set[str]:
    result = set()
    if isinstance(value, dict):
        for key, item in value.items():
            if key in IDENTITY_FIELDS and isinstance(item, str) and CHILD.fullmatch(item):
                result.add(item)
            elif isinstance(item, (dict, list)):
                result.update(child_ids(item))
    elif isinstance(value, list):
        for item in value:
            result.update(child_ids(item))
    return result


def validate(register: dict, discovered: set[str]) -> list[str]:
    rows = register['prior_registered_children'] + register['new_children']
    identities = [row['id'] for row in rows]
    expected = set(identities)
    errors = []
    if len(identities) != len(expected):
        errors.append('duplicate registered child identifier')
    if discovered - expected:
        errors.append('unregistered discovered children: ' + ', '.join(sorted(discovered - expected)))
    if expected - discovered:
        errors.append('registered children lack receipts: ' + ', '.join(sorted(expected - discovered)))
    counts = register['denominator_reconciliation']
    required = {'unique_registered_children_found': len(register['prior_registered_children']),
                'children_added': len(register['new_children']),
                'named_children_after': len(expected), 'parent_tickets': 31,
                'tracked_obligations_after': 31 + len(expected), 'gates': 4}
    for key, value in required.items():
        if counts.get(key) != value:
            errors.append(f'incorrect denominator {key}: expected {value}')
    return errors


def main() -> int:
    register = json.loads(REGISTER.read_text(encoding='utf-8'))
    discovered = set()
    for path in sorted((ROOT / 'benchmarks').glob('phase7-*.json')):
        if path != REGISTER:
            discovered.update(child_ids(json.loads(path.read_text(encoding='utf-8'))))
    # New child registrations are authoritative in this register.
    discovered.update(row['id'] for row in register['new_children'])
    errors = validate(register, discovered)
    for row in register['prior_registered_children']:
        receipts = [ROOT / path for path in row['receipts']]
        if not receipts or not all(path.is_file() for path in receipts):
            errors.append(f"missing historical receipt for {row['id']}")
        elif not any(row['id'] in child_ids(json.loads(path.read_text(encoding='utf-8')))
                     for path in receipts):
            errors.append(f"receipt does not register {row['id']}")
    print('\n'.join(errors) if errors else
          f'Phase 7 child registry: 31 parents + {len(discovered)} children = {31 + len(discovered)} obligations; 4 gates')
    return int(bool(errors))


if __name__ == '__main__':
    raise SystemExit(main())
