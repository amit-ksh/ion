import base64
import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest
import uuid

spec = importlib.util.spec_from_file_location('ion_job_worker', Path(__file__).with_name('ion_job_worker.py'))
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)

class JobTests(unittest.TestCase):
    def request(self, script, **overrides):
        value = {'version': 1, 'request_id': str(uuid.uuid4()),
                 'argv': ['/usr/bin/python3', '-c', script], 'timeout_seconds': 2,
                 'inputs': [{'path': 'input.txt', 'content_base64': base64.b64encode(b'input').decode()}],
                 'output_paths': ['output.txt']}
        value.update(overrides)
        return value

    def run_fixture(self, value):
        # Explicitly use the bundled host Python for a private fixture. Production rejects Windows.
        value['argv'][0] = sys.executable
        worker.validate(value) if os.name == 'posix' else None
        with tempfile.TemporaryDirectory(prefix='ion-worker-test-') as folder:
            return worker.run(value, Path(folder) / 'work')

    def test_actual_command_files_exit_environment_and_output_receipt(self):
        value = self.request("import os,pathlib; assert 'ION_TEST_SECRET' not in os.environ; pathlib.Path('output.txt').write_bytes(pathlib.Path('input.txt').read_bytes()+b' result'); print('done')")
        os.environ['ION_TEST_SECRET'] = 'not-for-child'
        try:
            result = self.run_fixture(value)
        finally:
            del os.environ['ION_TEST_SECRET']
        self.assertEqual(result['exit_code'], 0)
        self.assertEqual(base64.b64decode(result['outputs'][0]['content_base64']), b'input result')
        self.assertEqual(result['outputs'][0]['sha256'], worker.hashlib.sha256(b'input result').hexdigest())
        self.assertEqual(base64.b64decode(result['stdout']['content_base64']).strip(), b'done')

    def test_validation_rejects_private_escape_alias_and_unknown_input(self):
        for path in ['../secret', '/root/secret', 'a//b', '.env', 'a/.ssh/id', 'a\\b', 'a/./b', 'out.pem']:
            with self.subTest(path=path), self.assertRaises(ValueError):
                worker.validate(self.request('', output_paths=[path]))
        for field, value in [('version', 2), ('timeout_seconds', True), ('argv', ['python', '-c', 'x']),
                             ('request_id', str(uuid.UUID(int=0))), ('extra', True)]:
            with self.subTest(field=field), self.assertRaises((ValueError, TypeError)):
                worker.validate(self.request('', **{field: value}))
        with self.assertRaises(ValueError):
            worker.validate(self.request('', output_paths=['x', 'X']))

    def test_failed_checks_timeout_truncation_and_unsafe_outputs(self):
        result = self.run_fixture(self.request("import sys; print('x'*10000); sys.exit(7)", output_paths=[]))
        self.assertEqual(result['exit_code'], 7)
        self.assertTrue(result['stdout']['truncated'])
        result = self.run_fixture(self.request("import time; time.sleep(10)", timeout_seconds=1, output_paths=[]))
        self.assertTrue(result['timed_out'])
        if os.name == 'posix':
            result = self.run_fixture(self.request("import os; os.symlink('/etc/passwd','output.txt')"))
            self.assertFalse(result['outputs'])
            self.assertEqual(len(result['output_errors']), 1)

    def test_binary_transfer_and_aggregate_bounds(self):
        value = self.request('', inputs=[{'path': 'binary.bin', 'content_base64': base64.b64encode(b'\0\xff').decode()}])
        self.assertEqual(worker.validate(value)[0][1], b'\0\xff')
        value['inputs'] *= 2
        with self.assertRaises(ValueError):
            worker.validate(value)
        with self.assertRaises(ValueError):
            worker.validate(self.request('', inputs=[{'path': 'x', 'content_base64': '%%%'}]))

if __name__ == '__main__':
    unittest.main()
