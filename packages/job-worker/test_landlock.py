"""Pure contract/failure tests and opt-in real Linux Landlock process tests."""
import ctypes
import errno
import importlib.util
import json
import os
import platform
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('ion_landlock', Path(__file__).with_name('ion_job_landlock.py'))
landlock = importlib.util.module_from_spec(spec)
spec.loader.exec_module(landlock)
READS = ['/usr', '/etc/ld.so.cache']
WRITES = ['/tmp/ion-private/work', '/tmp/ion-private/home', '/tmp/ion-private/tmp']
DEVICE = {'path': '/dev/null', 'type': 'char', 'major': 1, 'minor': 3,
          'read': True, 'write': True, 'ioctl': False}


class PlanTests(unittest.TestCase):
    def test_valid_contract_is_frozen_without_aliasing_owner_device_input(self):
        device = dict(DEVICE)
        reads, writes, nodes = landlock.plan(READS, WRITES, [device])
        device['read'] = False
        self.assertEqual(reads, tuple(READS))
        self.assertEqual(writes, tuple(WRITES))
        self.assertTrue(nodes[0]['read'])

    def test_absolute_canonical_byte_and_depth_bounds(self):
        for value in ['/', '', 'usr', '/usr/', '//usr', '/usr/../usr', '/usr/./lib',
                      '/usr\x00/lib', '/usr\n/lib', '/usr\\lib', '/x/' + 'Ã©' * 2048,
                      '/' + '/'.join('x' for _ in range(65)), '/\ud800']:
            with self.subTest(value=ascii(value)):
                with self.assertRaises(landlock.BoundaryError):
                    landlock.plan([value], WRITES, [DEVICE])
        landlock.plan(['/usr/' + 'a' * 4091], WRITES, [])
        with self.assertRaises(landlock.BoundaryError):
            landlock.plan(['/usr/' + 'a' * 4092], WRITES, [])

    def test_collection_and_strict_device_types_are_bounded(self):
        for reads, writes, devices in [(iter(READS), WRITES, []), ([], WRITES, []),
                                      (READS, WRITES[:2], []), (READS, WRITES + ['/tmp/other'], []),
                                      (['/img/x' + str(i) for i in range(129)], WRITES, []),
                                      (READS, WRITES, [DEVICE] * 65)]:
            with self.assertRaises(landlock.BoundaryError):
                landlock.plan(reads, writes, devices)
        for key, value in [('read', 1), ('write', 0), ('ioctl', 'true'), ('type', 'block'),
                           ('major', True), ('minor', -1), ('major', 2**32), ('path', '/proc/device')]:
            with self.subTest(key=key, value=value):
                with self.assertRaises(landlock.BoundaryError):
                    landlock.plan(READS, WRITES, [{**DEVICE, key: value}])
        for value in [{**DEVICE, 'extra': True},
                      {k: v for k, v in DEVICE.items() if k != 'minor'},
                      {**DEVICE, 'read': False, 'write': False, 'ioctl': False}, None]:
            with self.assertRaises(landlock.BoundaryError):
                landlock.plan(READS, WRITES, [value])

    def test_protected_collector_services_and_global_scratch_are_never_granted(self):
        for path in ['/ion', '/ion/results', '/proc', '/proc/self/status', '/sys',
                     '/run', '/run/service/data', '/dev', '/dev/shm/file', '/tmp', '/tmp/foreign']:
            with self.subTest(path=path):
                with self.assertRaises(landlock.BoundaryError):
                    landlock.plan([path], WRITES, [])
        for path in ['/ion/job', '/run/tmp', '/dev/shm/private', '/proc/x', '/sys/x',
                     '/usr/private', '/etc/private', '/var/tmp/private', '/tmp', '/home']:
            with self.assertRaises(landlock.BoundaryError):
                landlock.plan(READS, [path, WRITES[1], WRITES[2]], [])
        # Component comparison must not confuse similar names with protected paths.
        landlock.plan(['/ion-image'], ['/tmp/a/work', '/tmp/a/home', '/tmp/a/tmp'], [])

    def test_overlapping_roles_cannot_broaden_exact_device_or_write_permissions(self):
        for reads, writes, devices in [
                (['/usr', '/usr/lib'], WRITES, []),
                (READS, [WRITES[0], '/tmp/ion-private', WRITES[2]], []),
                (READS, [WRITES[0], WRITES[0], WRITES[2]], []),
                (['/image'], WRITES, [{**DEVICE, 'path': '/image/device'}]),
                (READS, WRITES, [DEVICE, DEVICE])]:
            with self.assertRaises(landlock.BoundaryError):
                landlock.plan(reads, writes, devices)

    def test_only_explicit_static_driver_and_host_metadata_paths_are_accepted(self):
        accepted = ['/proc/driver/nvidia', '/proc/driver/nvidia/gpus/0000:01:00.0/information',
                    '/proc/driver/nvidia-caps/mig/config', '/proc/devices', '/proc/cpuinfo',
                    '/sys/module/nvidia/initstate', '/sys/devices/soc0/family',
                    '/sys/devices/system/cpu/online', '/sys/devices/system/node/node0/cpulist',
                    '/sys/devices/pci0000:00/0000:00:01.0/vendor']
        for path in accepted:
            self.assertEqual(landlock.read_path(path), path)
            landlock.plan([path], WRITES, [DEVICE])
        denied = ['/proc', '/proc/driver', '/proc/driver/nvidia-other', '/proc/self/status',
                  '/proc/self/fd', '/proc/thread-self/status', '/proc/123/fd/0',
                  '/proc/123/fdinfo', '/proc/123/map_files', '/proc/123/mem',
                  '/proc/driver/nvidia/fd', '/proc/driver/nvidia/gpus/mem',
                  '/proc/sys', '/proc/net', '/sys', '/sys/devices', '/sys/module',
                  '/sys/kernel/debug', '/sys/kernel/security', '/sys/fs/cgroup',
                  '/sys/devices/pci0000:00/0000:00:01.0',
                  '/sys/devices/pci0000:00/0000:00:01.0/resource0',
                  '/sys/devices/pci0000:00/0000:00:01.0/config',
                  '/sys/devices/pci-foreign/vendor', '/sys/class/drm/card0/device/vendor']
        for path in denied:
            with self.subTest(path=path):
                with self.assertRaises(landlock.BoundaryError):
                    landlock.plan([path], WRITES, [DEVICE])

    def test_masks_handle_denied_special_creation_and_minimal_ioctl(self):
        denied = (1 << 6) | (1 << 9) | (1 << 10) | (1 << 11) | (1 << 12)
        self.assertEqual(landlock.HANDLED_FS & denied, denied)
        self.assertEqual(landlock.WRITE_TREE & (denied | landlock.IOCTL_DEV), 0)
        self.assertEqual(landlock.READ_TREE & (landlock.WRITE_FILE | landlock.TRUNCATE | landlock.IOCTL_DEV), 0)
        self.assertEqual(landlock.WRITE_TREE & (landlock.REFER | landlock.TRUNCATE),
                         landlock.REFER | landlock.TRUNCATE)
        self.assertEqual(ctypes.sizeof(landlock.Ruleset), 8)
        self.assertEqual(ctypes.sizeof(landlock.PathRule), 12)

    def test_unsupported_platform_has_no_kernel_fallback(self):
        with patch.object(landlock.sys, 'platform', 'unsupported'):
            with self.assertRaises(landlock.BoundaryError):
                landlock.install(READS, WRITES, [DEVICE])


