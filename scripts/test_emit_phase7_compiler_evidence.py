"""Focused tests for bounded compiler-output capture."""
import os
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from emit_phase7_compiler_evidence import run_streamed_stdout


class StreamedCompilerOutputTests(unittest.TestCase):
    def setUp(self):
        self.temp_root = Path(__file__).resolve().parents[1] / '.verification-tmp'
        self.temp_root.mkdir(exist_ok=True)

    def run_child(self, code):
        with tempfile.TemporaryDirectory(prefix='stream-output-', dir=self.temp_root) as folder:
            root = Path(folder)
            artifact = root / 'artifact.bin'
            errors = root / 'stderr.log'
            command = [sys.executable, '-c', code]
            with errors.open('wb') as stderr:
                result = run_streamed_stdout(command, cwd=root, env=os.environ.copy(),
                                             artifact=artifact, stderr=stderr)
            return result, artifact.read_bytes(), errors.read_text(encoding='utf-8')

    def test_streams_large_output_to_artifact(self):
        code = "import sys; [sys.stdout.buffer.write(b'x' * 65536) for _ in range(80)]"
        result, output, stderr = self.run_child(code)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(len(output), 80 * 65536)
        self.assertEqual(output, b'x' * (80 * 65536))
        self.assertEqual(stderr, '')

    def test_preserves_artifact_and_stderr_on_child_failure(self):
        code = "import sys; sys.stdout.buffer.write(b'partial'); print('failed', file=sys.stderr); sys.exit(7)"
        result, output, stderr = self.run_child(code)
        self.assertEqual(result.returncode, 7)
        self.assertEqual(output, b'partial')
        self.assertEqual(stderr, 'failed\n')


if __name__ == '__main__':
    unittest.main()
