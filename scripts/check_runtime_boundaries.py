#!/usr/bin/env python3
"""Bounded Phase 5 source cuts; cfg-aware lexical evidence, not a symbol graph."""
from __future__ import annotations
import argparse
import json
import re
from pathlib import Path
from check_owner_boundaries import strip_inactive_cfg
from check_multiplexing_phase4_coverage import masked

ROOT = Path(__file__).resolve().parents[1]
FACADE_FREE = {
    'src/runtime_inference/insomnia/evidence.rs',
    'src/runtime_inference/insomnia/extraction.rs',
    'src/runtime_inference/insomnia/processor/result.rs',
    'src/runtime_inference/insomnia/worker/model.rs',
    'src/runtime_inference/dream_canonical.rs',
    'src/runtime_inference/dream_processor.rs',
    'src/runtime_inference/dream_community_naming.rs',
    'src/runtime_inference/dream_settlement.rs',
    'src/runtime_inference/insomnia/processor/application.rs',
    'src/retrieval/lexical_contract.rs',
    'src/config_security/model_config.rs',
    'src/config_security/model_config_codec.rs',
}
CONFIG_FREE = {
    'src/config_security/config.rs',
    'src/config_security/credential.rs',
    'src/config_security/model_config.rs',
    'src/config_security/model_config_codec.rs',
    'src/config_security/runtime_config.rs',
}
UPPER = {'runtime-gateway', 'public-consumers'}


def violations(path: str, source: str, component: str | None, facts: set[str]) -> list[str]:
    view = masked(strip_inactive_cfg(source, facts))
    rules = []
    if path in FACADE_FREE:
        rules.append(('facade callback', r'(?<!Self::)\bCva\b|(?<!Self::)(?<!InsomniaProcessError::)\bPhylactery\b(?!\s*\(\s*String\s*\))'))
    if path in CONFIG_FREE:
        rules.append(('provider service in configuration', r'\b(?:model_switchboard|model_switchboard_codec|ModelSwitchboard|dream_processor_frontier)\b'))
    if (component is not None and component not in UPPER) or path in FACADE_FREE:
        rules.append(('lower owner calls runtime implementation', r'\b(?:GatewayRuntime|InstanceRuntime|runtime_host_gateway|runtime_host_instances)\b'))
    return [f'{path}:{view.count(chr(10), 0, match.start()) + 1}: {label}: {match.group()}'
            for label, pattern in rules for match in re.finditer(pattern, view)]


def self_test() -> None:
    path = 'src/runtime_inference/insomnia/evidence.rs'
    for bad in ('use crate::Cva;', 'fn read(owner: &mut crate::facade::Cva) {}',
                'use crate::Cva as Owner;', 'use crate::{Cva, Archive};',
                'fn read(gateway: &crate::GatewayRuntime) {}'):
        assert violations(path, bad, 'insomnia', {'windows'}), bad
    assert not violations(path, '// Cva\nfn f() { let text = "Cva"; }', 'insomnia', {'windows'})
    assert not violations(path, '#[cfg(test)] fn f(cva: &mut Cva) {}', 'insomnia', {'windows'})
    assert violations(path, '#[cfg(windows)] fn f(cva: &mut Cva) {}', 'insomnia', {'windows'})
    assert not violations(path, '#[cfg(unix)] fn f(cva: &mut Cva) {}', 'insomnia', {'windows'})
    assert violations('src/config_security/config.rs', 'use crate::model_switchboard::validate_switchboard;', 'config-security', {'windows'})
    assert violations('src/memory/example.rs', 'use crate::runtime_host_gateway;', 'memory-store', {'windows'})
    print('Runtime boundary negative/positive fixtures passed (11 cases).')


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
    mapping = json.loads((ROOT / 'tools/pitlord/production-components.json').read_text(encoding='utf-8'))
    facts = set(mapping['cfg_profile']['rustc_cfg_facts'])
    files = {row['path']: row.get('component') for row in mapping['files'] if row.get('classification') == 'production'}
    for path in FACADE_FREE | CONFIG_FREE:
        files.setdefault(path, None)
    errors = []
    checked = 0
    for path, component in sorted(files.items()):
        source = ROOT / path
        if not source.is_file():
            if path in FACADE_FREE | CONFIG_FREE:
                errors.append(f'{path}: required boundary source missing')
            continue
        if (component is not None and component not in UPPER) or path in FACADE_FREE | CONFIG_FREE:
            checked += 1
            errors.extend(violations(path, source.read_text(encoding='utf-8'), component, facts))
    for error in errors:
        print(error)
    print(f'Runtime ownership source guard: {checked} files, {len(errors)} violations; bounded lexical proof only.')
    return int(bool(errors))


if __name__ == '__main__':
    raise SystemExit(main())
