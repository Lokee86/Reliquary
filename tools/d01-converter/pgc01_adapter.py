"""Strict read-only adapter for the pinned provisional PG-C01 contract."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
from types import ModuleType
from typing import Any


PINNED_COMMIT = "0f824a09fe01f5ff327ab56f3296f36d8f4d7c87"
PINNED_PARENT = "dd77dfa8f7d3c73e3e03cd07b2f4f0f7c9850fae"
PINNED_CODE_SHA256 = "af6763707b5f7918b5295dc0fbcce9a2d55e975abe29f4eedaa00a9584e24245"
PINNED_REGISTRY_HASHES = {
    "semantic_groups": "8ca5594e463de065622ff4f3caccc17f0a61d1a95477f0284a48419f342032b0",
    "record_schemas": "0963412591cc2d202a613584fd458ed14d5888e9e381d5283177eae40b41c79d",
    "physical_carriers": "d7d782f7701a363fb0c4b75c6eabd04200a3889cac370d122d66ea31b0080dc5",
    "contract_bundle": "7a1abda477874b2a1ff78f75520e075f909750d45a17ce3d6528110334f33688",
}
PINNED_CONTRACT_BLOBS = {
    "tools/corpus-evidence-v1/corpus_evidence_v1.py": "68e1a110486d1aa0d88e0642f56224a288f9594c",
    "tools/corpus-evidence-v1/registry/semantic-groups-v1.json": "ee966ee261a5ba5c38d3faf964daf3e6561debc5",
    "tools/corpus-evidence-v1/registry/record-schemas-v1.json": "a9d8787d2ce8578993d1d8dad2ce2625ccecffc7",
    "tools/corpus-evidence-v1/registry/physical-carriers-v1.json": "fdf55b0b27bdfc828cf33872bb37efd751d22f8a",
    "tools/corpus-evidence-v1/fixtures/bundle.sha256": "6e8a6a8eb67ac40107a545d8e8cb76f2333a7ab7",
}
EXPECTED_CARRIER_STATUS = "unclassified-no-authenticated-inventory"


class ContractSnapshotError(RuntimeError):
    """The PG-C01 checkout does not match the explicitly pinned snapshot."""


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _duplicate_rejecting_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ContractSnapshotError("duplicate key in D01 PG-C01 binding")
        value[key] = item
    return value


def _git(repo_root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo_root), *args],
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if result.returncode != 0:
        raise ContractSnapshotError("cannot verify the PG-C01 Git revision")
    return result.stdout.strip()


def _load_binding(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_duplicate_rejecting_object,
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ContractSnapshotError("cannot read the D01 PG-C01 pin") from error
    if not isinstance(value, dict):
        raise ContractSnapshotError("D01 PG-C01 pin must be a JSON object")
    expected = {
        "schema",
        "status",
        "pg_c01_commit",
        "pg_c01_parent",
        "contract_root",
        "contract_code_sha256",
        "registry_hashes",
        "physical_carrier_inventory",
        "synthetic_contract_example",
        "d01_review_record",
    }
    if set(value) != expected:
        raise ContractSnapshotError("D01 PG-C01 pin has unexpected fields")
    if value.get("d01_review_record") != "pgc01-d01-review.provisional.json":
        raise ContractSnapshotError("D01 scoped review record is missing")
    if value["schema"] != "reliquary-d01-pgc01-binding-provisional-v1":
        raise ContractSnapshotError("unsupported D01 PG-C01 pin schema")
    if value["status"] != "provisional-not-accepted":
        raise ContractSnapshotError("PG-C01 is not accepted by this provisional lane")
    if value["pg_c01_commit"] != PINNED_COMMIT or value["pg_c01_parent"] != PINNED_PARENT:
        raise ContractSnapshotError("PG-C01 revision differs from the reviewed provisional pin")
    if value["contract_root"] != "tools/corpus-evidence-v1":
        raise ContractSnapshotError("PG-C01 contract root differs from the pinned layout")
    if value["contract_code_sha256"] != PINNED_CODE_SHA256:
        raise ContractSnapshotError("PG-C01 code hash in the pin differs")
    if value["registry_hashes"] != PINNED_REGISTRY_HASHES:
        raise ContractSnapshotError("PG-C01 registry hashes in the pin differ")
    inventory = value["physical_carrier_inventory"]
    if inventory != {
        "status": EXPECTED_CARRIER_STATUS,
        "authentic_admission": False,
        "entry_count": 0,
        "unclassified_carrier_count": 0,
    }:
        raise ContractSnapshotError("PG-C01 pin must keep authentic admission closed")
    example = value["synthetic_contract_example"]
    if example != {
        "path": "tools/corpus-evidence-v1/fixtures/report-example-pass-synthetic.v1.json",
        "status": "PASS",
        "evidence_scope": "synthetic_fixture",
        "source_admission": "forbidden",
    }:
        raise ContractSnapshotError("synthetic PG-C01 example must remain non-admissible")
    review_path = path.with_name("pgc01-d01-review.provisional.json")
    try:
        review = json.loads(review_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ContractSnapshotError("cannot read the D01 PG-C01 review record") from error
    if (
        review.get("schema") != "reliquary-pgc01-d01-review-provisional-v1"
        or review.get("review_lane") != "D01-T01"
        or review.get("exact_commit") != PINNED_COMMIT
        or review.get("parent_commit") != PINNED_PARENT
        or review.get("outcome") != "D01-reviewed-provisional-input-unmodified"
        or review.get("reviewed_contract_inputs_unchanged") is not True
        or review.get("authentic_admission") is not False
        or review.get("registry_hashes") != PINNED_REGISTRY_HASHES
    ):
        raise ContractSnapshotError("D01 review record differs from the pinned PG-C01 snapshot")
    return value


@dataclass(frozen=True)
class PGC01Snapshot:
    contract_root: Path
    repository_root: Path
    revision: str
    api: ModuleType
    groups: dict[str, Any]
    records: dict[str, Any]
    carriers: dict[str, Any]
    registry_hashes: dict[str, str]

    @property
    def authentic_admission(self) -> bool:
        return False


def load_pinned_snapshot(
    contract_root: Path,
    binding_path: Path | None = None,
) -> PGC01Snapshot:
    """Load only exact, pinned PG-C01 metadata and its validator; never reads source data."""
    root = Path(contract_root).resolve(strict=True)
    if root.name != "corpus-evidence-v1" or root.parent.name != "tools":
        raise ContractSnapshotError("expected the pinned tools/corpus-evidence-v1 layout")
    repo_root = root.parent.parent.resolve(strict=True)
    if Path(_git(root, "rev-parse", "--show-toplevel")).resolve(strict=True) != repo_root:
        raise ContractSnapshotError("PG-C01 root is not inside the expected Git worktree")
    revision = _git(repo_root, "rev-parse", "HEAD")
    if revision != PINNED_COMMIT:
        raise ContractSnapshotError("PG-C01 HEAD differs from the pinned provisional commit")
    if _git(repo_root, "rev-parse", f"{revision}^") != PINNED_PARENT:
        raise ContractSnapshotError("PG-C01 parent differs from the D01-reviewed snapshot")
    changed_paths = _git(
        repo_root, "diff", "--name-only", "--no-renames", PINNED_PARENT, PINNED_COMMIT
    ).splitlines()
    if changed_paths != ["tools/corpus-evidence-v1/tests/test_corpus_evidence_v1.py"]:
        raise ContractSnapshotError("PG-C01 delta differs from the D01-reviewed test-only change")
    contract_diff = subprocess.run(
        [
            "git", "-C", str(repo_root), "diff", "--quiet", PINNED_PARENT, PINNED_COMMIT, "--",
            "tools/corpus-evidence-v1/corpus_evidence_v1.py",
            "tools/corpus-evidence-v1/registry",
            "tools/corpus-evidence-v1/schemas",
            "tools/corpus-evidence-v1/fixtures",
        ],
        check=False,
    )
    if contract_diff.returncode != 0:
        raise ContractSnapshotError("D01-reviewed PG-C01 contract inputs differ from dd77")
    for relative_path, expected_blob in PINNED_CONTRACT_BLOBS.items():
        if _git(repo_root, "rev-parse", f"{PINNED_COMMIT}:{relative_path}") != expected_blob:
            raise ContractSnapshotError("PG-C01 contract blob differs from the D01 review record")

    pin_path = Path(binding_path) if binding_path else Path(__file__).with_name(
        "pgc01-binding.provisional.json"
    )
    _load_binding(pin_path)

    code_path = root / "corpus_evidence_v1.py"
    code = code_path.read_bytes()
    if _sha256(code) != PINNED_CODE_SHA256:
        raise ContractSnapshotError("PG-C01 validator bytes differ from the pinned snapshot")
    committed_code = subprocess.run(
        [
            "git",
            "-C",
            str(repo_root),
            "show",
            f"{PINNED_COMMIT}:tools/corpus-evidence-v1/corpus_evidence_v1.py",
        ],
        check=False,
        capture_output=True,
    )
    if committed_code.returncode != 0 or committed_code.stdout != code:
        raise ContractSnapshotError("PG-C01 validator is not the exact committed file")

    spec = importlib.util.spec_from_file_location(
        "reliquary_pgc01_contract_0f824a09", code_path
    )
    if spec is None or spec.loader is None:
        raise ContractSnapshotError("cannot load the pinned PG-C01 validator")
    api = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(api)
    except Exception as error:
        raise ContractSnapshotError("pinned PG-C01 validator failed to load") from error

    try:
        groups, records, carriers = api.load_registries(root)
        api.validate_registry_bundle(groups, records, carriers)
        hashes = api.registry_hashes(root)
    except Exception as error:
        raise ContractSnapshotError("pinned PG-C01 registry validation failed") from error
    if hashes != PINNED_REGISTRY_HASHES:
        raise ContractSnapshotError("PG-C01 registry bundle differs from the pinned hashes")
    if (
        carriers.get("status") != EXPECTED_CARRIER_STATUS
        or carriers.get("authentic_admission") is not False
        or carriers.get("entries") != []
        or carriers.get("unclassified_carriers") != []
    ):
        raise ContractSnapshotError("PG-C01 physical-carrier map is not empty and fail-closed")
    if any(registry.get("authentic_admission") is not False for registry in (groups, records, carriers)):
        raise ContractSnapshotError("a PG-C01 registry unexpectedly enables authentic admission")
    try:
        api.validate_authentic_admission(groups, records, carriers)
    except api.ContractError:
        pass
    else:
        raise ContractSnapshotError("PG-C01 unexpectedly admits authentic source data")

    return PGC01Snapshot(
        contract_root=root,
        repository_root=repo_root,
        revision=revision,
        api=api,
        groups=groups,
        records=records,
        carriers=carriers,
        registry_hashes=hashes,
    )