class InstallFailureTests(unittest.TestCase):
    def fixture(self, *, fail=None, abi=6, info_override=None):
        seen = {'closed': [], 'rules': [], 'restricted': False}
        class Kernel:
            def abi(self):
                if abi < 5:
                    raise landlock.BoundaryError()
                return abi
            def create(self):
                if fail == 'create':
                    raise landlock.BoundaryError()
                return 100
            def add(self, ruleset, descriptor, rights):
                seen['rules'].append((ruleset, descriptor, rights))
                if fail == 'add':
                    raise landlock.BoundaryError()
            def restrict(self, ruleset):
                if fail == 'restrict':
                    raise landlock.BoundaryError()
                seen['restricted'] = True
        paths = {}
        def open_path(path, image=False):
            if fail == 'open':
                raise OSError(errno.EACCES, 'fixture')
            fd = len(paths) + 101
            paths[fd] = path
            return fd
        def info(fd):
            path = paths[fd]
            if info_override:
                override = info_override(path)
                if override:
                    return override
            if path in WRITES:
                return types.SimpleNamespace(st_mode=stat.S_IFDIR | 0o700, st_uid=1000, st_gid=1000)
            if path == '/dev/null':
                return types.SimpleNamespace(st_mode=stat.S_IFCHR | 0o666, st_uid=0, st_rdev=0)
            return types.SimpleNamespace(st_mode=stat.S_IFDIR | 0o755, st_uid=0)
        stack = [
            patch.object(landlock, '_Kernel', Kernel),
            patch.object(landlock, '_open_path', open_path),
            patch.object(landlock.os, 'geteuid', return_value=1000, create=True),
            patch.object(landlock.os, 'getegid', return_value=1000, create=True),
            patch.object(landlock.os, 'fstat', side_effect=info),
            patch.object(landlock.os, 'major', return_value=1, create=True),
            patch.object(landlock.os, 'minor', return_value=3, create=True),
            patch.object(landlock.os, 'close', side_effect=seen['closed'].append),
        ]
        return seen, stack

    def run_fixture(self, **options):
        from contextlib import ExitStack
        seen, patches = self.fixture(**options)
        with ExitStack() as stack:
            for item in patches:
                stack.enter_context(item)
            try:
                result = landlock.install(READS, WRITES, [DEVICE])
            except landlock.BoundaryError:
                result = None
        return result, seen

    def test_returns_actual_abi_only_after_restrict_and_all_setup_fds_close(self):
        result, seen = self.run_fixture(abi=8)
        self.assertEqual(result, 8)
        self.assertTrue(seen['restricted'])
        self.assertEqual(sorted(seen['closed']), list(range(100, 107)))
        self.assertEqual(seen['rules'][-1][2], landlock.READ_FILE | landlock.WRITE_FILE)

    def test_query_missing_or_old_create_add_open_restrict_fail_without_success(self):
        for options, expected_closed in [
                ({'abi': 4}, []), ({'fail': 'create'}, []),
                ({'fail': 'open'}, [100]), ({'fail': 'add'}, [100, 101]),
                ({'fail': 'restrict'}, list(range(100, 107)))]:
            with self.subTest(options=options):
                result, seen = self.run_fixture(**options)
                self.assertIsNone(result)
                self.assertFalse(seen['restricted'])
                self.assertEqual(sorted(seen['closed']), expected_closed)

    def test_objects_must_have_expected_type_owner_mode_and_device_identity(self):
        cases = [
            ('/usr', types.SimpleNamespace(st_mode=stat.S_IFDIR | 0o777, st_uid=0)),
            ('/usr', types.SimpleNamespace(st_mode=stat.S_IFREG | 0o644, st_uid=1000)),
            (WRITES[0], types.SimpleNamespace(st_mode=stat.S_IFDIR | 0o755, st_uid=1000, st_gid=1000)),
            (WRITES[0], types.SimpleNamespace(st_mode=stat.S_IFDIR | 0o700, st_uid=0, st_gid=1000)),
            ('/dev/null', types.SimpleNamespace(st_mode=stat.S_IFBLK | 0o666, st_uid=0, st_rdev=0)),
            ('/dev/null', types.SimpleNamespace(st_mode=stat.S_IFCHR | 0o666, st_uid=1000, st_rdev=0)),
        ]
        for path, info in cases:
            result, seen = self.run_fixture(info_override=lambda actual: info if actual == path else None)
            self.assertIsNone(result)
            self.assertFalse(seen['restricted'])
            self.assertIn(100, seen['closed'])
        seen, patches = self.fixture()
        from contextlib import ExitStack
        with ExitStack() as stack:
            for item in patches:
                stack.enter_context(item)
            stack.enter_context(patch.object(landlock.os, 'minor', return_value=4, create=True))
            with self.assertRaises(landlock.BoundaryError):
                landlock.install(READS, WRITES, [DEVICE])
        self.assertIn(100, seen['closed'])

    def test_close_all_attempts_remaining_fds_after_error(self):
        closed = []
        def close(fd):
            closed.append(fd)
            if fd == 3:
                raise OSError(errno.EBADF, 'fixture')
        with patch.object(landlock.os, 'close', side_effect=close):
            with self.assertRaises(landlock.BoundaryError):
                landlock._close_all([1, 2, 3, 4])
        self.assertEqual(closed, [4, 3, 2, 1])



