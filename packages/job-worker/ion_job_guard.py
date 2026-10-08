#!/usr/bin/env python3
"""Internal fixed child launcher; no user code loads before the kernel filter."""
import ctypes
import errno
import json
import os
from pathlib import Path
import platform
import re
import stat
import sys

CONTRACT = 'linux-seccomp-socket-deny/v1'
ALLOW = 0x7fff0000
KILL_PROCESS = 0x80000000
ERRNO = 0x00050000
NETWORK_CALLS = (
    'socket', 'socketpair', 'connect', 'bind', 'listen', 'accept', 'accept4',
    'sendto', 'sendmsg', 'sendmmsg', 'recvfrom', 'recvmsg', 'recvmmsg', 'shutdown',
    'getsockopt', 'setsockopt', 'getpeername', 'getsockname',
)
ESCAPE_CALLS = (
    'ptrace', 'process_vm_readv', 'process_vm_writev', 'pidfd_getfd',
    'bpf', 'setns', 'unshare',
)
RING_CALLS = ('io_uring_setup', 'io_uring_enter', 'io_uring_register')
NEW_NAMESPACES = (0x80, 0x00020000, 0x02000000, 0x04000000,
                  0x08000000, 0x10000000, 0x20000000, 0x40000000)
ARCH = {'x86_64': 0xc000003e, 'aarch64': 0xc00000b7}


class BoundaryError(Exception):
    pass


class Comparison(ctypes.Structure):
    _fields_ = [('arg', ctypes.c_uint), ('op', ctypes.c_int),
                ('datum_a', ctypes.c_uint64), ('datum_b', ctypes.c_uint64)]


class Version(ctypes.Structure):
    _fields_ = [('major', ctypes.c_uint), ('minor', ctypes.c_uint), ('micro', ctypes.c_uint)]


def trusted_file(path):
    # Follow only into root-owned, non-writable image paths. The child cannot replace
    # this library/launcher or discover a library from its workspace.
    path = path.resolve(strict=True)
    info = path.stat()
    if not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
        raise BoundaryError()
    for parent in path.parents:
        info = parent.stat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
            raise BoundaryError()
    return path


def library():
    machine = platform.machine()
    directories = {'x86_64': 'x86_64-linux-gnu', 'aarch64': 'aarch64-linux-gnu'}
    if machine not in directories:
        raise BoundaryError()
    lib = None
    for prefix in ('/usr/lib', '/lib'):
        path = Path(prefix) / directories[machine] / 'libseccomp.so.2'
        if path.exists():
            lib = ctypes.CDLL(str(trusted_file(path)), use_errno=True)
            break
    if lib is None:
        raise BoundaryError()
    declarations = {
        'seccomp_version': ([], ctypes.POINTER(Version)),
        'seccomp_api_get': ([], ctypes.c_uint),
        'seccomp_arch_native': ([], ctypes.c_uint32),
        'seccomp_init': ([ctypes.c_uint32], ctypes.c_void_p),
        'seccomp_release': ([ctypes.c_void_p], None),
        'seccomp_attr_set': ([ctypes.c_void_p, ctypes.c_int, ctypes.c_uint32], ctypes.c_int),
        'seccomp_syscall_resolve_name': ([ctypes.c_char_p], ctypes.c_int),
        'seccomp_rule_add_array': ([ctypes.c_void_p, ctypes.c_uint32, ctypes.c_int,
                                    ctypes.c_uint, ctypes.POINTER(Comparison)], ctypes.c_int),
        'seccomp_load': ([ctypes.c_void_p], ctypes.c_int),
    }
    for name, (arguments, result) in declarations.items():
        function = getattr(lib, name)
        function.argtypes, function.restype = arguments, result
    version = lib.seccomp_version()
    if (not version or (version.contents.major, version.contents.minor, version.contents.micro) < (2, 5, 0)
        or lib.seccomp_api_get() < 3 or lib.seccomp_arch_native() != ARCH[machine]):
        raise BoundaryError()
    return lib


def install_filter(lib):
    context = lib.seccomp_init(ALLOW)
    if not context:
        raise BoundaryError()
    def checked(result):
        if result != 0:
            raise BoundaryError()
    def rule(name, error, comparison=None):
        number = lib.seccomp_syscall_resolve_name(name.encode('ascii'))
        # Other negative values are valid libseccomp pseudo syscall identities.
        if number == -1:
            raise BoundaryError()
        checked(lib.seccomp_rule_add_array(context, ERRNO | error, number,
            0 if comparison is None else 1,
            None if comparison is None else ctypes.byref(comparison)))
    try:
        # ACT_BADARCH=2, CTL_NNP=3, CTL_TSYNC=4. Do not add any compatibility ABI.
        checked(lib.seccomp_attr_set(context, 2, KILL_PROCESS))
        checked(lib.seccomp_attr_set(context, 3, 1))
        checked(lib.seccomp_attr_set(context, 4, 1))
        for name in NETWORK_CALLS:
            rule(name, errno.ENETDOWN)
        for name in ESCAPE_CALLS:
            rule(name, errno.EPERM)
        for name in RING_CALLS:
            rule(name, errno.ENOSYS)
        # glibc can fall back to ordinary clone for threads when clone3 is absent.
        rule('clone3', errno.ENOSYS)
        for flag in NEW_NAMESPACES:
            rule('clone', errno.EPERM, Comparison(0, 7, flag, flag))
        checked(lib.seccomp_load(context))
    finally:
        lib.seccomp_release(context)


