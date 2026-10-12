"""Internal Linux file/device boundary for the strict offline GPU profile.

The caller verifies the reviewed mount/device snapshot, demotes, closes inherited
descriptors and calls this once in its single-threaded fixed launcher before exec.
No historical ordinary job invokes this helper.
"""
import ctypes
import os
import platform
import stat
import sys

CONTRACT = 'linux-landlock-file-device/v1'
MIN_ABI = 5
MAX_READ_PATHS = 128
MAX_DEVICES = 64
MAX_PATH_BYTES = 4096
MAX_DEPTH = 64
# Linux UAPI (v6.12): x86_64 and native arm64 use these syscall identities.
SYSCALLS = {'x86_64': (444, 445, 446), 'aarch64': (444, 445, 446)}
EXECUTE = 1 << 0
WRITE_FILE = 1 << 1
READ_FILE = 1 << 2
READ_DIR = 1 << 3
REMOVE_DIR = 1 << 4
REMOVE_FILE = 1 << 5
MAKE_DIR = 1 << 7
MAKE_REG = 1 << 8
REFER = 1 << 13
TRUNCATE = 1 << 14
IOCTL_DEV = 1 << 15
# Handle all filesystem rights through ABI 5, including rights never granted:
# creation of devices, FIFO, sockets and symlinks remains denied.
HANDLED_FS = (1 << 16) - 1
READ_TREE = EXECUTE | READ_FILE | READ_DIR
WRITE_TREE = READ_TREE | WRITE_FILE | REMOVE_DIR | REMOVE_FILE | MAKE_DIR | MAKE_REG | REFER | TRUNCATE
DEVICE_KEYS = {'path', 'type', 'major', 'minor', 'read', 'write', 'ioctl'}
DENIED_TREES = ('/ion', '/proc', '/sys', '/run', '/dev')
IMAGE_TREES = ('/usr', '/etc', '/lib', '/lib64', '/bin', '/sbin', '/opt', '/boot', '/root', '/var')


class BoundaryError(Exception):
    """The required kernel boundary could not be established."""


class Ruleset(ctypes.Structure):
    # Passing only the first UAPI member keeps the same strict FS policy on ABI 5+.
    _fields_ = [('handled_access_fs', ctypes.c_uint64)]


class PathRule(ctypes.Structure):
    _pack_ = 1  # UAPI explicitly packs this structure: 8-byte mask + 4-byte FD.
    _fields_ = [('allowed_access', ctypes.c_uint64), ('parent_fd', ctypes.c_int32)]


def _path(value):
    if (type(value) is not str or not value.startswith('/') or value == '/'
            or '\\' in value or any(ord(c) < 32 or ord(c) == 127 for c in value)):
        raise BoundaryError()
    try:
        encoded = value.encode('utf-8')
    except UnicodeError:
        raise BoundaryError() from None
    parts = value[1:].split('/')
    if len(encoded) > MAX_PATH_BYTES or len(parts) > MAX_DEPTH or any(p in ('', '.', '..') for p in parts):
        raise BoundaryError()
    return value


def _beneath(path, parent):
    return path == parent or path.startswith(parent + '/')


def _overlap(left, right):
    return _beneath(left, right) or _beneath(right, left)


def plan(read_paths, writable_roots, devices):
    """Validate and freeze the bounded owner inputs; performs no I/O."""
    if (type(read_paths) not in (list, tuple) or not 1 <= len(read_paths) <= MAX_READ_PATHS
            or type(writable_roots) not in (list, tuple) or len(writable_roots) != 3
            or type(devices) not in (list, tuple) or len(devices) > MAX_DEVICES):
        raise BoundaryError()
    reads = tuple(_path(value) for value in read_paths)
    writes = tuple(_path(value) for value in writable_roots)
    nodes = []
    for device in devices:
        if (type(device) is not dict or set(device) != DEVICE_KEYS or device['type'] != 'char'
                or any(type(device[key]) is not bool for key in ('read', 'write', 'ioctl'))
                or not any(device[key] for key in ('read', 'write', 'ioctl'))
                or any(type(device[key]) is not int or not 0 <= device[key] <= 0xffffffff
                       for key in ('major', 'minor'))):
            raise BoundaryError()
        node = dict(device)
        node['path'] = _path(node['path'])
        if any(_beneath(node['path'], root) for root in ('/ion', '/proc', '/sys', '/run', '/tmp')):
            raise BoundaryError()
        nodes.append(node)
    for path in reads:
        if path in ('/tmp', '/home') or any(_beneath(path, root) for root in DENIED_TREES + ('/tmp',)):
            raise BoundaryError()
    for path in writes:
        if path in ('/tmp', '/home') or any(_overlap(path, root) for root in DENIED_TREES + IMAGE_TREES):
            raise BoundaryError()
    # Rules are additive: an ancestor read grant would override an exact device's
    # read=False. Reject overlaps across all roles rather than silently broadening.
    paths = reads + writes + tuple(node['path'] for node in nodes)
    for index, path in enumerate(paths):
        if any(_overlap(path, previous) for previous in paths[:index]):
            raise BoundaryError()
    return reads, writes, tuple(nodes)


