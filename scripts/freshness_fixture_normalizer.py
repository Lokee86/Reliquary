"""Disposable N5 fixture staging/verification/promotion. Delete at N10.

N6/N7 perform owner work on the fixed candidates; this tool never patches records.
Multi-file promotion is failure-atomic, with explicit recovery after process death.
"""
from __future__ import annotations
import argparse
from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[1]
AUTHORITATIVE = Path("C:/!bin/workspace/reliquary-fixtures/local/authoritative")
NAMES = ("project.prj.rel", "user.phy", "AUTHORITATIVE.txt", "SHA256SUMS.txt")


@dataclass(frozen=True)
class Fixture:
    name: str
    directory: Path
    rel_hash: str
    phy_hash: str
    rel_owner: str
    phy_owner: str


FIXTURES = {
    name: Fixture(name, AUTHORITATIVE / name, *facts)
    for name, facts in {
        "chatgpt-first14d": (
            "49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b",
            "344f1cba2a759a339fe8959135a075210851f236cc9652246ac924320a92c12c",
            "73eff05f-e65e-4fb4-b430-f9af2237b48b", "ce8c8fc9-640d-4e76-abd8-dab60424763b"),
        "chatgpt-first28d": (
            "8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4",
            "a095352cfac556e04cb2ad6f3258ced5aacecbf88429a6deb36c7659dfc715cd",
            "73eff05f-e65e-4fb4-b430-f9af2237b48b", "ce8c8fc9-640d-4e76-abd8-dab60424763b"),
        "rhelm-david-r-ellis-pre-relationship": (
            "e53255ff5258aaeb5fc4437e5993613868adf646d9e5f711fe2b079f7a340fd4",
            "110301f1f12873b8ac1a2eb5000a004fdcd0287d7e141622e6cd18e7b2afe937",
            "41fd7fac-75d5-4d05-899c-0dce0823aa36", "4ad56e4a-ecc4-468f-8742-eaa343efd9e7"),
    }.items()
}


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def canonical(value) -> bytes:
    return (json.dumps(value, sort_keys=True, indent=2, ensure_ascii=False) + "\n").encode("utf-8")


def write_json(path: Path, value):
    temporary = path.with_name(path.name + ".writing")
    with temporary.open("wb") as output:
        output.write(canonical(value))
        output.flush()
        os.fsync(output.fileno())
    os.replace(temporary, path)


def regular(path: Path):
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
        raise RuntimeError(f"not a private regular file: {path}")
    for ancestor in (path, *path.parents):
        if ancestor.is_symlink() or getattr(ancestor.lstat(), "st_file_attributes", 0) & 0x400:
            raise RuntimeError(f"link/reparse path refused: {path}")


def identity(path: Path, kind: int, owner: str):
    regular(path)
    with path.open("rb") as source:
        header = source.read(40)
    if (len(header) != 40 or header[:8] != b"CVA\x00\r\n\x1a\n"
            or header[8:16] != b"\x01\x00\x00\x00\x28\x00\x00\x00"
            or header[16] != kind or header[18:24] != bytes(6)
            or header[24:40] != uuid.UUID(owner).bytes):
        raise RuntimeError(f"typed owner identity mismatch: {path}")