class KernelUapiTests(unittest.TestCase):
    def kernel(self, values, prctl=lambda *args: 1 if args[0] == 39 else 0):
        kernel = object.__new__(landlock._Kernel)
        kernel.create_number, kernel.add_number, kernel.restrict_number = (444, 445, 446)
        calls = []
        def syscall(*args):
            calls.append(args)
            return values.pop(0)
        kernel.libc = types.SimpleNamespace(syscall=syscall, prctl=prctl)
        return kernel, calls

    def test_version_query_returns_kernel_value_and_never_accepts_old_disabled_missing(self):
        for value in [-1, 0, 1, 4]:
            kernel, calls = self.kernel([value])
            with self.assertRaises(landlock.BoundaryError):
                kernel.abi()
            self.assertEqual(calls[0][0].value, 444)
            self.assertIsNone(calls[0][1].value)
            self.assertEqual(calls[0][2].value, 0)
            self.assertEqual(calls[0][3].value, 1)
        kernel, _ = self.kernel([9])
        self.assertEqual(kernel.abi(), 9)

    def test_ruleset_and_packed_path_parameters_are_actual_uapi_layouts(self):
        kernel, calls = self.kernel([100, 0, 0])
        self.assertEqual(kernel.create(), 100)
        kernel.add(100, 101, landlock.IOCTL_DEV | landlock.READ_FILE)
        kernel.restrict(100)
        self.assertEqual(calls[0][2].value, 8)
        self.assertEqual(calls[0][1]._obj.handled_access_fs, 65535)
        self.assertEqual(calls[1][0].value, 445)
        self.assertEqual(calls[1][1].value, 100)
        self.assertEqual(calls[1][2].value, 1)
        self.assertEqual(calls[1][3]._obj.parent_fd, 101)
        self.assertEqual(calls[1][3]._obj.allowed_access, landlock.IOCTL_DEV | landlock.READ_FILE)
        self.assertEqual(calls[2][0].value, 446)

    def test_create_add_nnp_verification_and_restrict_errors_are_not_ignored(self):
        for method in ['create', 'add', 'restrict']:
            kernel, _ = self.kernel([-1])
            with self.assertRaises(landlock.BoundaryError):
                if method == 'create':
                    kernel.create()
                elif method == 'add':
                    kernel.add(100, 101, landlock.READ_FILE)
                else:
                    kernel.restrict(100)
        for prctl in [lambda *args: -1, lambda *args: 0]:
            kernel, calls = self.kernel([0], prctl=prctl)
            with self.assertRaises(landlock.BoundaryError):
                kernel.restrict(100)
            self.assertEqual(calls, [])


