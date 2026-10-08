#!/usr/bin/env python3
"""Production CLI acceptance inside a disposable, credential-free Linux image."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
import uuid

BINARY = "/usr/local/bin/ion-job-worker"
EXPORTED = None


def request(script, **changes):
    value = {"version": 1, "request_id": str(uuid.uuid4()),
             "argv": ["/usr/local/bin/python3", "-I", "-S", "-c", script],
             "timeout_seconds": 5, "inputs": [], "output_paths": []}
    value.update(changes)
    return value


class NativeCliTests(unittest.TestCase):
    def invoke(self, value, args=(), exit_code=0, missing_cap=None):
        data = json.dumps(value).encode() if not isinstance(value, bytes) else value
        env = {"PATH": "/usr/local/bin:/usr/bin:/bin", "ION_JOB_ISOLATED": "1",
               "ION_JOB_CHILD_UID": "1000", "ION_JOB_CHILD_GID": "1000",
               "ION_JOB_TEST_SECRET": "private-fixture-not-for-command"}
        command = [BINARY, *args]
        if missing_cap is not None:
            # A fixed private launcher changes only the supervisor's capabilities.
            # No user code or configurable library path enters this launcher.
            launcher = r"""import ctypes,os,sys
class Header(ctypes.Structure):
    _fields_=[('version',ctypes.c_uint32),('pid',ctypes.c_int)]
class Data(ctypes.Structure):
    _fields_=[('effective',ctypes.c_uint32),('permitted',ctypes.c_uint32),('inheritable',ctypes.c_uint32)]
libc=ctypes.CDLL(None,use_errno=True)
for name in ('capget','capset'):
    getattr(libc,name).argtypes=[ctypes.POINTER(Header),ctypes.POINTER(Data)]
    getattr(libc,name).restype=ctypes.c_int
