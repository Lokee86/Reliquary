"""Disposable N5 transaction tests. Native semantic acceptance is tested in Rust."""
import copy
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from contextlib import redirect_stdout
import io
from contextlib import redirect_stderr
import uuid
import freshness_fixture_normalizer as n


def snapshot():
    memory = {"m": {"body":"body","provenance":"source","revision":1}}
    return {"schema":1,"rel":{"owner":"rel","activity_count":2,"nodes":{"u":1,"a":2},
            "branches":{"main":"canonical"},"memories":memory,"history":{"m/1":"revision"},
            "graph":["edge"],"episodes":["episode"],"fragments":["mixed"],
            "accepted_use":[],"actual_dream":{"m":"genuine pass"},
            "archive_clock_valid":True,"preflight":{"ready":True},"vectors":{"ready":True},
            "publication":{"proven":{"m":"unique receipt"}},"publication_births":["m"]},
            "phy":{"owner":"phy","memories":copy.deepcopy(memory),"history":{"m/1":"revision"},"graph":[]}}


class FixtureTests(unittest.TestCase):
    def setUp(self):
        (n.ROOT / ".t").mkdir(exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(dir=n.ROOT / ".t",prefix="n5-transport-")
        self.root = Path(self.temp.name)
        self.directory = self.root / "authoritative"
        self.directory.mkdir()
        owners = [str(uuid.uuid4()),str(uuid.uuid4())]
        for name,kind,owner in zip(n.NAMES[:2],(1,2),owners):
            header = b"CVA\0\r\n\x1a\n\x01\x00\x00\x00\x28\x00\x00\x00" + bytes([kind,0]) + bytes(6) + uuid.UUID(owner).bytes
            (self.directory / name).write_bytes(header+b"transport-test baseline")
        (self.directory / n.NAMES[2]).write_text("authoritative marker\n",encoding="utf-8")
        (self.directory / n.NAMES[3]).write_text("original checksums\n",encoding="utf-8")
        self.fixture = n.Fixture("transport-test",self.directory,*(n.digest(self.directory/x) for x in n.NAMES[:2]),*owners)
        self.before = {name:(self.directory/name).read_bytes() for name in n.NAMES}
        self.proofs = snapshot()
        self.tool = n.Normalizer(self.fixture,self.root/"work",lambda mode:copy.deepcopy(self.proofs))

    def tearDown(self):
        for path in self.root.rglob("*"):
            if path.is_file():
                path.chmod(stat.S_IWRITE|stat.S_IREAD)
        self.temp.cleanup()

    def prepared(self):
        with redirect_stdout(io.StringIO()):
            self.tool.prepare()
        for name in n.NAMES[:2]:
            with self.tool.candidate(name).open("ab") as output:
                output.write(b" normalized transport bytes")
        n.write_json(self.tool.work/"evidence.json",{k:[] for k in (
            "preserved","derived_from_legacy_evidence","newly_executed_current_work","approximation")})

    def verified(self):
        self.prepared()
        self.tool.verify()

    def unchanged(self):
        self.assertEqual(self.before,{name:(self.directory/name).read_bytes() for name in n.NAMES})

    def test_cli_refuses_arbitrary_paths(self):
        with patch.object(sys,"argv",["normalizer","prepare",str(self.directory),"--binary","fake.exe"]),redirect_stdout(io.StringIO()),redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                n.main()
        self.unchanged()

    def test_hash_and_owner_gate_precedes_probe(self):
        for owner in (False,True):
            path = self.directory/n.NAMES[0]
            data = bytearray(self.before[n.NAMES[0]])
            data[24 if owner else -1] ^= 1
            path.write_bytes(data)
            with self.assertRaises(RuntimeError):
                self.tool.inspect()
            path.write_bytes(self.before[n.NAMES[0]])
        self.unchanged()

    def test_linked_input_and_metadata_drift_refuse_before_mutation(self):
        alias = self.root/"alias.rel"
        os.link(self.directory/n.NAMES[0],alias)
        with self.assertRaises(RuntimeError):
            self.tool.inspect()
        alias.unlink()
        self.prepared()
        marker = self.directory/n.NAMES[2]
        marker.write_bytes(marker.read_bytes()+b"concurrent marker drift")
        before = {name:(self.directory/name).read_bytes() for name in n.NAMES}
        with self.assertRaises(RuntimeError):
            self.tool.verify()
        self.assertEqual(before,{name:(self.directory/name).read_bytes() for name in n.NAMES})

    def test_unverified_and_stale_candidate_cannot_promote(self):
        self.prepared()
        with self.assertRaises(RuntimeError):
            self.tool.promote()
        self.tool.verify()
        with self.tool.candidate(n.NAMES[0]).open("ab") as output:
            output.write(b"drift")
        with self.assertRaises(RuntimeError):
            self.tool.promote()
        self.unchanged()

    def test_foreign_or_incomplete_native_proof_cannot_verify(self):
        self.prepared()
        for field,value in (("activity_count",3),("preflight",{"ready":False}),
                            ("accepted_use",["invented +25"]),("actual_dream",{}),("vectors",{"ready":False})):
            original = self.proofs["rel"][field]
            self.proofs["rel"][field] = value
            with self.assertRaises(RuntimeError):
                self.tool.verify()
            self.proofs["rel"][field] = original
        self.unchanged()

    def test_memory_revision_provenance_branch_and_graph_proof_cannot_change(self):
        before = snapshot()
        for field,value in (("memories",{}),("history",{}),("branches",{}),("graph",[]),
                            ("nodes",{}),("episodes",[]),("fragments",[])):
            after = copy.deepcopy(before)
            after["rel"][field] = value
            with self.assertRaises(RuntimeError):
                n.preserved(before,after)

    def test_legacy_birth_requires_proof_and_cannot_be_relabelled_actual_dream(self):
        before = snapshot()
        before["rel"]["publication_births"] = []
        before["rel"]["actual_dream"] = {}
        after = copy.deepcopy(before)
        after["rel"]["publication_births"] = ["m"]
        n.preserved(before,after)
        before["rel"]["publication"]["proven"] = {}
        with self.assertRaises(RuntimeError):
            n.preserved(before,after)
        before["rel"]["publication"]["proven"] = {"m":"retained proof"}
        after["rel"]["actual_dream"] = {"m":"fabricated actual"}
        with self.assertRaises(RuntimeError):
            n.preserved(before,after)

    def test_failure_at_each_promotion_boundary_restores_every_file(self):
        points = [stage+":"+name for name in n.NAMES for stage in ("before","after")]+["before:commit"]
        for point in points:
            with self.subTest(point=point):
                self.verified()
                self.tool.fault = lambda at,p=point: (_ for _ in ()).throw(OSError("injected")) if at==p else None
                with self.assertRaises(OSError):
                    self.tool.promote()
                self.unchanged()
                self.assertEqual(self.tool.state()["phase"],"rolled-back")
                self.assertFalse(any(self.tool.candidate(name).exists() for name in n.NAMES))
                self.tool.discard()

    def test_success_restores_readonly_and_committed_retry_preserves_hashes(self):
        for name in n.NAMES:
            (self.directory/name).chmod(stat.S_IREAD)
        self.verified()
        self.tool.promote()
        self.assertEqual(self.tool.state()["phase"],"committed")
        for name in n.NAMES:
            self.assertFalse((self.directory/name).stat().st_mode & stat.S_IWRITE)
            self.assertFalse(self.tool.candidate(name).exists())
        after = {name:n.digest(self.directory/name) for name in n.NAMES}
        self.tool.inspect()
        self.assertEqual(after,{name:n.digest(self.directory/name) for name in n.NAMES})

    def test_process_death_recovery_restores_pair_and_metadata(self):
        self.verified()
        code = '''import json,os,sys
from pathlib import Path
sys.path.insert(0,sys.argv[1])
import freshness_fixture_normalizer as n
spec=json.loads(sys.argv[2]);spec["directory"]=Path(spec["directory"])
tool=n.Normalizer(n.Fixture(**spec),Path(sys.argv[3]),None,lambda point:os._exit(88) if point=="after:user.phy" else None)
with tool.lock():tool.promote()
'''
        spec = dict(vars(self.fixture));spec["directory"]=str(spec["directory"])
        result = subprocess.run([sys.executable,"-c",code,str(n.ROOT/"scripts"),json.dumps(spec),str(self.tool.work)],
                                creationflags=subprocess.CREATE_NO_WINDOW if os.name=="nt" else 0)
        self.assertEqual(result.returncode,88)
        self.assertEqual(self.tool.state()["phase"],"promoting")
        with self.tool.lock():
            self.tool.recover()
        self.unchanged()

    def test_rollback_refuses_unknown_target_or_corrupt_backup_without_writing(self):
        self.verified()
        row = self.tool.state()
        for name in n.NAMES:
            import shutil
            shutil.copyfile(self.tool.target(name),self.tool.work/(name+".rollback"))
        row["phase"]="promoting";n.write_json(self.tool.state_path,row)
        (self.tool.work/(n.NAMES[0]+".rollback")).write_bytes(b"bad backup")
        with self.assertRaises(RuntimeError):
            self.tool.recover()
        self.unchanged()


if __name__=="__main__":
    unittest.main()
