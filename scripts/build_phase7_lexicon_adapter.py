#!/usr/bin/env python3
"""Reproduce the bounded local Phase 7 analyzer compatibility build."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[1]


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    sibling = ROOT.parent / 'lexicon-arcana'
    original = sibling / 'lexicon/adapters/rust'
    patch = ROOT / 'tools/pitlord/lexicon-toolstate-exclusion.patch'
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=sibling, text=True).strip()
    source_status = subprocess.check_output(
        ['git', 'status', '--porcelain', '--', 'lexicon/adapters/rust'], cwd=sibling, text=True,
    ).strip()
    if source_status:
        raise RuntimeError('Adapter source differs from its recorded sibling revision: ' + source_status)
    source_files = {
        p.relative_to(original).as_posix(): sha(p)
        for p in sorted((original / 'src').rglob('*.rs'))
    }
    builder_sha = sha(Path(__file__))
    output = ROOT / '.verification-tmp/phase7'
    copy = ROOT / 'target' / ('phase7-adapter-source-' + uuid.uuid4().hex[:12])
    copy.parent.mkdir(exist_ok=True)
    shutil.copytree(original, copy, ignore=shutil.ignore_patterns('target', '.git'))
    environment = os.environ.copy()
    temporary = ROOT / '.t'
    temporary.mkdir(exist_ok=True)
    environment.update(TEMP=str(temporary), TMP=str(temporary), CARGO_INCREMENTAL='0')
    commands = []

    def run(command: list[str]) -> None:
        commands.append(command)
        subprocess.run(command, cwd=ROOT, env=environment, check=True)

    relative = copy.relative_to(ROOT).as_posix()
    run(['git', 'apply', '--ignore-whitespace', '--check', '--directory=' + relative, str(patch)])
    run(['git', 'apply', '--ignore-whitespace', '--directory=' + relative, str(patch)])
    common = ['--manifest-path', str(copy / 'Cargo.toml'), '--target-dir', str(output / 'adapter-target'), '--locked']
    for fixture in ['parser::manifest_tests', 'orchestrator::production_manifest_tests', 'paths::tests']:
        run(['cargo', 'test', *common, fixture, '--', '--test-threads=1'])
    run(['cargo', 'build', '--release', *common])
    built = output / 'adapter-target/release/lexicon-rust-adapter.exe'
    destination = output / 'adapters/rust/lexicon-rust.exe'
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(built, destination)
    receipt = {
        'schema': 'phase7-local-lexicon-adapter-build.v2',
        'source_repository': str(sibling), 'source_revision': revision,
        'package': 'lexicon-rust-adapter', 'version': '0.5.0',
        'command': 'python scripts/build_phase7_lexicon_adapter.py', 'commands': commands,
        'manifest_sha256': sha(original / 'Cargo.toml'),
        'cargo_lock_sha256': sha(original / 'Cargo.lock'),
        'patch_path': patch.relative_to(ROOT).as_posix(), 'patch_sha256': sha(patch),
        'binary_path': destination.relative_to(ROOT).as_posix(), 'binary_sha256': sha(destination),
        'source_manifest_protocol': 'reliquary.production-source-manifest.v1',
        'fixture': 'pass: parser path/hash negatives; synthetic Cargo root and real/mirror Container::create; generated-state exclusion',
        'build_exit': 0,
        'source_files_before_patch': source_files,
        'builder_sha256': builder_sha,
        'temporary_source_copy': relative,
    }
    (output / 'lexicon-adapter-local-build-current.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print('Built and verified local analyzer adapter: ' + receipt['binary_sha256'])
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
