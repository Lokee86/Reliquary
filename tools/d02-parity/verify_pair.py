"""Independently verify one pinned pair of converted REL and PHY owners."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

import compare_archive_bindings as archive_bindings
import compare_authentic_archive as archive
import compare_authentic_freshness as freshness
import compare_authentic_graph as graph
import compare_authentic_memory as memory
from authentic_witness import read_source, require
from compare_clocked_prefix import compare, compare_archive_backing
from pair_samples import freeze, verify_samples


D01 = Path(__file__).resolve().parents[1] / "d01-converter"
COVERAGE = "witness/source-coverage.json"
MANIFEST = "witness/source-witness-manifest.json"
PREFIX = "independent/prefix-availability.json"
VECTORS = "independent/vector-cuts.json"
FRESHNESS = "transfers/freshness-source-transfer-v1/freshness-source-transfer.json"
MEMORY_TRANSFER = "transfers/memory-transfer-v1/memory-source-transfer.json"
PROJECT_BACKING = "independent/project-backing.json"
FIRST28D = "chatgpt-first28d"


def load(path):
    return json.loads(path.read_bytes())


def digest(path):
    return archive_bindings.digest_file(path)


def verify_artifact(path, expected):
    require(digest(path) == expected, f"pinned input changed at {path.name}")
    return path


def pair_pins(pair):
    from f03_verify_archived_oracle import PINS

    paths = [f"{pair}/project.prj.rel", f"{pair}/user.phy"]
    require(all(path in PINS for path in paths), "pair is outside the pinned historical sources")
    return {path: PINS[path] for path in paths}


def verify_source_copies(args, source_pins):
    for relative, pin in source_pins.items():
        original = verify_artifact(args.originals / relative, pin)
        copied = verify_artifact(args.copies / relative, pin)
        require(not original.samefile(copied), "original and disposable input name the same file")


def run_json(command, diagnostic, *, output=True):
    result = subprocess.run([str(item) for item in command], capture_output=True, timeout=900)
    if result.returncode:
        with diagnostic.open("xb") as file:
            file.write(result.stdout + result.stderr)
        raise ValueError(f"{diagnostic.stem} failed with exit {result.returncode}")
    return json.loads(result.stdout) if output else None


def native_generation(source, owner):
    require(source["native"]["history_generation"] == "0" * 32
            and source["native"]["rewrite_genesis"] is None,
            "source history requires a lineage mapping")
    kind = {"REL": 1, "PHY": 2}[owner["kind"]]
    scope = {"unspecified": 0, "organization": 1, "project": 2, "connection": 3}[owner["scope"]]
    return hashlib.sha256(b"reliquary-history-genesis-v1\0" + bytes.fromhex(owner["owner_uuid"])
                          + bytes([kind, scope])).hexdigest()


def check_owner(args, pins, relative, pin):
    coverage = args.evidence / COVERAGE
    source, owner, artifacts = read_source(coverage, pins[COVERAGE], pins[MANIFEST], pin)
    label = owner["kind"].lower()
    source_copy = args.copies / relative
    candidate = args.candidates / ("project.rel" if label == "rel" else "user.phy")
    composer = load(args.candidates / f"{label}-compose.json")
    writer = load(args.candidates / f"{label}-writer.json")
    require(composer["status"] == writer["status"] == "PASS"
            and composer["source_sha256"] == writer["source_sha256"] == pin
            and composer["source_coverage_sha256"] == pins[COVERAGE]
            and composer["source_frames"] == writer["source_frames"] == source["frame_stream"]["rows"]
            and composer["transfer_sha256"] == writer["transfer_sha256"],
            "writer custody does not identify the frozen owner and transfer")
    candidate_sha = writer["candidate_sha256"]
    verify_artifact(candidate, candidate_sha)
    native = run_json([args.cli, label, "verify", candidate], args.candidates / f"{label}-cli.log")
    require(native["verified"] and native["owner"]["owner_id"] == owner["owner_uuid"],
            "current CLI verified a different owner")
    legacy = subprocess.run([str(args.cli), label, "verify", str(source_copy)], capture_output=True, timeout=300)
    require(legacy.returncode != 0 and json.loads(legacy.stderr)["error"]["kind"] == "wrong_format",
            "normal 2.1 CLI did not reject the legacy format")
    print(f"{label.upper()} native CLI verification passed", file=sys.stderr, flush=True)

    common = (args.reader, candidate, candidate_sha, coverage, pins[COVERAGE], pins[MANIFEST], pin)
    pair = {row["kind"]: row for row in load(args.custody)["sources"]}
    mapping = (args.evidence / MEMORY_TRANSFER, pins[MEMORY_TRANSFER], pair["REL"], owner["kind"])
    checks = {"memory": memory.compare(*common, paired=mapping), "graph": graph.compare(*common)}
    generation = checks["memory"]["target_history_generation_sha256"]
    require(generation == native_generation(source, owner), "native history generation differs")
    checks["clocked_prefix"], archive_frontiers = compare(
        source_copy, source, owner, coverage.parent, artifacts, candidate, candidate_sha,
    )
    direct_args = [
        "--candidate", candidate, "--candidate-sha256", candidate_sha,
        "--source-copy", source_copy, "--source-sha256", pin,
    ]
    for name, extra in (
        ("journals", ["--paired-phy-owner", pair["PHY"]["owner_uuid"]]),
        ("vectors", ["--expectations", args.evidence / VECTORS, "--expectations-sha256", pins[VECTORS]]),
    ):
        checks[name] = run_json(
            [sys.executable, "-B", D01 / f"verify_candidate_{name}.py", *direct_args, *extra],
            args.candidates / f"{label}-{name}.log",
        )
    with tempfile.TemporaryDirectory(dir=args.candidates, prefix=".d02-entity-") as stage:
        entity_receipt = Path(stage) / "observation.json"
        run_json([
            sys.executable, "-B", args.source_tools / "f03_compare_current_entity_community.py",
            "--source-coverage", coverage, "--verified-conversion-inputs", args.custody,
            "--candidate", candidate, "--receipt", entity_receipt, "--source-sha256", pin,
            "--target-sha256", candidate_sha,
        ], args.candidates / f"{label}-entity-community.log", output=False)
        checks["entity_resolution_community"] = load(entity_receipt)
    if label == "rel":
        checks["archive"] = archive.compare(*common)
        binding = f"independent/archive-bindings/{source['copy_id']}-archive-binding-expectation.json"
        checks["archive_bindings"] = archive_bindings.compare(
            args.reader, candidate, candidate_sha, (args.evidence / binding).read_bytes(), pins[binding],
        )
        availability = load(args.evidence / PREFIX)
        require(availability["status"] == "PASS"
                and availability["source_coverage_sha256"] == pins[COVERAGE]
                and availability["source_manifest_sha256"] == pins[MANIFEST],
                "Archive availability was frozen for different source evidence")
        availability_owner, = [row for row in availability["owners"] if row["source_sha256"] == pin]
        checks["archive_backing"] = compare_archive_backing(
            args.evidence / "independent/prefix-availability", availability_owner,
            checks["archive"].pop("project_backing"), archive_frontiers,
        )
        checks["freshness"] = freshness.compare(
            args.reader, candidate, candidate_sha, args.evidence / FRESHNESS, pins[FRESHNESS], pin,
        )
    require(all(result["status"] == "PASS" for result in checks.values()),
            "one or more retained history checks remain incomplete")
    print(f"{label.upper()} independent history comparisons passed", file=sys.stderr, flush=True)
    return {
        "owner": owner, "source_scope": source["header_observation"]["native_scope_observed_from_header"],
        "source_sha256": pin, "candidate_sha256": candidate_sha, "transfer_sha256": writer["transfer_sha256"],
        "source_frames": writer["source_frames"], "target_frames": writer["target_frames"],
        "source_history_generation": source["native"]["history_generation"], "native_history_generation": generation,
        "current_cli_verify": True, "current_cli_rejects_original_format": True, "checks": checks,
    }


def verify(args):
    pair = getattr(args, "pair", FIRST28D)
    source_pins = pair_pins(pair)
    frozen_sha = digest(args.frozen)
    frozen = load(args.frozen)
    require(frozen.get("pair", FIRST28D) == pair, "the frozen expectations belong to a different pair")
    pins = frozen["inputs"]

    def verify_sources():
        verify_source_copies(args, source_pins)
        for name, pin in pins.items():
            verify_artifact(args.evidence / name, pin)
        verify_artifact(args.custody, frozen["custody_sha256"])
        verify_artifact(args.frozen, frozen_sha)

    verify_sources()
    tools = {name: digest(getattr(args, name)) for name in ("cli", "reader", "writer")}
    owners = {}
    for relative, pin in source_pins.items():
        record = check_owner(args, pins, relative, pin)
        owners[record["owner"]["kind"]] = record
    require(set(owners) == {"REL", "PHY"}, "expected one verified REL and PHY")
    samples = verify_samples(args.cli, args.candidates, frozen["samples"])
    tools["grant_check"] = digest(args.grant_check)
    tested = subprocess.run(
        [str(args.grant_check), "--exact", "migrated_pair_preserves_private_memory_grants", "--ignored"],
        env={**os.environ, "RELIQUARY_MIGRATION_REL": str(args.candidates / "project.rel"),
             "RELIQUARY_MIGRATION_PHY": str(args.candidates / "user.phy"),
             "RELIQUARY_MIGRATION_MEMORY": frozen["samples"]["history"]["PHY"]["memory_id"]},
        capture_output=True, timeout=420,
    )
    require(tested.returncode == 0 and b"1 passed" in tested.stdout,
            "native paired and ungranted PHY reads did not pass")
    samples["phy_grant_boundary"] = "PASS"
    verify_sources()
    for name, row in owners.items():
        verify_artifact(args.candidates / ("project.rel" if name == "REL" else "user.phy"),
                        row["candidate_sha256"])
    for name, pin in tools.items():
        verify_artifact(getattr(args, name), pin)
    report = {
        "schema_id": "reliquary-owner-history-pair-verification-v1", "pair": pair, "status": "PASS",
        "frozen_expectation_sha256": frozen_sha, "source_evidence_sha256": pins,
        "tools_sha256": tools, "owners": owners, "native_reads": samples,
        "originals_copies_and_candidates_unchanged": True,
    }
    destination = args.candidates / "pair-verification.json"
    write_new(destination, report)


def write_new(destination, report):
    with tempfile.TemporaryDirectory(dir=destination.parent, prefix=".d02-pair-") as stage:
        staged = Path(stage) / destination.name
        with staged.open("x", encoding="utf-8", newline="\n") as file:
            json.dump(report, file, indent=2, sort_keys=True)
            file.write("\n")
            file.flush()
            os.fsync(file.fileno())
        os.link(staged, destination)


def freeze_inputs(args):
    pair = getattr(args, "pair", FIRST28D)
    source_pins = pair_pins(pair)
    verify_source_copies(args, source_pins)
    coverage = args.evidence / COVERAGE
    source_rows = load(coverage)["sources"]
    require(sorted(row["source_sha256"] for row in source_rows) == sorted(source_pins.values()),
            "the source witness does not identify exactly the selected pair")
    pins = {name: digest(args.evidence / name) for name in
            (COVERAGE, MANIFEST, PREFIX, VECTORS, FRESHNESS, MEMORY_TRANSFER)}
    custody = load(args.custody)
    require(custody["status"] == "VERIFIED_DISPOSABLE_COPY_INPUTS"
            and custody["source_coverage_sha256"] == pins[COVERAGE]
            and custody["source_manifest_sha256"] == pins[MANIFEST]
            and len(custody["sources"]) == len(source_pins)
            and all(row["source_sha256"] == row["verified_disposable_sha256"] == source_pins.get(row["fixture"])
                    for row in custody["sources"]),
            "the disposable-copy receipt does not match the selected pair")
    owners = {}
    for pin in source_pins.values():
        source, owner, artifacts = read_source(coverage, pins[COVERAGE], pins[MANIFEST], pin)
        require(any(row["source_sha256"] == pin and row["kind"] == owner["kind"]
                    and row["owner_uuid"] == owner["owner_uuid"] and row["scope"] == owner["scope"]
                    for row in custody["sources"]), "disposable-copy identity differs from the witness")
        owners[owner["kind"]] = source
    binding = f"independent/archive-bindings/{owners['REL']['copy_id']}-archive-binding-expectation.json"
    pins[binding] = digest(args.evidence / binding)
    reports = [PREFIX, VECTORS]
    if int(owners["REL"]["physical_carrier_counts"].get("PRJFILE1", 0)):
        pins[PROJECT_BACKING] = digest(args.evidence / PROJECT_BACKING)
        reports.append(PROJECT_BACKING)
    require(all(load(args.evidence / name)["status"] == "PASS" for name in reports),
            "independent source evidence is incomplete")
    samples = freeze(args.evidence, args.copies / pair / "project.prj.rel", owners, artifacts)
    write_new(args.frozen, {"pair": pair, "inputs": pins, "custody_sha256": digest(args.custody), "samples": samples})


def main(default_pair=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--freeze", action="store_true")
    parser.add_argument("--pair", required=default_pair is None, default=default_pair,
                        help="pinned source directory containing the REL and PHY")
    for name in ("source-tools", "evidence", "custody", "originals", "copies", "frozen"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    for name in ("candidates", "cli", "reader", "writer", "grant-check"):
        parser.add_argument(f"--{name}", type=Path)
    args = parser.parse_args()
    if not args.freeze and any(getattr(args, name) is None for name in
                               ("candidates", "cli", "reader", "writer", "grant_check")):
        parser.error("verification requires --candidates, --cli, --reader, --writer, and --grant-check")
    sys.path.insert(0, str(args.source_tools))
    try:
        (freeze_inputs if args.freeze else verify)(args)
    except (ValueError, OSError, KeyError, TypeError, IndexError, subprocess.SubprocessError) as error:
        print(f"Pair verification incomplete. {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
