#!/usr/bin/env python3
"""Capture same-source, locked rustdoc/MIR/HIR evidence for the Phase 7 gate.

Generated artifacts stay in the repository. This records evidence; a successful
emission is not architecture acceptance. The checker retains unknown calls.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def snapshot() -> dict:
    files = [{'path': p.relative_to(ROOT).as_posix(), 'sha256': sha(p)}
             for p in sorted((ROOT / 'src').rglob('*.rs'))]
    fingerprint = hashlib.sha256(json.dumps(
        files, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    return {'schema': 'phase7-compiler-source-manifest.v1', 'files': files,
            'fingerprint_sha256': fingerprint}


def policy_snapshot() -> dict:
    paths = ['Cargo.toml', 'Cargo.lock', 'tools/pitlord/production-components.json',
             'tools/pitlord/convergence-component-dag.json']
    return {path: sha(ROOT / path) for path in paths}


def crate_mir_from_cargo_log(log: Path, target: Path) -> Path:
    """Select the MIR matching this command's exact root-crate artifact hash."""
    artifact_files = []
    for line in log.read_text(encoding='utf-8', errors='replace').splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if (message.get('reason') == 'compiler-artifact'
                and message.get('target', {}).get('name') == 'reliquary_memory'
                and 'lib' in message.get('target', {}).get('kind', [])):
            artifact_files.extend(message.get('filenames', []))
    crate_stems = set()
    for name in artifact_files:
        path = Path(name)
        stem = path.stem.removeprefix('lib')
        if path.suffix in ('.rlib', '.rmeta') and re.fullmatch(
                r'reliquary_memory-[0-9a-f]+', stem):
            crate_stems.add(stem)
    candidates = [target / 'debug/deps' / f'{stem}.mir' for stem in crate_stems
                  if (target / 'debug/deps' / f'{stem}.mir').is_file()]
    if len(candidates) != 1:
        raise RuntimeError(f'Cargo log resolves {len(candidates)} crate MIR artifacts: {candidates}')
    return candidates[0]