def install_resources():
    # Old saved jobs omit the policy. New owner launches require it explicitly;
    # this preserves exact historical request/recovery and GPU image contracts.
    encoded = os.environ.pop('ION_JOB_RESOURCE_LIMITS', None)
    if encoded is None:
        return
    if len(encoded) > 1024:
        raise BoundaryError()
    limits = json.loads(encoded)
    if (not isinstance(limits, dict) or set(limits) != {'version', 'cpu_seconds',
        'address_space_bytes', 'max_file_bytes', 'processes', 'open_files'}
        or any(type(v) is not int for v in limits.values()) or limits['version'] != 1
        or not 1 <= limits['cpu_seconds'] <= 120
        or not 64*1024*1024 <= limits['address_space_bytes'] <= 8*1024*1024*1024
        or not 64*1024 <= limits['max_file_bytes'] <= 1024*1024*1024
        or not 1 <= limits['processes'] <= 256 or not 16 <= limits['open_files'] <= 256):
        raise BoundaryError()
    import resource
    for kind, name in [(resource.RLIMIT_CPU, 'cpu_seconds'),
                       (resource.RLIMIT_AS, 'address_space_bytes'),
                       (resource.RLIMIT_FSIZE, 'max_file_bytes'),
                       (resource.RLIMIT_NPROC, 'processes'),
                       (resource.RLIMIT_NOFILE, 'open_files')]:
        value = limits[name]
        _, inherited = resource.getrlimit(kind)
        if inherited != resource.RLIM_INFINITY:
            value = min(value, inherited)
        resource.setrlimit(kind, (value, value))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def prepare(uid, gid):
    if sys.platform != 'linux' or platform.machine() not in ARCH:
        raise BoundaryError()
    release = re.match(r'^(\d+)\.(\d+)', platform.release())
    if not release or tuple(map(int, release.groups())) < (5, 4):
        raise BoundaryError()
    policy = trusted_file(Path('/usr/local/libexec/ion-job-kernels.json'))
    with policy.open('rb') as source:
        data = source.read(4097)
    if len(data) > 4096:
        raise BoundaryError()
    accepted = json.loads(data)
    if (not isinstance(accepted, dict) or set(accepted) != {'version', 'releases'}
        or type(accepted['version']) is not int or accepted['version'] != 1
        or not isinstance(accepted['releases'], list) or not 1 <= len(accepted['releases']) <= 8
        or any(not isinstance(v, str) or not re.fullmatch(r'[A-Za-z0-9_.+\-]{1,128}', v)
               for v in accepted['releases'])
        or len(set(accepted['releases'])) != len(accepted['releases'])
        or platform.release() not in accepted['releases']):
        raise BoundaryError()
    if (uid <= 0 or gid <= 0 or os.getresuid() != (uid, uid, uid)
        or os.getresgid() != (gid, gid, gid) or os.getgroups()):
        raise BoundaryError()
    # Popen closes inherited FDs before exec. Reject alternate standard channels too:
    # only /dev/null stdin and actual stdout/stderr pipes are allowed by this contract.
    for descriptor in (0, 1, 2):
        info = os.fstat(descriptor)
        if descriptor == 0:
            null = os.stat('/dev/null')
            if not stat.S_ISCHR(info.st_mode) or info.st_rdev != null.st_rdev:
                raise BoundaryError()
        elif not stat.S_ISFIFO(info.st_mode):
            raise BoundaryError()
    libc = ctypes.CDLL(None, use_errno=True)
    libc.prctl.argtypes = [ctypes.c_int, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_ulong]
    libc.prctl.restype = ctypes.c_int
    if libc.prctl(38, 1, 0, 0, 0) != 0 or libc.prctl(39, 0, 0, 0, 0) != 1:
        raise BoundaryError()
    status = dict(line.split(':', 1) for line in Path('/proc/self/status').read_text().splitlines() if ':' in line)
    if any(int(status[name].strip(), 16) != 0 for name in ('CapPrm', 'CapEff', 'CapAmb')):
        raise BoundaryError()
    trusted_file(Path(__file__))
    install_resources()
    install_filter(library())
    descriptors = os.listdir('/proc/self/fd')
    if len(descriptors) > 256:
        raise BoundaryError()
    for name in descriptors:
        if int(name) > 2:
            try:
                os.close(int(name))
            except OSError as error:
                if error.errno != errno.EBADF:
                    raise


def main(arguments=None):
    arguments = sys.argv[1:] if arguments is None else arguments
    if arguments == ['--version']:
        print(CONTRACT)
        return 0
    if (len(arguments) < 4 or arguments[2] != '--' or not arguments[0].isdigit()
        or not arguments[1].isdigit() or not arguments[3].startswith('/')):
        sys.stderr.write('Invalid internal child invocation.\n')
        return 2
    try:
        prepare(int(arguments[0]), int(arguments[1]))
        os.execve(arguments[3], arguments[3:], dict(os.environ))
    except (BoundaryError, OSError, ValueError, KeyError, AttributeError):
        sys.stderr.write('Child execution boundary unavailable; command was not started.\n')
        return 125


if __name__ == '__main__':
    raise SystemExit(main())
