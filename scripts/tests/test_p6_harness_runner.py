import importlib.util, pathlib, tempfile, unittest, sys
ROOT=pathlib.Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location("p6",ROOT/"run_multiplexing_p6_harness.py")
p6=importlib.util.module_from_spec(spec);spec.loader.exec_module(p6)

class VerdictTests(unittest.TestCase):
    def run_child(self, script, **options):
        with tempfile.TemporaryDirectory() as directory:
            root=pathlib.Path(directory)
            return p6.run(root,root,"child",[sys.executable,"-c",script],dict(__import__("os").environ),2,**options)
    def test_zero_matched_tests_cannot_pass_a_required_matrix(self):
        result=self.run_child("print('test result: ok. 0 passed; 0 failed; 0 ignored')",require_tests=True)
        self.assertFalse(result["passed"])
    def test_nonzero_exit_without_durable_ready_marker_is_not_an_expected_crash(self):
        result=self.run_child("raise SystemExit(3)",expected_abort=True)
        self.assertFalse(result["passed"])
    def test_expected_abort_records_nonzero_code_and_does_not_claim_ordinary_test_success(self):
        result=self.run_child("print('P6_CRASH_READY {}',flush=True);raise SystemExit(3)",expected_abort=True)
        self.assertTrue(result["passed"])
        self.assertEqual(result["exit_code"],3)
        self.assertEqual(result["test_summaries"],[])
    def test_comparison_rejects_different_hardware_and_workloads(self):
        before={"environment":{"hardware":{"cpu":"a"},"revision":"base"},"workload":{"seed":38},"rollup":{}}
        after={"environment":{"hardware":{"cpu":"b"},"revision":"candidate"},"workload":{"seed":38},"rollup":{}}
        with self.assertRaises(ValueError):p6.compare(after,before)
        after["environment"]["hardware"]={"cpu":"a"};after["workload"]={"seed":39}
        with self.assertRaises(ValueError):p6.compare(after,before)

    def test_regression_suite_does_not_inflate_requested_timing_repetitions(self):
        metric={"workload":"checkpoints","values":{"checkpoint_ns":100}}
        result=p6.rollup([
            {"name":"harness-0","metrics":[metric]},
            {"name":"existing-gateway","metrics":[{"workload":"checkpoints","values":{"checkpoint_ns":900}}]},
        ])
        self.assertEqual(result["checkpoints/checkpoint_ns"]["samples"],1)
        self.assertEqual(result["checkpoints/checkpoint_ns"]["median"],100)

if __name__=="__main__":unittest.main()
