import base64
import importlib.util
import os
import json
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

    def test_supervisor_caps_reject_missing_permissions_before_execution(self):
        required = sum(1 << bit for bit in (0, 5, 6, 7))
        worker.require_supervisor_capabilities(required | (1 << 1))
        worker.require_supervisor_capabilities(required | (1 << 2))
        for mask in [0, required, -1, True, 1 << 64, None] + [
            (required | (1 << 1)) & ~(1 << bit) for bit in (0, 5, 6, 7)
        ]:
            with self.subTest(mask=mask), self.assertRaises(ValueError):
                worker.require_supervisor_capabilities(mask)

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

class GpuResultTests(unittest.TestCase):
    request = JobTests.request
    run_fixture = JobTests.run_fixture
    def test_result_volume_stores_actual_nonzero_logs_and_output_without_overwrite(self):
        request = self.request("import pathlib,sys; pathlib.Path('output.txt').write_bytes(b'checkpoint'); print('training failed',file=sys.stderr); sys.exit(9)")
        result = self.run_fixture(request)
        with tempfile.TemporaryDirectory(prefix='ion-result-test-') as temp:
            volume = Path(temp)
            folder = worker.prepare_result_directory(volume, request['request_id'])
            worker.write_result(folder, result)
            saved = json.loads((folder / 'receipt.json').read_bytes())
            self.assertEqual(saved, result)
            self.assertEqual(saved['exit_code'], 9)
            self.assertEqual(base64.b64decode(saved['stderr']['content_base64']).strip(), b'training failed')
            self.assertEqual(base64.b64decode(saved['outputs'][0]['content_base64']), b'checkpoint')
            with self.assertRaises(FileExistsError):
                worker.write_result(folder, result)
            with self.assertRaises(FileExistsError):
                worker.prepare_result_directory(volume, request['request_id'])

    def test_result_identity_size_and_destination_fail_before_writing(self):
        request = self.request('', output_paths=[])
        result = self.run_fixture(request)
        with tempfile.TemporaryDirectory(prefix='ion-result-test-') as temp:
            volume = Path(temp)
            for identity in ['../escape', str(uuid.UUID(int=0)), '12345678-1234-4234-8234-123456789ABC']:
                with self.assertRaises(ValueError):
                    worker.prepare_result_directory(volume, identity)
            folder = worker.prepare_result_directory(volume, request['request_id'])
            with self.assertRaises(ValueError):
                worker.write_result(folder, {**result,'request_id': str(uuid.uuid4())})
            with self.assertRaises(ValueError):
                worker.write_result(folder, {**result,'extra': 'x'*(512*1024)})
            self.assertFalse((folder/'receipt.json').exists())
            if os.name == 'posix':
                link = volume/'link'
                link.symlink_to(folder, target_is_directory=True)
                with self.assertRaises(ValueError):
                    worker.prepare_result_directory(link, str(uuid.uuid4()))
                (folder/'receipt.json').symlink_to(volume/'outside')
                with self.assertRaises(FileExistsError):
                    worker.write_result(folder, result)
                self.assertFalse((volume/'outside').exists())

if __name__ == '__main__':
    unittest.main()
