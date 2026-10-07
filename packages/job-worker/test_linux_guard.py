"""Run in the built disposable Linux image, with the fixed installed worker/guard."""
import base64
import errno
import json
import os
from pathlib import Path
import platform
import socket
import subprocess
import sys
import threading
import unittest
import uuid

WORKER = '/usr/local/bin/ion-job-worker'

@unittest.skipUnless(sys.platform == 'linux' and os.geteuid() == 0, 'Requires the isolated Linux test image')
class LinuxGuardTests(unittest.TestCase):
    def result(self, script, outputs=None, inherited_fds=()):
        request = {'version': 1, 'request_id': str(uuid.uuid4()),
                   'argv': [sys.executable, '-c', script], 'timeout_seconds': 4,
                   'inputs': [], 'output_paths': [] if outputs is None else outputs}
        env = {key: value for key, value in os.environ.items() if key in ('PATH', 'LANG', 'LC_ALL')}
        env.update(ION_JOB_ISOLATED='1', ION_JOB_CHILD_UID='1000', ION_JOB_CHILD_GID='1000',
                   ION_TEST_SECRET='fixture-only', PYTHONPATH='/tmp/ignored-by-child')
        process = subprocess.run([WORKER], input=json.dumps(request).encode(),
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env, timeout=15, pass_fds=inherited_fds)
        self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
        value = json.loads(process.stdout)
        self.assertEqual(value['request_id'], request['request_id'])
        return value

    def passed(self, script, outputs=None, inherited_fds=()):
        result = self.result(script, outputs, inherited_fds)
        self.assertEqual(result['exit_code'], 0, base64.b64decode(result['stderr']['content_base64']))
        self.assertFalse(result['timed_out'])
        return result

    def test_network_domains_and_metadata_fail_before_connect_while_collector_remains_online(self):
        # Prove the outer runtime permits sockets; denial must come from the child.
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            listener.listen(1)
            port = listener.getsockname()[1]
            errors = []
            def collector():
                try:
                    listener.settimeout(5)
                    conn, _ = listener.accept()
                    with conn:
                        self.assertEqual(conn.recv(4), b'ping')
                        conn.sendall(b'pong')
                except BaseException as error:
                    errors.append(error)
            thread = threading.Thread(target=collector)
            thread.start()
            self.passed("""
import errno,socket
for family,kind in [(socket.AF_INET,socket.SOCK_STREAM), (socket.AF_INET,socket.SOCK_DGRAM),
                    (socket.AF_INET6,socket.SOCK_STREAM), (socket.AF_UNIX,socket.SOCK_STREAM),
                    (socket.AF_UNIX,socket.SOCK_DGRAM), (socket.AF_NETLINK,socket.SOCK_RAW),
                    (socket.AF_PACKET,socket.SOCK_RAW)]:
    try: socket.socket(family,kind)
    except OSError as e: assert e.errno == errno.ENETDOWN, (family,e.errno)
    else: raise AssertionError('socket escaped')
for host in ('127.0.0.1','169.254.169.254','10.0.0.1','1.1.1.1'):
    try: socket.create_connection((host,80),timeout=.1)
    except OSError as e: assert e.errno == errno.ENETDOWN
    else: raise AssertionError('connection escaped')
try: socket.socketpair()
except OSError as e: assert e.errno == errno.ENETDOWN
else: raise AssertionError('Unix descriptor channel escaped')
""")
            with socket.create_connection(('127.0.0.1', port), timeout=2) as client:
                client.sendall(b'ping')
                self.assertEqual(client.recv(4), b'pong')
            thread.join(6)
            self.assertFalse(thread.is_alive())
            if errors:
                raise errors[0]

    def test_kernel_filter_and_empty_privileges_survive_fork_and_exec(self):
        self.passed(r"""
import errno,os,pathlib,socket,subprocess,sys,threading
fields=dict(x.split(':',1) for x in pathlib.Path('/proc/self/status').read_text().splitlines() if ':' in x)
assert os.getresuid()==(1000,1000,1000) and os.getresgid()==(1000,1000,1000) and os.getgroups()==[]
assert fields['NoNewPrivs'].strip()=='1' and fields['Seccomp'].strip()=='2'
assert all(int(fields[k].strip(),16)==0 for k in ('CapPrm','CapEff','CapAmb'))
seen=[]
t=threading.Thread(target=lambda:seen.append(1)); t.start(); t.join(); assert seen==[1]
pid=os.fork()
if pid==0:
    try: socket.socket()
    except OSError as e: os._exit(0 if e.errno==errno.ENETDOWN else 12)
    os._exit(13)
assert os.waitpid(pid,0)[1]==0
code='import errno,socket;\ntry: socket.socket()\nexcept OSError as e: assert e.errno==errno.ENETDOWN\nelse: raise AssertionError()'
assert subprocess.run([sys.executable,'-c',code]).returncode==0
""")

    def test_raw_network_ring_and_namespace_syscalls_are_denied(self):
        # Native syscall IDs are resolved using the same official library; execution
        # itself is raw syscall, independent of Python's socket wrappers.
        self.passed("""
import ctypes,errno
l=ctypes.CDLL('libseccomp.so.2'); l.seccomp_syscall_resolve_name.argtypes=[ctypes.c_char_p]; l.seccomp_syscall_resolve_name.restype=ctypes.c_int
c=ctypes.CDLL(None,use_errno=True); c.syscall.restype=ctypes.c_long
for name,error in [('socket',errno.ENETDOWN),('socketpair',errno.ENETDOWN),('connect',errno.ENETDOWN),
                   ('sendmsg',errno.ENETDOWN),('recvmsg',errno.ENETDOWN),
                   ('io_uring_setup',errno.ENOSYS),('io_uring_enter',errno.ENOSYS),('io_uring_register',errno.ENOSYS),
                   ('clone3',errno.ENOSYS),('unshare',errno.EPERM),('setns',errno.EPERM),
                   ('ptrace',errno.EPERM),('process_vm_readv',errno.EPERM),('process_vm_writev',errno.EPERM),
                   ('pidfd_getfd',errno.EPERM),('bpf',errno.EPERM)]:
    ctypes.set_errno(0); assert c.syscall(l.seccomp_syscall_resolve_name(name.encode()),0,0,0,0,0,0)==-1
    assert ctypes.get_errno()==error,(name,ctypes.get_errno())
for flag in (0x80,0x20000,0x2000000,0x4000000,0x8000000,0x10000000,0x20000000,0x40000000):
    ctypes.set_errno(0)
    assert c.syscall(l.seccomp_syscall_resolve_name(b'clone'),ctypes.c_ulong(flag|17),0,0,0,0)==-1
    assert ctypes.get_errno()==errno.EPERM
""")

    @unittest.skipUnless(platform.machine() == 'x86_64', 'x32 escape is specific to x86_64')
    def test_x32_alternate_abi_kills_the_child_and_does_not_publish_success(self):
        result = self.result("import ctypes; c=ctypes.CDLL(None); c.syscall.restype=ctypes.c_long; c.syscall(0x40000000+41,2,1,0); raise AssertionError('alternate ABI survived')")
        self.assertEqual(result['exit_code'], -31)
        self.assertFalse(result['outputs'])

    def test_inheritable_network_handle_and_injection_variables_cannot_reach_command(self):
        # Root supervisor descriptors are not passed to the fixed launcher.
        with socket.socket() as network:
            network.set_inheritable(True)
            result = self.passed("""
import errno,os,pathlib,socket
for key in ('LD_PRELOAD','LD_LIBRARY_PATH','PYTHONPATH','ION_TEST_SECRET','ION_JOB_ISOLATED','ION_JOB_CHILD_UID','ION_JOB_CHILD_GID'):
    assert key not in os.environ
for n in os.listdir('/proc/self/fd'):
    if int(n)>2:
        try: os.fstat(int(n))
        except OSError as e: assert e.errno==errno.EBADF
        else: raise AssertionError('inherited descriptor escaped')
pathlib.Path('report.bin').write_bytes(b'kernel-checked')
""", ['report.bin'], (network.fileno(),))
        self.assertEqual(base64.b64decode(result['outputs'][0]['content_base64']), b'kernel-checked')

    def test_missing_library_fails_before_user_code_and_preserves_failure_receipt(self):
        directory = '/usr/lib/x86_64-linux-gnu' if platform.machine() == 'x86_64' else '/usr/lib/aarch64-linux-gnu'
        library = Path(directory) / 'libseccomp.so.2'
        backup = library.with_name('libseccomp.so.2.ion-test-backup')
        self.assertTrue(library.exists())
        self.assertFalse(backup.exists())
        library.rename(backup)
        try:
            result = self.result("import pathlib; pathlib.Path('escaped').write_text('ran')", ['escaped'])
        finally:
            backup.rename(library)
        self.assertEqual(result['exit_code'], 125)
        self.assertFalse(result['outputs'])
        self.assertEqual(len(result['output_errors']), 1)
        self.assertIn(b'command was not started', base64.b64decode(result['stderr']['content_base64']))

    def test_unapproved_kernel_release_fails_before_user_code(self):
        policy = Path('/usr/local/libexec/ion-job-kernels.json')
        original = policy.read_bytes()
        policy.write_text(json.dumps({'version':1,'releases':['unapproved-kernel']}))
        try:
            result = self.result("import pathlib; pathlib.Path('escaped').write_text('ran')", ['escaped'])
        finally:
            policy.write_bytes(original)
        self.assertEqual(result['exit_code'], 125)
        self.assertFalse(result['outputs'])
        self.assertIn(b'command was not started', base64.b64decode(result['stderr']['content_base64']))

    def test_nonzero_command_outputs_and_logs_remain_authentic(self):
        result = self.result("import pathlib,sys; pathlib.Path('checkpoint.bin').write_bytes(b'failed checkpoint'); print('actual failure',file=sys.stderr); sys.exit(9)", ['checkpoint.bin'])
        self.assertEqual(result['exit_code'], 9)
        self.assertEqual(base64.b64decode(result['outputs'][0]['content_base64']), b'failed checkpoint')
        self.assertEqual(base64.b64decode(result['stderr']['content_base64']).strip(), b'actual failure')
    def test_private_result_volume_is_unreachable_to_child_and_exclusive_for_collector(self):
        request = {'version': 1, 'request_id': str(uuid.uuid4()),
                   'argv': [sys.executable, '-c', """
import errno,pathlib,sys
try: pathlib.Path('/ion/results/forged').write_text('forged')
except OSError as e: assert e.errno==errno.EACCES
else: raise AssertionError('result mount escaped')
pathlib.Path('checkpoint.bin').write_bytes(b'guarded failure')
sys.exit(9)
"""], 'timeout_seconds': 4, 'inputs': [], 'output_paths': ['checkpoint.bin']}
        request_file = Path('/ion/job-request.json')
        volume = Path('/ion/results')
        self.assertFalse(request_file.exists())
        volume.mkdir(mode=0o700, exist_ok=True)
        request_file.write_text(json.dumps(request))
        env = {'PATH': os.environ['PATH'], 'ION_JOB_ISOLATED':'1',
               'ION_JOB_CHILD_UID':'1000', 'ION_JOB_CHILD_GID':'1000'}
        args = [WORKER, '--request-file', str(request_file), '--result-volume', str(volume)]
        try:
            first = subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env, timeout=15)
            self.assertEqual(first.returncode, 0, first.stdout + first.stderr)
            self.assertTrue(json.loads(first.stdout)['result_written'])
            receipt = volume / request['request_id'] / 'receipt.json'
            saved = receipt.read_bytes()
            result = json.loads(saved)
            self.assertEqual(result['exit_code'], 9)
            self.assertEqual(base64.b64decode(result['outputs'][0]['content_base64']), b'guarded failure')
            self.assertFalse((volume/'forged').exists())
            repeated = subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env, timeout=15)
            self.assertEqual(repeated.returncode, 1)
            self.assertEqual(receipt.read_bytes(), saved)
        finally:
            request_file.unlink()


if __name__ == '__main__':
    unittest.main()