def preserved(before, after):
    """A candidate may add owner work, never erase original authority."""
    for scope in ("rel", "phy"):
        old, new = before[scope], after[scope]
        if old["owner"] != new["owner"]:
            raise RuntimeError(f"{scope} owner changed")
        for key, value in old["memories"].items():
            candidate = new["memories"].get(key, {})
            if any(candidate.get(k) != value[k] for k in ("body", "provenance")):
                raise RuntimeError(f"{scope} Memory body/provenance changed: {key}")
        for key, value in old["history"].items():
            if new["history"].get(key) != value:
                raise RuntimeError(f"{scope} original revision changed: {key}")
        if not set(old["graph"]).issubset(new["graph"]):
            raise RuntimeError(f"{scope} retained graph authority removed")
    old, new = before["rel"], after["rel"]
    for key in ("activity_count", "nodes", "branches", "accepted_use"):
        if old[key] != new[key]:
            raise RuntimeError(f"retained {key} changed")
    for key in ("episodes", "fragments"):
        if not set(old[key]).issubset(new[key]):
            raise RuntimeError(f"retained {key} changed")
    for key, value in old["actual_dream"].items():
        if new["actual_dream"].get(key) != value:
            raise RuntimeError(f"actual Dream proof changed: {key}")
    existing = set(old["memories"])
    derived_births = (set(new["publication_births"]) - set(old["publication_births"])) & existing
    if not derived_births.issubset(old["publication"]["proven"]):
        raise RuntimeError("legacy birth lacks unique retained publication proof")
    new_actual = (set(new["actual_dream"]) - set(old["actual_dream"])) & existing
    if not new_actual.issubset(old["publication_births"]):
        raise RuntimeError("legacy conversion cannot claim a newly fabricated actual first-cycle Dream")
    if not new["archive_clock_valid"] or not new["preflight"]["ready"]:
        raise RuntimeError(f"candidate H1 preflight failed: {new['preflight']}")
    if not new["vectors"]["ready"]:
        raise RuntimeError(f"candidate fragment/target vector coverage is incomplete: {new['vectors']}")