def run_streamed_stdout(command: list[str], *, cwd: Path, env: dict[str, str],
                        artifact: Path, stderr) -> subprocess.CompletedProcess:
    """Drain large compiler output through a bounded pipe into its artifact."""
    with artifact.open('wb') as output:
        with subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE,
                              stderr=stderr) as process:
            assert process.stdout is not None
            for chunk in iter(lambda: process.stdout.read(1024 * 1024), b''):
                output.write(chunk)
            returncode = process.wait()
    return subprocess.CompletedProcess(command, returncode)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--features', default='')
    parser.add_argument('--name', default='closure')
    parser.add_argument('--reuse-target-dir', type=Path,
                        help='Reuse an isolated target directory created by an earlier emission')
    args = parser.parse_args()
    emitter_path = Path(__file__).resolve()
    emitter_sha_before = sha(emitter_path)
    if not args.name or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in args.name):
        parser.error('--name must be a simple lowercase artifact label')
    output = ROOT / '.verification-tmp' / 'phase7' / args.name
    output.mkdir(parents=True, exist_ok=True)
    target = (ROOT / args.reuse_target_dir).resolve() if args.reuse_target_dir else output / 'target'
    if args.reuse_target_dir and not target.is_dir():
        parser.error(f'isolated target directory does not exist: {target}')
    try:
        target.relative_to(ROOT)
    except ValueError:
        parser.error('compiler target directory must stay inside this repository')
    if not args.reuse_target_dir and target.exists():
        parser.error(f'fresh isolated target directory already exists: {target}')
    temporary = ROOT / '.t'
    temporary.mkdir(exist_ok=True)
    environment = os.environ.copy()
    environment.update(RUSTC_BOOTSTRAP='1', CARGO_INCREMENTAL='0',
                       CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0',
                       TEMP=str(temporary), TMP=str(temporary),
                       CARGO_TARGET_DIR=str(target))
    features = sorted(filter(None, args.features.split(',')))
    common = ['--lib', '--locked', '--jobs', '1']
    common += ['--message-format=json-render-diagnostics']
    if features:
        common += ['--features', ','.join(features)]
    before = snapshot()
    policy_before = policy_snapshot()
    manifest = output / 'source-manifest.json'
    manifest.write_text(json.dumps(before, indent=2) + '\n', encoding='utf-8')
    built_mir_dir = output / 'built-mir'
    if built_mir_dir.exists():
        raise RuntimeError(f'built MIR dump directory already exists: {built_mir_dir}')
    built_mir_dir.mkdir()
    commands = [
        ('rustdoc', ['cargo', 'rustdoc', *common, '--', '--document-private-items',
                     '-Z', 'unstable-options', '--output-format', 'json']),
        ('mir', ['cargo', 'rustc', *common, '--', '--emit=dep-info,metadata,link,mir',
                 '-Zmir-include-spans=yes', '-Zverbose-internals=yes',
                 '-Zdump-mir=built', f'-Zdump-mir-dir={built_mir_dir.resolve()}']),
        ('hir', ['cargo', 'rustc', *common, '--', '-Zunpretty=hir-tree']),
        ('thir', ['cargo', 'rustc', *common, '--', '-Zunpretty=thir-tree']),
    ]
    started = time.monotonic()
    results = []
    for name, command in commands:
        print('Emitting ' + name, flush=True)
        log = output / (name + '.log')
        artifact = output / ('owners.hir' if name == 'hir' else 'calls.thir')
        with log.open('wb') as errors:
            if name in ('hir', 'thir'):
                result = run_streamed_stdout(command, cwd=ROOT, env=environment,
                                             artifact=artifact, stderr=errors)
            else:
                result = subprocess.run(command, cwd=ROOT, env=environment,
                                        stdout=errors, stderr=subprocess.STDOUT)
        results.append({'name': name, 'command': command, 'exit': result.returncode,
                        'log': log.relative_to(ROOT).as_posix(), 'log_sha256': sha(log)})
        if result.returncode:
            print(f'{name} failed; see {log}', flush=True)
            return result.returncode
    after = snapshot()
    doc = output / 'rustdoc.json'
    mir = output / 'crate.mir'
    doc_source = target / 'doc/reliquary_memory.json'
    mir_source = crate_mir_from_cargo_log(output / 'mir.log', target)
    doc.write_bytes(doc_source.read_bytes())
    mir.write_bytes(mir_source.read_bytes())
    built_mir_files = sorted(built_mir_dir.rglob('*.built.after.mir'),
                             key=lambda path: path.relative_to(ROOT).as_posix())
    if not built_mir_files:
        raise RuntimeError(f'rustc emitted no built.after MIR files under {built_mir_dir}')
    built_mir_manifest = output / 'built-mir-manifest.json'
    built_mir_manifest.write_text(json.dumps({
        'schema': 'phase7-built-mir-manifest.v1',
        'directory': built_mir_dir.relative_to(ROOT).as_posix(),
        'files': [{'path': path.relative_to(ROOT).as_posix(), 'sha256': sha(path)}
                  for path in built_mir_files],
    }, indent=2) + '\n', encoding='utf-8')
    receipt = {
        'schema': 'phase7-compiler-provenance.v1',
        'source_snapshot': {'before_sha256': before['fingerprint_sha256'],
                            'after_sha256': after['fingerprint_sha256'],
                            'file_count': len(before['files']),
                            'manifest_path': manifest.relative_to(ROOT).as_posix(),
                            'manifest_sha256': sha(manifest)},
        'artifacts': {name: {'path': str(path), 'sha256': sha(path)}
                      for name, path in [('rustdoc', doc), ('mir', mir),
                                         ('hir', output / 'owners.hir'),
                                         ('thir', output / 'calls.thir'),
                                         ('built_mir_manifest', built_mir_manifest)]},
        'commands': results,
        'policy_hashes': policy_before,
        'policy_hashes_after': policy_snapshot(),
        'environment': {k: environment.get(k) for k in [
            'RUSTC_BOOTSTRAP', 'CARGO_INCREMENTAL', 'CARGO_PROFILE_DEV_DEBUG',
            'CARGO_PROFILE_TEST_DEBUG', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS',
            'RUSTC_WORKSPACE_WRAPPER', 'RUSTC_WRAPPER', 'CARGO_BUILD_TARGET']},
        'build_profile': 'dev',
        'rustc_version': subprocess.check_output(['rustc', '-Vv'], text=True),
        'cfg': sorted(subprocess.check_output(['rustc', '--print', 'cfg'], text=True)
                      .splitlines() + [f'feature="{feature}"' for feature in features]),
        'features': features, 'elapsed_seconds': time.monotonic() - started,
        'target_directory': target.relative_to(ROOT).as_posix(),
        'selected_mir_artifact': mir_source.relative_to(ROOT).as_posix(),
        'tools': {
            'emitter': {'path': 'scripts/emit_phase7_compiler_evidence.py',
                        'sha256_before': emitter_sha_before,
                        'sha256_after': sha(emitter_path)},
        },
    }
    provenance = output / 'provenance.json'
    provenance.write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
    tools_stable = receipt['tools']['emitter']['sha256_before'] == \
        receipt['tools']['emitter']['sha256_after']
    if before != after or policy_before != receipt['policy_hashes_after'] or not tools_stable:
        print('Source, compiler policy or evidence tool changed during emission; evidence is invalid.', flush=True)
        return 1
    return subprocess.call([sys.executable, str(ROOT / 'scripts/check_phase7_compiler_calls.py'),
                            '--rustdoc', str(doc), '--mir', str(mir),
                            '--hir', str(output / 'owners.hir'), '--thir', str(output / 'calls.thir'),
                            '--built-mir-manifest', str(built_mir_manifest),
                            '--provenance', str(provenance), '--output', str(output / 'calls.json')], cwd=ROOT)


if __name__ == '__main__':
    raise SystemExit(main())