class _Kernel:
    def __init__(self):
        machine = platform.machine()
        if (sys.platform != 'linux' or machine not in SYSCALLS
                or ctypes.sizeof(ctypes.c_void_p) != 8 or ctypes.sizeof(ctypes.c_long) != 8
                or sys.byteorder != 'little' or ctypes.sizeof(Ruleset) != 8
                or ctypes.sizeof(PathRule) != 12):
            raise BoundaryError()
        self.create_number, self.add_number, self.restrict_number = SYSCALLS[machine]
        self.libc = ctypes.CDLL(None, use_errno=True)
        self.libc.syscall.argtypes = [ctypes.c_long]
        self.libc.syscall.restype = ctypes.c_long
        self.libc.prctl.argtypes = [ctypes.c_int, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_ulong]
        self.libc.prctl.restype = ctypes.c_int

    def abi(self):
        result = self.libc.syscall(ctypes.c_long(self.create_number), ctypes.c_void_p(),
                                   ctypes.c_size_t(0), ctypes.c_uint(1))
        if result < MIN_ABI:
            raise BoundaryError()
        return result

    def create(self):
        attributes = Ruleset(HANDLED_FS)
        result = self.libc.syscall(ctypes.c_long(self.create_number), ctypes.byref(attributes),
                                   ctypes.c_size_t(ctypes.sizeof(attributes)), ctypes.c_uint(0))
        if not 0 <= result <= 0x7fffffff:
            raise BoundaryError()
        return result

    def add(self, ruleset, path_fd, rights):
        attributes = PathRule(rights, path_fd)
        if self.libc.syscall(ctypes.c_long(self.add_number), ctypes.c_int(ruleset),
                             ctypes.c_int(1), ctypes.byref(attributes), ctypes.c_uint(0)) != 0:
            raise BoundaryError()

    def restrict(self, ruleset):
        if (self.libc.prctl(38, 1, 0, 0, 0) != 0 or self.libc.prctl(39, 0, 0, 0, 0) != 1
                or self.libc.syscall(ctypes.c_long(self.restrict_number),
                                     ctypes.c_int(ruleset), ctypes.c_uint(0)) != 0):
            raise BoundaryError()


def _close_all(descriptors):
    failed = False
    for descriptor in reversed(descriptors):
        try:
            os.close(descriptor)
        except OSError:
            failed = True
    if failed:
        raise BoundaryError()


def _open_path(path, image=False):
    # Pin every component through directory FDs. O_NOFOLLOW on only the full path
    # would still follow an ancestor symlink and race against path replacement.
    flags = os.O_PATH | os.O_NOFOLLOW | os.O_CLOEXEC
    descriptors = []
    keep_final = False
    try:
        descriptors.append(os.open('/', flags | os.O_DIRECTORY))
        components = path[1:].split('/')
        for index, component in enumerate(components):
            parent = os.fstat(descriptors[-1])
            if image and (parent.st_uid != 0 or parent.st_mode & 0o022):
                raise BoundaryError()
            child = os.open(component, flags | (0 if index == len(components) - 1 else os.O_DIRECTORY),
                            dir_fd=descriptors[-1])
            descriptors.append(child)
            if stat.S_ISLNK(os.fstat(child).st_mode):
                raise BoundaryError()
        keep_final = True
        return descriptors[-1]
    finally:
        # Attempt every close even when another close fails. A close failure also
        # closes the would-be returned FD and refuses execution.
        try:
            _close_all(descriptors[:-1] if keep_final else descriptors)
        except BoundaryError:
            if keep_final:
                _close_all(descriptors[-1:])
            raise


def install(read_paths, writable_roots, devices):
    """Apply the fixed filesystem policy, returning the queried ABI only on success.

    Mount origins, exact accessible special objects and private scratch freshness
    must first be established by the trusted runtime verifier. Existing FDs are
    outside Landlock's open checks; the fixed launcher must close them beforehand.
    """
    reads, writes, nodes = plan(read_paths, writable_roots, devices)
    kernel = _Kernel()
    if os.geteuid() <= 0 or os.getegid() <= 0:
        raise BoundaryError()
    abi = kernel.abi()
    ruleset = None
    try:
        ruleset = kernel.create()
        entries = [(path, 'read', None) for path in reads]
        entries += [(path, 'write', None) for path in writes]
        entries += [(node['path'], 'device', node) for node in nodes]
        for path, kind, node in entries:
            descriptor = _open_path(path, image=kind == 'read')
            try:
                info = os.fstat(descriptor)
                if kind == 'read':
                    if (not (stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode))
                            or info.st_uid != 0 or info.st_mode & 0o022):
                        raise BoundaryError()
                    rights = READ_TREE if stat.S_ISDIR(info.st_mode) else EXECUTE | READ_FILE
                elif kind == 'write':
                    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.geteuid()
                            or info.st_gid != os.getegid() or stat.S_IMODE(info.st_mode) != 0o700):
                        raise BoundaryError()
                    rights = WRITE_TREE
                else:
                    if (not stat.S_ISCHR(info.st_mode) or info.st_uid != 0
                            or os.major(info.st_rdev) != node['major'] or os.minor(info.st_rdev) != node['minor']):
                        raise BoundaryError()
                    rights = (READ_FILE if node['read'] else 0) | (WRITE_FILE if node['write'] else 0)
                    rights |= IOCTL_DEV if node['ioctl'] else 0
                kernel.add(ruleset, descriptor, rights)
            finally:
                os.close(descriptor)
        kernel.restrict(ruleset)
    except (OSError, ValueError, AttributeError):
        raise BoundaryError() from None
    finally:
        if ruleset is not None:
            os.close(ruleset)
    return abi