class Normalizer:
    def __init__(self, fixture: Fixture, work: Path, probe, fault=lambda point: None):
        self.fixture, self.work, self.probe, self.fault = fixture, work, probe, fault
        if not work.resolve().is_relative_to(ROOT.resolve()):
            raise RuntimeError("audit/scratch must remain inside the active repository")
        for ancestor in (work,*work.parents):
            if ancestor.exists() and (ancestor.is_symlink() or getattr(ancestor.lstat(),"st_file_attributes",0)&0x400):
                raise RuntimeError("linked audit/scratch refused")
        self.work.mkdir(parents=True, exist_ok=True)
        self.state_path = work / "state.json"

    def target(self, name):
        if name not in NAMES:
            raise RuntimeError("unknown fixture target")
        return self.fixture.directory / name

    def candidate(self, name):
        return self.target(name).with_name(name + ".n5-candidate")

    @contextmanager
    def lock(self):
        path = self.work / "operation.lock"
        with path.open("a+b") as lock:
            lock.seek(0)
            lock.write(b"0")
            lock.flush()
            lock.seek(0)
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            try:
                yield
            finally:
                lock.seek(0)
                if os.name == "nt":
                    msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
                else:
                    fcntl.flock(lock, fcntl.LOCK_UN)

    def state(self):
        row = json.loads(self.state_path.read_text(encoding="utf-8"))
        if row["schema"] != 1 or row["fixture"] != self.fixture.name:
            raise RuntimeError("foreign fixture ledger")
        expected = dict(zip(NAMES[:2], (self.fixture.rel_hash, self.fixture.phy_hash)))
        if any(row["before"][name]["sha256"] != value for name, value in expected.items()):
            raise RuntimeError("unallowlisted ledger baseline")
        return row

    def validate_sources(self, expected=None):
        identity(self.target(NAMES[0]), 1, self.fixture.rel_owner)
        identity(self.target(NAMES[1]), 2, self.fixture.phy_owner)
        expected = expected or dict(zip(NAMES[:2], (self.fixture.rel_hash, self.fixture.phy_hash)))
        for name, value in expected.items():
            regular(self.target(name))
            if digest(self.target(name)) != value:
                raise RuntimeError(f"source hash drift: {name}")

    def inspect(self):
        if self.state_path.exists():
            row = self.state()
            if row["phase"] == "promoting":
                raise RuntimeError("interrupted promotion: recover before proceeding")
            if row["phase"] == "committed":
                self.validate_sources(row["after"])
                self.cleanup(row)
                return row["report"]
        self.validate_sources()
        inspection = self.work / "inspection"
        inspection.mkdir(exist_ok=True)
        try:
            for name in NAMES[:2]:
                shutil.copyfile(self.target(name), inspection / name)
            for name, expected in zip(NAMES[:2], (self.fixture.rel_hash,self.fixture.phy_hash)):
                if digest(inspection / name) != expected:
                    raise RuntimeError(f"inspection copy is not the allowlisted input: {name}")
            snapshot = self.probe("inspect")
        finally:
            for name in NAMES[:2]:
                (inspection / name).unlink(missing_ok=True)
            inspection.rmdir()
        self.validate_sources()
        write_json(self.work / "baseline.json", snapshot)
        report = {"schema": 1, "fixture": self.fixture.name, "source":str(self.fixture.directory),
                  "preserved": ["owner/Archive clock", "Memory bodies/provenance/history", "graph authority", "actual Dream", "accepted-use proof"],
                  "derived_from_legacy_evidence": [], "newly_executed_current_work": [], "approximation": [],
                  "h1_before": snapshot["rel"]["preflight"], "vectors_before":snapshot["rel"]["vectors"],
                  "proven_publication_proposals":snapshot["rel"]["publication"],
                  "inspection_provenance":snapshot.get("run_provenance",{}),
                  "promotion": "not authorized by verification yet"}
        write_json(self.work / "proposal.json", report)
        return report

    def prepare(self):
        if self.state_path.exists():
            raise RuntimeError("existing stage: verify, recover or discard it explicitly")
        report = self.inspect()
        print(canonical(report).decode("utf-8"), end="", flush=True)
        if any(self.candidate(name).exists() for name in NAMES):
            raise RuntimeError("unowned candidate exists")
        before = {}
        try:
            for name in NAMES:
                source = self.target(name)
                regular(source)
                before[name] = {"sha256":digest(source), "bytes":source.stat().st_size,
                                "mode":stat.S_IMODE(source.stat().st_mode)}
                shutil.copyfile(source, self.candidate(name))
            self.validate_sources({n:r["sha256"] for n,r in before.items()})
            write_json(self.state_path, {"schema":1,"fixture":self.fixture.name,"phase":"prepared",
                       "before":before,"baseline_digest":digest(self.work / "baseline.json"),"report":report})
        except BaseException:
            for name in NAMES:
                self.candidate(name).unlink(missing_ok=True)
            raise
        return report

    def verify(self):
        row = self.state()
        if row["phase"] not in ("prepared", "verified"):
            raise RuntimeError("no prepared candidate")
        self.validate_sources({n:r["sha256"] for n,r in row["before"].items()})
        identity(self.candidate(NAMES[0]), 1, self.fixture.rel_owner)
        identity(self.candidate(NAMES[1]), 2, self.fixture.phy_owner)
        if digest(self.work / "baseline.json") != row["baseline_digest"]:
            raise RuntimeError("baseline proof drift")
        evidence = json.loads((self.work / "evidence.json").read_text(encoding="utf-8"))
        categories = ("preserved", "derived_from_legacy_evidence", "newly_executed_current_work", "approximation")
        if set(evidence) != set(categories) or any(not isinstance(evidence[k], list) for k in categories):
            raise RuntimeError("normalization evidence must distinguish all four categories")
        before = json.loads((self.work / "baseline.json").read_text(encoding="utf-8"))
        after = self.probe("verify")
        preserved(before, after)
        old, new = before["rel"], after["rel"]
        derived = (set(new["publication_births"]) - set(old["publication_births"])) & set(old["memories"])
        claims = {r.get("memory_id"):r.get("evidence_digest") for r in evidence["derived_from_legacy_evidence"]
                  if isinstance(r,dict) and r.get("kind")=="publication-birth"}
        for memory in derived:
            expected = hashlib.sha256(canonical(old["publication"]["proven"][memory])).hexdigest()
            if claims.get(memory) != expected:
                raise RuntimeError(f"derived publication must name its retained proof: {memory}")
        required_actual = set(new["publication_births"]) - (set(old["memories"]) - set(old["publication_births"]))
        if not required_actual.issubset(new["actual_dream"]):
            raise RuntimeError("current native publications still require genuine completed first-cycle Dream")
        legacy = set(old["memories"]) - set(new["actual_dream"])
        approximations = {r.get("memory_id") for r in evidence["approximation"]
                          if isinstance(r,dict) and r.get("mode")=="legacy-approximation" and isinstance(r.get("roots"),list)}
        if not legacy.issubset(approximations):
            raise RuntimeError("every legacy Dream result, including no-link, needs explicit approximation coverage")
        historical_marker = self.target(NAMES[2]).read_text(encoding="utf-8")
        hashes = {name:digest(self.candidate(name)) for name in NAMES[:2]}
        report = {**row["report"], **evidence, "before":row["before"], "after":hashes,
                  "h1_after":after["rel"]["preflight"], "vectors_after":after["rel"]["vectors"],
                  "actual_dream_preserved":len(before["rel"]["actual_dream"]),
                  "actual_dream_after":len(after["rel"]["actual_dream"]),
                  "verification_provenance":after.get("run_provenance",{}), "promotion":"verified candidate"}
        marker = "Current authoritative Freshness normalization state (N5 verified normalization):\n"
        for scope,name in zip(("rel","phy"),NAMES[:2]):
            marker += f"{scope.upper()} owner: {after[scope]['owner']}\n{scope.upper()} SHA256: {hashes[name]}\n"
            marker += f"{scope.upper()} bytes: {self.candidate(name).stat().st_size}\n"
            marker += f"{scope.upper()} versions: {json.dumps(after[scope].get('versions',{}),sort_keys=True)}\n"
        marker += "Ordinary-owner reopen, invariant comparison, current vector coverage and H1 preflight passed.\n"
        marker += "Evidence categories and before/after proof are recorded in the temporary N5 verification audit.\n"
        marker += "\nHistorical pre-normalization marker (prior versions/status; preserved verbatim):\n" + historical_marker
        self.candidate(NAMES[2]).write_text(marker,encoding="utf-8")
        self.candidate(NAMES[3]).write_text("".join(f"{hashes[name]}  {name}\n" for name in NAMES[:2]),encoding="utf-8")
        row.update(phase="verified",after={name:digest(self.candidate(name)) for name in NAMES},
                   evidence_digest=digest(self.work / "evidence.json"),report=report)
        write_json(self.work / "verification.json", {"snapshot":after,"report":report})
        row["verification_digest"] = digest(self.work / "verification.json")
        write_json(self.state_path,row)
        return report

    def recover(self):
        row = self.state()
        if row["phase"] != "promoting":
            raise RuntimeError("no interrupted promotion")
        # Validate all rollback authority before replacing any target.
        for name in NAMES:
            backup = self.work / (name + ".rollback")
            regular(backup)
            if digest(backup) != row["before"][name]["sha256"]:
                raise RuntimeError(f"rollback evidence drift: {name}")
            regular(self.target(name))
            if digest(self.target(name)) not in (row["before"][name]["sha256"],row["after"][name]):
                raise RuntimeError(f"unknown target after interruption: {name}")
        for name in NAMES:
            target = self.target(name)
            target.chmod(stat.S_IWRITE | stat.S_IREAD)
            temporary = self.candidate(name)
            temporary.unlink(missing_ok=True)
            shutil.copyfile(self.work / (name + ".rollback"),temporary)
            os.replace(temporary,target)
            target.chmod(row["before"][name]["mode"])
        self.validate_sources({n:r["sha256"] for n,r in row["before"].items()})
        row["phase"] = "rolled-back"
        write_json(self.state_path,row)
        self.cleanup(row)
        return row["report"]

    def cleanup(self, row):
        for name in NAMES:
            self.candidate(name).unlink(missing_ok=True)
            (self.work / (name + ".rollback")).unlink(missing_ok=True)

    def promote(self):
        row = self.state()
        if row["phase"] != "verified":
            raise RuntimeError("candidate has not passed verification")
        self.validate_sources({n:r["sha256"] for n,r in row["before"].items()})
        if (digest(self.work / "evidence.json") != row["evidence_digest"]
                or digest(self.work / "verification.json") != row["verification_digest"]):
            raise RuntimeError("verification evidence drift")
        for name in NAMES:
            regular(self.candidate(name))
            if digest(self.candidate(name)) != row["after"][name]:
                raise RuntimeError(f"candidate drift: {name}")
            with self.candidate(name).open("r+b") as staged:
                os.fsync(staged.fileno())
        for name in NAMES:
            shutil.copyfile(self.target(name),self.work / (name + ".rollback"))
            with (self.work / (name + ".rollback")).open("r+b") as backup:
                os.fsync(backup.fileno())
            if digest(self.work / (name + ".rollback")) != row["before"][name]["sha256"]:
                raise RuntimeError(f"backup source drift: {name}")
        self.validate_sources({n:r["sha256"] for n,r in row["before"].items()})
        row["phase"] = "promoting"
        write_json(self.state_path,row)
        try:
            for name in NAMES:
                target = self.target(name)
                self.fault("before:" + name)
                target.chmod(stat.S_IWRITE | stat.S_IREAD)
                os.replace(self.candidate(name),target)
                target.chmod(row["before"][name]["mode"])
                self.fault("after:" + name)
            self.validate_sources(row["after"])
            self.fault("before:commit")
            row["phase"] = "committed"
            write_json(self.state_path,row)
        except BaseException:
            self.recover()
            raise
        self.cleanup(row)
        return row["report"]

    def discard(self):
        row = self.state()
        if row["phase"] == "promoting":
            raise RuntimeError("recover the interrupted promotion first")
        if row["phase"] == "committed":
            raise RuntimeError("keep the committed audit until N10")
        self.validate_sources({n:r["sha256"] for n,r in row["before"].items()})
        self.cleanup(row)
        self.state_path.unlink()