class PathDescriptorTests(unittest.TestCase):
    def run_path(self, *, fail=None, final_mode=stat.S_IFDIR | 0o755):
        from contextlib import ExitStack
        opened, closed = [], []
        def open_fd(path, flags, dir_fd=None):
            if path == 'image' and fail == 'open':
                raise OSError(errno.EACCES, 'fixture')
            fd = len(opened) + 10
            opened.append((fd, path, flags, dir_fd))
            return fd
        def info(fd):
            if fd == 12:
                return types.SimpleNamespace(st_mode=final_mode, st_uid=0)
            return types.SimpleNamespace(st_mode=stat.S_IFDIR | 0o755, st_uid=0)
        with ExitStack() as stack:
            for name, value in [('O_PATH', 0x200000), ('O_NOFOLLOW', 0x20000),
                                ('O_CLOEXEC', 0x80000), ('O_DIRECTORY', 0x10000)]:
                stack.enter_context(patch.object(landlock.os, name, value, create=True))
            stack.enter_context(patch.object(landlock.os, 'open', side_effect=open_fd))
            stack.enter_context(patch.object(landlock.os, 'fstat', side_effect=info))
            stack.enter_context(patch.object(landlock.os, 'close', side_effect=closed.append))
            try:
                fd = landlock._open_path('/usr/image', image=True)
            except (landlock.BoundaryError, OSError):
                fd = None
        return fd, opened, closed

    def test_every_component_is_nofollow_and_parents_are_closed_after_leaf_is_pinned(self):
        fd, opened, closed = self.run_path()
        self.assertEqual(fd, 12)
        self.assertEqual(closed, [11, 10])
        self.assertEqual([entry[3] for entry in opened], [None, 10, 11])
        self.assertTrue(all(entry[2] & 0x20000 and entry[2] & 0x200000 for entry in opened))
        self.assertTrue(opened[0][2] & 0x10000 and opened[1][2] & 0x10000)
        self.assertFalse(opened[2][2] & 0x10000)

    def test_failed_open_or_symlink_closes_every_acquired_component(self):
        fd, _, closed = self.run_path(fail='open')
        self.assertIsNone(fd)
        self.assertEqual(closed, [11, 10])
        fd, _, closed = self.run_path(final_mode=stat.S_IFLNK | 0o777)
        self.assertIsNone(fd)
        self.assertEqual(closed, [12, 11, 10])