header=Header(0x20080522,0)
values=(Data*2)()
assert libc.capget(ctypes.byref(header),values)==0
bit=int(sys.argv[1]); entry=values[bit//32]; flag=1<<(bit%32)
entry.effective &= ~flag; entry.permitted &= ~flag; entry.inheritable &= ~flag
assert libc.capset(ctypes.byref(header),values)==0
os.execve(sys.argv[2],sys.argv[2:],dict(os.environ))
"""
            command = ['/usr/local/bin/python3','-I','-B','-S','-c',launcher,str(missing_cap),*command]
        result = subprocess.run(command, input=data, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, env=env, cwd="/ion", timeout=20)
        self.assertEqual(result.returncode, exit_code, result.stderr[:1024])
        self.assertEqual(result.stderr, b"")
        self.assertLessEqual(len(result.stdout), 512 * 1024)
        return json.loads(result.stdout)

    def test_binary_transfer_native_identity_socket_boundary_and_nonzero_receipt(self):
        global EXPORTED
        payload = bytes(range(256)) * 200
        script = r"""import errno,os,pathlib,socket,sys
assert os.geteuid()==1000 and os.getegid()==1000 and os.getgroups()==[]
assert 'ION_JOB_TEST_SECRET' not in os.environ and 'ION_JOB_ISOLATED' not in os.environ
status=pathlib.Path('/proc/self/status').read_text()
assert 'CapEff:\t0000000000000000' in status and 'NoNewPrivs:\t1' in status
for family in (socket.AF_INET, socket.AF_INET6, socket.AF_UNIX):
    try: socket.socket(family, socket.SOCK_STREAM)
    except OSError as error: assert error.errno==errno.ENETDOWN
    else: raise AssertionError('Native child socket boundary missing')
try: pathlib.Path('/proc/%s/environ'%os.getppid()).read_bytes()
except PermissionError: pass
else: raise AssertionError('Child read supervisor environment')
pathlib.Path('output.bin').write_bytes(pathlib.Path('input.bin').read_bytes()+b'\x00\xffresult')
print('checked',end='')
sys.exit(7)
"""
        value = request(script, inputs=[{"path": "input.bin", "content_base64": base64.b64encode(payload).decode()}], output_paths=["output.bin"])
        receipt = self.invoke(value)
        self.assertEqual(receipt['request_id'], value['request_id'])
        self.assertEqual(receipt['exit_code'], 7, base64.b64decode(receipt['stderr']['content_base64']).decode('utf-8', errors='replace'))
        self.assertFalse(receipt['timed_out'] or receipt['canceled'])
        self.assertEqual(base64.b64decode(receipt['stdout']['content_base64']), b'checked')
        self.assertFalse(receipt['output_errors'])
        output = receipt['outputs'][0]
        data = base64.b64decode(output['content_base64'])
        self.assertEqual(data, payload+b'\x00\xffresult')
        self.assertEqual(output['bytes'],len(data))
        self.assertEqual(output['sha256'],hashlib.sha256(data).hexdigest())
        EXPORTED = (value, receipt)

    def test_native_cli_supervisor_refuses_missing_permissions_before_child_effects(self):
        for bit in (0, 1, 5, 6, 7):
            with self.subTest(capability=bit):
                marker=Path('/tmp','ion-cli-denied-'+str(uuid.uuid4()))
                value=request("import pathlib; pathlib.Path(%r).write_bytes(b'ran')" % str(marker))
                result=self.invoke(value,exit_code=1,missing_cap=bit)
                self.assertEqual(result['error'],'Job contract or execution failed')
                self.assertFalse(marker.exists(),'Command started without supervisor permissions')

    def test_native_cli_timeout_and_log_bounds(self):
        receipt = self.invoke(request("import sys,time; print('x'*20000,flush=True); time.sleep(30)",timeout_seconds=1))
        self.assertTrue(receipt['timed_out'])
        self.assertLess(receipt['exit_code'],0)
        self.assertTrue(receipt['stdout']['truncated'])
        self.assertEqual(len(base64.b64decode(receipt['stdout']['content_base64'])),8192)
        self.assertGreater(receipt['stdout']['bytes'],8192)

    def test_native_cli_reaps_detached_descendant_before_file_collection(self):
        value = request("""import os,pathlib,time
child=os.fork()
if child==0:
    os.setsid(); time.sleep(30); pathlib.Path('output.bin').write_bytes(b'late'); os._exit(0)
pathlib.Path('output.bin').write_bytes(b'original')
print(child,flush=True)
os._exit(0)
""",output_paths=['output.bin'])
        receipt = self.invoke(value)
        self.assertEqual(receipt['exit_code'],0)
        self.assertFalse(receipt['timed_out'])
        child = int(base64.b64decode(receipt['stdout']['content_base64']).strip())
        self.assertFalse(Path('/proc',str(child)).exists(),'Detached descendant still exists')
        self.assertEqual(base64.b64decode(receipt['outputs'][0]['content_base64']),b'original')

    def test_native_cli_rejects_link_and_oversized_outputs(self):
        script = r"""import os,pathlib
pathlib.Path('data').write_bytes(b'original')
os.link('data','hard.bin')
os.symlink('data','soft.bin')
pathlib.Path('large.bin').write_bytes(b'x'*65537)
"""
        receipt = self.invoke(request(script,output_paths=['hard.bin','soft.bin','large.bin']))
        self.assertEqual(receipt['exit_code'],0)
        self.assertEqual(receipt['outputs'],[])
        self.assertEqual({item['path'] for item in receipt['output_errors']},{'hard.bin','soft.bin','large.bin'})

    def test_native_cli_invalid_input_never_becomes_a_receipt(self):
        invalid = [b'{malformed private-fixture-not-echoed', b' '*(512*1024+1)]
        value = request('')
        invalid += [{**value,'version':2}, {**value,'private':'private-fixture-not-echoed'},
                    {**value,'output_paths':['../private']},
                    {**value,'argv':['python3','-c','invalid']}]
        for value in invalid:
            with self.subTest(input_type=type(value).__name__):
                result = self.invoke(value, exit_code=1)
                self.assertEqual(set(result),{'error','help'})
                self.assertNotIn('private-fixture-not-echoed', json.dumps(result))

    def test_native_cli_exclusive_gpu_result_and_private_parent(self):
        result_root = Path('/ion/results')
        result_root.mkdir(mode=0o700,exist_ok=False)
        effect = Path('/tmp', 'ion-cli-effect-'+str(uuid.uuid4()))
        script = r"""import pathlib,sys
try: pathlib.Path('/ion/results/forged').write_bytes(b'forged')
except PermissionError: pass
else: raise AssertionError('Child reached private collector mount')
with pathlib.Path(%r).open('a') as output: output.write('run\n')
pathlib.Path('output.bin').write_bytes(b'checkpoint')
sys.exit(9)
""" % str(effect)
        value = request(script,output_paths=['output.bin'])
        Path('/ion/job-request.json').write_text(json.dumps(value),encoding='utf-8')
        args = ['--request-file','/ion/job-request.json','--result-volume','/ion/results']
        acknowledgment = self.invoke(b'',args)
        self.assertEqual(acknowledgment,{'version':1,'request_id':value['request_id'],'result_written':True})
        saved = Path('/ion/results',value['request_id'],'receipt.json').read_bytes()
        receipt = json.loads(saved)
        self.assertEqual(receipt['exit_code'],9)
        self.assertEqual(base64.b64decode(receipt['outputs'][0]['content_base64']),b'checkpoint')
        self.assertEqual(effect.read_text(),'run\n')
        self.assertEqual(Path('/ion').stat().st_mode & 0o777,0o700)
        self.invoke(b'',args,exit_code=1)
        self.assertEqual(effect.read_text(),'run\n','Duplicate result identity repeated the command')
        self.assertEqual(Path('/ion/results',value['request_id'],'receipt.json').read_bytes(),saved)
        self.assertFalse(Path('/ion/results/forged').exists())

    def test_native_cli_manifest_help_and_version(self):
        manifest = json.loads(Path('/ion/job-host.json').read_bytes())
        self.assertEqual(manifest,{'version':1,'worker':'ion-job-worker/1','credential_free':True,
                                   'network':'disabled','supervisor':'root','child':'nonroot',
                                   'network_boundary':'linux-seccomp-socket-deny/v1'})
        for arg in ['--help','--version']:
            result = subprocess.run([BINARY,arg],capture_output=True,timeout=5)
            self.assertEqual(result.returncode,0)
            self.assertIn(b'ion-job-worker',result.stdout)
        result = subprocess.run([BINARY,'--unknown'],capture_output=True,timeout=5)
        self.assertEqual(result.returncode,2)
        self.assertEqual(json.loads(result.stdout)['error'],'Unsupported arguments')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture-dir', type=Path, help='Export the verified native request, receipt and image manifest after all tests pass')
    args=parser.parse_args()
    if sys.platform!='linux' or os.geteuid()!=0 or os.environ.get('ION_JOB_CLI_ACCEPTANCE')!='1' or not Path(BINARY).is_file():
        parser.error('Use the reviewed disposable Linux worker image as root with ION_JOB_CLI_ACCEPTANCE=1')
    result=unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(NativeCliTests))
    if not result.wasSuccessful():
        return 1
    if args.fixture_dir is not None:
        root=args.fixture_dir.resolve(strict=True)
        if not root.is_dir() or args.fixture_dir.is_symlink() or EXPORTED is None:
            raise ValueError('Use a fresh mounted output directory')
        for name, data in [('request.json',EXPORTED[0]),('receipt.json',EXPORTED[1]),('manifest.json',json.loads(Path('/ion/job-host.json').read_bytes()))]:
            with (root/name).open('x',encoding='utf-8') as target:
                json.dump(data,target,separators=(',',':'))
    print(json.dumps({'tests':result.testsRun,'passed':True,'fixture_exported':args.fixture_dir is not None},separators=(',',':')))
    return 0


if __name__=='__main__':
    raise SystemExit(main())