def native_probe(fixture: Fixture, work: Path, binary: Path, mode: str):
    binary = binary.resolve(strict=True)
    if binary.parent not in (ROOT/"target/debug/deps",ROOT/"target/release/deps") or not binary.name.startswith("reliquary_memory-"):
        raise RuntimeError("use the current repo-local library test binary")
    base = work / "inspection" if mode == "inspect" else fixture.directory
    paths = [base / name if mode == "inspect" else base / (name + ".n5-candidate") for name in NAMES[:2]]
    hashes = [digest(p) for p in paths]
    request = work / "request.json"
    write_json(request, {"fixture":fixture.name,"mode":mode,"rel_sha256":hashes[0],"phy_sha256":hashes[1]})
    output = work / "probe.json"
    output.unlink(missing_ok=True)
    env = dict(os.environ,RELIQUARY_N5_REQUEST=str(request),TMP=str(ROOT / ".t"),TEMP=str(ROOT / ".t"))
    (ROOT / ".t").mkdir(exist_ok=True)
    with (work / "probe.log").open("w",encoding="utf-8") as log:
        subprocess.run([str(binary),"fixture_normalization_owner_probe","--ignored","--nocapture","--test-threads=1"],
                       cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,check=True,
                       creationflags=subprocess.CREATE_NO_WINDOW if os.name=="nt" else 0)
    if hashes != [digest(p) for p in paths]:
        raise RuntimeError("native inspection changed candidate bytes")
    result = json.loads(output.read_text(encoding="utf-8"))
    if result["schema"] != 1:
        raise RuntimeError("unknown native report")
    result["run_provenance"] = {"binary_sha256":digest(binary), "contract_sha256":{
        path:digest(ROOT/path) for path in ("scripts/freshness_fixture_normalizer.py",
            "src/facade/fixture_normalization_probe.rs","src/facade/fixture_normalization_publication.rs",
            "src/freshness_storage.rs","Cargo.lock")}}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action",choices=("inspect","prepare","verify","promote","recover","discard"))
    parser.add_argument("fixture",choices=tuple(FIXTURES))
    parser.add_argument("--binary",type=Path,required=True,help="current repo-local library test executable")
    args = parser.parse_args()
    fixture = FIXTURES[args.fixture]
    work = ROOT / "target/freshness-normalization" / fixture.name
    normalizer = Normalizer(fixture,work,lambda mode:native_probe(fixture,work,args.binary,mode))
    with normalizer.lock():
        result = getattr(normalizer,args.action)()
    if result is not None:
        print(canonical(result).decode("utf-8"),end="")


if __name__ == "__main__":
    main()