# Every irreversible installation happens in a separate disposable process. This
# suite intentionally never installs Landlock in the test runner or on Windows.
@unittest.skipUnless(sys.platform == 'linux' and os.environ.get('ION_TEST_LANDLOCK') == '1',
                     'Opt-in disposable Linux ABI>=5 image required')
class LinuxLandlockTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        abi = landlock._Kernel().abi()
        print('Real Landlock: kernel=' + platform.release() + ', architecture=' +
              platform.machine() + ', ABI=' + str(abi), flush=True)

    def child(self, operation, *, ioctl=False, expected=0):
        with tempfile.TemporaryDirectory(prefix='ion-landlock-test-') as base:
            os.chmod(base, 0o755)
            roots = [str(Path(base) / name) for name in ('work', 'home', 'tmp')]
            for root in roots:
                os.mkdir(root, 0o700)
                if os.geteuid() == 0:
                    os.chown(root, 1000, 1000)
            denied = Path(base) / 'collector'
            denied.write_text('collector-only-fixture', encoding='utf-8')
            denied.chmod(0o644)
            outside_executable = Path(base) / 'outside-executable'
            outside_executable.write_bytes(Path('/usr/bin/true').read_bytes())
            outside_executable.chmod(0o755)
            uid, gid = (1000, 1000) if os.geteuid() == 0 else (os.geteuid(), os.getegid())
            device = dict(DEVICE, ioctl=ioctl)
            info = os.stat('/dev/null')
            device['major'], device['minor'] = os.major(info.st_rdev), os.minor(info.st_rdev)
            script = (
                "import ctypes,errno,fcntl,importlib.util,json,os,pathlib,stat,subprocess,sys,threading\n"
                "spec=importlib.util.spec_from_file_location('ll'," + repr(str(Path(landlock.__file__).resolve())) + ")\n"
                "ll=importlib.util.module_from_spec(spec); spec.loader.exec_module(ll)\n"
                "if os.geteuid()==0: os.setgroups([]); os.setresgid(" + repr(gid) + "," + repr(gid) + "," + repr(gid) + "); os.setresuid(" + repr(uid) + "," + repr(uid) + "," + repr(uid) + ")\n"
                "roots=" + repr(roots) + "\n"
                "denied=" + repr(str(denied)) + "\n"
                "outside_executable=" + repr(str(outside_executable)) + "\n"
                "device=" + repr(device) + "\n"
                "def denied_op(fn):\n"
                " try: fn()\n"
                " except OSError as e: assert e.errno in (errno.EACCES,errno.EPERM),e\n"
                " else: raise AssertionError('filesystem boundary escaped')\n"
                + operation
            )
            result = subprocess.run([sys.executable, '-I', '-c', script],
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=20)
            self.assertEqual(result.returncode, expected, result.stdout + result.stderr)

    def test_readonly_image_private_file_io_and_collector_denial(self):
        self.child("""
abi=ll.install(['/usr','/etc/ld.so.cache'],roots,[device]); assert abi>=5
p=pathlib.Path(roots[0])/'regular'; p.write_bytes(b'hello')
assert p.read_bytes()==b'hello'; os.truncate(p,3); assert p.read_bytes()==b'hel'
q=pathlib.Path(roots[1])/'linked'; os.link(p,q); assert q.read_bytes()==b'hel'
os.rename(q,str(pathlib.Path(roots[2])/'moved'))
denied_op(lambda:open(denied,'rb'))
denied_op(lambda:open('/etc/passwd','rb'))
denied_op(lambda:open('/usr/ion-child-write-marker','wb'))
denied_op(lambda:open('/tmp/ion-child-write-marker','wb'))
denied_op(lambda:os.link(denied,str(pathlib.Path(roots[0])/'escape')))
denied_op(lambda:os.mkfifo(str(pathlib.Path(roots[0])/'fifo')))
denied_op(lambda:os.symlink(denied,str(pathlib.Path(roots[0])/'symlink')))
denied_op(lambda:os.mknod(str(pathlib.Path(roots[0])/'device'),stat.S_IFCHR|0o600,os.makedev(1,3)))
assert subprocess.run([sys.executable,'-I','-c','pass']).returncode==0
p.unlink()
""")

    def test_explicit_proc_sys_metadata_is_readable_without_blanket_kernel_view(self):
        self.child("""
reads=['/usr','/etc/ld.so.cache','/proc/cpuinfo','/sys/devices/system/cpu/online']
ll.install(reads,roots,[device])
assert pathlib.Path('/proc/cpuinfo').read_bytes()
assert pathlib.Path('/sys/devices/system/cpu/online').read_bytes()
denied_op(lambda:open('/proc/meminfo','rb'))
denied_op(lambda:open('/proc/self/status','rb'))
denied_op(lambda:open('/sys/kernel/uevent_seqnum','rb'))
denied_op(lambda:open('/proc/cpuinfo','wb'))
# Docker mounts sysfs readonly: VFS may reject before the LSM write check.
try: open('/sys/devices/system/cpu/online','wb')
except OSError as e: assert e.errno in (errno.EACCES,errno.EPERM,errno.EROFS),e
else: raise AssertionError('sysfs metadata became writable')
""")

    def test_device_driver_ioctl_is_denied_without_approved_right(self):
        self.child("""
ll.install(['/usr','/etc/ld.so.cache'],roots,[device])
fd=os.open('/dev/null',os.O_RDWR)
denied_op(lambda:fcntl.ioctl(fd,0xdeadbeef,0))
os.close(fd)
""")

    def test_exact_device_ioctl_grant_reaches_driver_not_permission_denial(self):
        self.child("""
ll.install(['/usr','/etc/ld.so.cache'],roots,[device])
fd=os.open('/dev/null',os.O_RDWR)
try: fcntl.ioctl(fd,0xdeadbeef,0)
except OSError as e: assert e.errno==errno.ENOTTY,e
else: raise AssertionError('null unexpectedly implements fixture ioctl')
os.close(fd)
""", ioctl=True)

    def test_denied_opens_survive_threads_fork_and_exec(self):
        self.child(r"""
ll.install(['/usr','/etc/ld.so.cache'],roots,[device])
seen=[]
def thread():
 try: open(denied,'rb')
 except OSError as e: seen.append(e.errno)
t=threading.Thread(target=thread); t.start(); t.join()
assert seen==[errno.EACCES],seen
pid=os.fork()
if pid==0:
 try: open(denied,'rb')
 except OSError as e: os._exit(0 if e.errno==errno.EACCES else 8)
 os._exit(9)
assert os.waitpid(pid,0)[1]==0
code='import errno;\ntry: open('+repr(denied)+', "rb")\nexcept OSError as e: assert e.errno==errno.EACCES\nelse: raise AssertionError()'
assert subprocess.run([sys.executable,'-I','-c',code]).returncode==0
""")

    def test_preopened_file_descriptor_demonstrates_required_guard_closure(self):
        self.child("""
fd=os.open(denied,os.O_RDONLY)
ll.install(['/usr','/etc/ld.so.cache'],roots,[device])
denied_op(lambda:open(denied,'rb'))
denied_op(lambda:os.open('/proc/self/fd/'+str(fd),os.O_RDONLY))
# Kernel explicitly preserves this prior FD's rights. The fixed guard must close
# it before applying the domain; this fixture documents that required premise.
assert os.read(fd,64)==b'collector-only-fixture'
os.close(fd)
""")

    def test_execve_by_fd_checks_execute_permission_even_for_preopened_outside_file(self):
        self.child("""
outside=os.open(outside_executable,os.O_RDONLY)
ll.install(['/usr','/etc/ld.so.cache'],roots,[device])
assert os.execve in os.supports_fd
pid=os.fork()
if pid==0:
 try: os.execve(outside,['outside-executable'],dict(os.environ))
 except OSError as e: os._exit(0 if e.errno==errno.EACCES else 8)
 os._exit(9)
assert os.waitpid(pid,0)[1]==0
os.close(outside)
approved=os.open('/usr/bin/true',os.O_RDONLY)
pid=os.fork()
if pid==0:
 os.execve(approved,['true'],dict(os.environ))
 os._exit(9)
assert os.waitpid(pid,0)[1]==0
os.close(approved)
""")

    def test_root_symlink_cannot_become_a_rule_or_leak_setup_fds(self):
        self.child("""
alias=pathlib.Path(roots[0])/'alias'; os.symlink(roots[0],alias)
before=set(os.listdir('/proc/self/fd'))
try: ll.install(['/usr','/etc/ld.so.cache'],[str(alias),roots[1],roots[2]],[device])
except ll.BoundaryError: pass
else: raise AssertionError('symlink rule accepted')
assert set(os.listdir('/proc/self/fd'))==before
nested=pathlib.Path(roots[0])/'nested'; nested.mkdir()
try: ll.install(['/usr','/etc/ld.so.cache'],[str(alias/'nested'),roots[1],roots[2]],[device])
except ll.BoundaryError: pass
else: raise AssertionError('ancestor symlink rule accepted')
assert set(os.listdir('/proc/self/fd'))==before
""")


if __name__ == '__main__':
    unittest.main()
