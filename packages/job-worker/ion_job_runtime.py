"""Internal strict GPU runtime snapshot checks, not ongoing confinement."""
import ctypes
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys

CONTRACT = 'linux-gpu-runtime-snapshot/v1'
PROFILE_PATH = '/usr/local/libexec/ion-job-runtime.json'
MAX_PROFILE = 64 * 1024
MAX_MOUNT_BYTES = 256 * 1024
MAX_MOUNTS = 512
MAX_ENTRIES = 100000
MAX_TREE_BYTES = 16 * 1024 * 1024
MAX_DEPTH = 64
MAX_PATH = 4096
MAX_WITNESS = 64 * 1024
FD_ALIASES = {'/dev/fd': '/proc/self/fd', '/dev/stdin': '/proc/self/fd/0',
              '/dev/stdout': '/proc/self/fd/1', '/dev/stderr': '/proc/self/fd/2'}
LOCAL_FS = frozenset(('overlay', 'ext4', 'xfs', 'btrfs', 'tmpfs', 'ramfs',
                      'proc', 'sysfs', 'cgroup2', 'devtmpfs', 'devpts', 'mqueue'))
PROTECTED_READ_ROOTS = frozenset(('/', '/dev', '/proc', '/sys', '/run', '/ion', '/tmp'))

# Keep protected metadata categories aligned with ion_job_landlock.read_path.
STATIC_PROC_FILES = frozenset(('/proc/cpuinfo', '/proc/meminfo', '/proc/devices',
                               '/proc/version', '/proc/uptime', '/proc/stat'))
NVIDIA_PROC_TREES = ('/proc/driver/nvidia', '/proc/driver/nvidia-caps',
                     '/proc/driver/nvidia-nvlink', '/proc/driver/nvidia-nvswitch')
SYS_METADATA_TREES = ('/sys/devices/system/cpu', '/sys/devices/system/node',
                      '/sys/module/nvidia', '/sys/module/nvidia_uvm',
                      '/sys/module/nvidia_modeset', '/sys/module/nvidia_drm',
                      '/sys/module/nvidia_peermem')
PCI_METADATA_FILES = frozenset(('vendor', 'device', 'subsystem_vendor', 'subsystem_device',
                                'class', 'numa_node', 'local_cpulist', 'local_cpus',
                                'uevent', 'modalias'))


class BoundaryError(ValueError):
    """Unsupported, ambiguous or changed runtime; arbitrary child must not start."""


def _canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True).encode('ascii')


def profile_sha256(profile):
    return hashlib.sha256(_canonical(parse_profile(_canonical(profile)))).hexdigest()


def _fail():
    raise BoundaryError('GPU runtime snapshot validation unavailable')


def _object(value, fields):
    if not isinstance(value, dict) or set(value) != set(fields):
        _fail()


def _integer(value, maximum):
    if type(value) is not int or not 0 <= value <= maximum:
        _fail()
    return value


def _path(value, root=False, literal=False):
    if (not isinstance(value, str) or not value.startswith('/') or len(value.encode('utf-8')) > MAX_PATH
        or any(ord(c) < 32 or ord(c) == 127 for c in value) or '\\' in value or (not literal and any(c in value for c in '*?[]'))
        or (value != '/' and (value.endswith('/') or any(p in ('', '.', '..') for p in value[1:].split('/'))))
        or (value == '/' and not root) or len(value[1:].split('/')) > MAX_DEPTH):
        _fail()
    return value


def _below(path, parent):
    return path == parent or (parent == '/' or path.startswith(parent + '/'))


def _strings(value, count=64):
    if not isinstance(value, list) or not 1 <= len(value) <= count or len(set(value)) != len(value):
        _fail()
    if any(not isinstance(v, str) or not v or len(v.encode('utf-8')) > MAX_PATH
           or any(ord(c) < 33 or ord(c) > 126 for c in v) or ',' in v for v in value):
        _fail()
    return sorted(value)


def _duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            _fail()
        result[key] = value
    return result


def read_path(value):
    """The same pure read-root contract is used by profile parsing and install."""
    path = _path(value)
    if path in ('/home',) or any(_below(path, root) for root in ('/ion', '/run', '/dev', '/tmp')):
        _fail()
    if _below(path, '/proc'):
        # Static driver/host metadata only. No dynamic process subtree, including
        # self, thread-self, numeric PIDs, task/*, fd, fdinfo, map_files or mem.
        if (any(component in ('fd', 'fdinfo', 'map_files', 'mem') for component in path.split('/'))
                or (path not in STATIC_PROC_FILES and not any(_below(path, root) for root in NVIDIA_PROC_TREES))):
            _fail()
    if _below(path, '/sys'):
        metadata_tree = any(_below(path, root) for root in SYS_METADATA_TREES)
        # PCI BAR/resource and arbitrary driver directories can expose hardware
        # interfaces despite regular-file stat types. Grant only named info leaves.
        pci_metadata = (re.fullmatch(r'/sys/devices/pci[0-9a-fA-F]{4}:[0-9a-fA-F]{2}'
                                     r'(?:/[0-9a-fA-F]{4}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}\.[0-7])+'
                                     r'/[a-z_]+', path) is not None
                        and path.rsplit('/', 1)[-1] in PCI_METADATA_FILES)
        if not metadata_tree and not pci_metadata and path != '/sys/devices/soc0/family':
            _fail()
    return path

def parse_profile(data):
    if not isinstance(data, bytes) or not 1 <= len(data) <= MAX_PROFILE:
        _fail()
    try:
        value = json.loads(data.decode('utf-8'), object_pairs_hook=_duplicates,
                           parse_constant=lambda _: _fail())
        _object(value, ('version', 'mounts', 'devices', 'read_trees', 'writable_parent', 'collector'))
        if type(value['version']) is not int or value['version'] != 1:
            _fail()
        _path(value['writable_parent'])
        if any(_below(value['writable_parent'], p) for p in ('/dev', '/proc', '/sys', '/run', '/ion')):
            _fail()
        if value['collector'] is not None and value['collector'] != '/ion/results':
            _fail()
        if not isinstance(value['mounts'], list) or not 1 <= len(value['mounts']) <= MAX_MOUNTS:
            _fail()
        mounts, points = [], set()
        for entry in value['mounts']:
            _object(entry, ('point', 'root', 'fstype', 'options', 'super_options'))
            point, source_root = _path(entry['point'], True), _path(entry['root'], True)
            fs = entry['fstype']
            if (not isinstance(fs, str) or not re.fullmatch(r'[a-z0-9_.-]{1,64}', fs)
                or (fs not in LOCAL_FS and not (point == value['collector'] and fs.startswith('fuse.')))
                or point in points):
                _fail()
            points.add(point)
            options, super_options = _strings(entry['options']), _strings(entry['super_options'])
            if sum(flag in options for flag in ('ro', 'rw')) != 1 or sum(flag in super_options for flag in ('ro', 'rw')) != 1:
                _fail()
            mounts.append({'point': point, 'root': source_root, 'fstype': fs,
                           'options': options, 'super_options': super_options})
        if '/' not in points or (value['collector'] is not None and value['collector'] not in points):
            _fail()
        if not isinstance(value['devices'], list) or not 1 <= len(value['devices']) <= 64:
            _fail()
        devices, paths = [], set()
        for entry in value['devices']:
            _object(entry, ('path', 'type', 'major', 'minor', 'read', 'write', 'ioctl'))
            path = _path(entry['path'])
            if path in paths or entry['type'] != 'char' or path in FD_ALIASES or not _below(path, '/dev'):
                _fail()
            paths.add(path)
            _integer(entry['major'], (1 << 32) - 1)
            _integer(entry['minor'], (1 << 32) - 1)
            if any(type(entry[k]) is not bool for k in ('read', 'write', 'ioctl')) or not any(entry[k] for k in ('read', 'write', 'ioctl')):
                _fail()
            devices.append(dict(entry))
        read_trees = value['read_trees']
        if not isinstance(read_trees, list) or not 1 <= len(read_trees) <= 32:
            _fail()
        if len(set(read_trees)) != len(read_trees):
            _fail()
        for path in read_trees:
            read_path(path)
            if (path in PROTECTED_READ_ROOTS or _below(path, '/dev') or _below(path, '/run')
                or _below(path, '/ion') or _below(path, value['writable_parent'])
                or any(_below(path, p) or _below(p, path) for p in paths)):
                _fail()
        if any(_below(a, b) or _below(b, a) for i, a in enumerate(read_trees) for b in read_trees[i+1:]):
            _fail()
        return {'version': 1, 'mounts': sorted(mounts, key=lambda v: v['point']),
                'devices': sorted(devices, key=lambda v: v['path']), 'read_trees': sorted(read_trees),
                'writable_parent': value['writable_parent'], 'collector': value['collector']}
    except (TypeError, UnicodeError, json.JSONDecodeError, RecursionError, OverflowError):
        _fail()


def _flags():
    if sys.platform != 'linux' or not all(hasattr(os, name) for name in ('O_NOFOLLOW', 'O_DIRECTORY', 'O_CLOEXEC', 'O_PATH')):
        _fail()
    return os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC


def _metadata(info):
    return {'dev': info.st_dev, 'ino': info.st_ino, 'mode': info.st_mode,
            'uid': info.st_uid, 'gid': info.st_gid, 'nlink': info.st_nlink, 'size': info.st_size}


def _same(a, b):
    if _metadata(a) != _metadata(b) or a.st_mtime_ns != b.st_mtime_ns or a.st_ctime_ns != b.st_ctime_ns:
        _fail()


def _open_directory(path, trusted=False):
    _flags()
    flags = os.O_PATH | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_DIRECTORY
    fd = os.open('/', flags)
    try:
        parts = PurePosixPath(_path(path, True)).parts[1:]
        info = os.fstat(fd)
        if trusted and (info.st_uid != 0 or info.st_mode & 0o022):
            _fail()
        for part in parts:
            before = os.stat(part, dir_fd=fd, follow_symlinks=False)
            next_fd = os.open(part, flags, dir_fd=fd)
            try:
                info = os.fstat(next_fd)
                _same(before, info)
                if trusted and (info.st_uid != 0 or info.st_mode & 0o022):
                    _fail()
            except BaseException:
                os.close(next_fd)
                raise
            os.close(fd)
            fd = next_fd
        return fd
    except BaseException:
        os.close(fd)
        raise


def _read_fd(fd, maximum):
    chunks, total = [], 0
    while total <= maximum:
        chunk = os.read(fd, min(16384, maximum + 1 - total))
        if not chunk:
            break
        chunks.append(chunk)
        total += len(chunk)
    if total > maximum:
        _fail()
    return b''.join(chunks)


def load_profile():
    """Only fixed root-owned profile paths, with no ancestor symlinks or writers."""
    parent, name = PROFILE_PATH.rsplit('/', 1)
    directory = _open_directory(parent, trusted=True)
    try:
        before = os.stat(name, dir_fd=directory, follow_symlinks=False)
        if not stat.S_ISREG(before.st_mode) or before.st_uid != 0 or before.st_nlink != 1 or before.st_mode & 0o022:
            _fail()
        fd = os.open(name, _flags() | os.O_NONBLOCK, dir_fd=directory)
        try:
            info = os.fstat(fd)
            _same(before, info)
            if (not stat.S_ISREG(info.st_mode) or info.st_uid != 0 or info.st_nlink != 1 or info.st_mode & 0o022):
                _fail()
            data = _read_fd(fd, MAX_PROFILE)
            _same(info, os.fstat(fd))
            _same(info, os.stat(name, dir_fd=directory, follow_symlinks=False))
        finally:
            os.close(fd)
    finally:
        os.close(directory)
    return parse_profile(data)


def _unescape(value):
    # mountinfo uses only these four octal pathname escapes.
    if re.search(r'\\(?!040|011|012|134)', value):
        _fail()
    return re.sub(r'\\(040|011|012|134)', lambda m: chr(int(m.group(1), 8)), value)


def parse_mountinfo(data):
    if not isinstance(data, bytes) or not 1 <= len(data) <= MAX_MOUNT_BYTES or not data.endswith(b'\n'):
        _fail()
    try:
        lines = data.decode('utf-8').splitlines()
        if not 1 <= len(lines) <= MAX_MOUNTS:
            _fail()
        entries, ids, points = [], set(), set()
        for line in lines:
            if len(line.encode('utf-8')) > 4096:
                _fail()
            fields = line.split(' ')
            if '' in fields or fields.count('-') != 1:
                _fail()
            marker = fields.index('-')
            if marker != 6 or len(fields) != 10:
                # No shared/master/propagating or unknown optional fields.
                _fail()
            if any(not re.fullmatch(r'[0-9]{1,20}', v) for v in fields[:2]):
                _fail()
            mount_id, parent_id = map(int, fields[:2])
            if not 1 <= mount_id < (1 << 64) or not 1 <= parent_id < (1 << 64) or mount_id in ids:
                _fail()
            match = re.fullmatch(r'([0-9]{1,10}):([0-9]{1,10})', fields[2])
            if match is None:
                _fail()
            major, minor = map(int, match.groups())
            _integer(major, (1 << 32)-1)
            _integer(minor, (1 << 32)-1)
            root, point = _path(_unescape(fields[3]), True), _path(_unescape(fields[4]), True)
            if point in points:
                _fail()
            ids.add(mount_id)
            points.add(point)
            fs = fields[7]
            if not re.fullmatch(r'[a-z0-9_.-]{1,64}', fs):
                _fail()
            options, super_options = _strings(fields[5].split(',')), _strings(fields[9].split(','))
            if sum(flag in options for flag in ('ro', 'rw')) != 1 or sum(flag in super_options for flag in ('ro', 'rw')) != 1:
                _fail()
            entries.append({'id': mount_id, 'parent': parent_id, 'major': major, 'minor': minor,
                            'root': root, 'point': point, 'fstype': fs, 'options': options,
                            'super_options': super_options})
        if '/' not in points:
            _fail()
        return sorted(entries, key=lambda v: v['point'])
    except (UnicodeError, TypeError, ValueError, RecursionError):
        _fail()


def _kernel_mountinfo():
    fd = os.open('/proc/self/mountinfo', _flags())
    try:
        libc = ctypes.CDLL(None, use_errno=True)
        libc.fstatfs.argtypes = [ctypes.c_int, ctypes.c_void_p]
        libc.fstatfs.restype = ctypes.c_int
        filesystem = ctypes.create_string_buffer(512)
        if libc.fstatfs(fd, filesystem) != 0 or ctypes.c_long.from_buffer(filesystem).value != 0x9fa0:
            _fail()
        data = _read_fd(fd, MAX_MOUNT_BYTES)
    finally:
        os.close(fd)
    return data


def _mounts(profile):
    data = _kernel_mountinfo()
    entries = parse_mountinfo(data)
    keys = ('point', 'root', 'fstype', 'options', 'super_options')
    if [{k: v[k] for k in keys} for v in entries] != profile['mounts']:
        _fail()
    collector = profile['collector']
    if collector is not None:
        selected = next(v for v in entries if v['point'] == collector)
        shared = [v for v in entries if (v['major'], v['minor']) == (selected['major'], selected['minor'])]
        if len(shared) != 1 or selected['root'] != '/':
            _fail()
        if 'rw' not in selected['options'] or 'rw' not in selected['super_options']:
            _fail()
    return entries


def _parent_stat(path):
    parent, name = path.rsplit('/', 1)
    fd = _open_directory(parent or '/')
    try:
        return os.stat(name, dir_fd=fd, follow_symlinks=False)
    finally:
        os.close(fd)


def _roots(profile, roots, identity):
    if not isinstance(roots, (list, tuple)) or not 1 <= len(roots) <= 4:
        _fail()
    if (not isinstance(identity, (list, tuple)) or len(identity) != 2
        or any(type(v) is not int or not 0 < v < (1 << 32) for v in identity)):
        _fail()
    paths, result = [], []
    for path in roots:
        _path(path)
        if (not _below(path, profile['writable_parent']) or path == profile['writable_parent']
            or any(_below(path, p) or _below(p, path) for p in profile['read_trees'])
            or any(_below(path, p) or _below(p, path) for p in paths)
            or _below(path, '/ion')):
            _fail()
        info = _parent_stat(path)
        if (not stat.S_ISDIR(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o700
            or (info.st_uid, info.st_gid) != tuple(identity)):
            _fail()
        paths.append(path)
        fd = _open_directory(path)
        try:
            _same(info, os.fstat(fd))
        finally:
            os.close(fd)
        result.append({'path': path, **_metadata(info)})
    return sorted(result, key=lambda v: v['path'])


def _collector(profile, collector, identity):
    if collector != profile['collector']:
        _fail()
    if collector is None:
        return None
    info = _parent_stat('/ion')
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != 0 or stat.S_IMODE(info.st_mode) != 0o700:
        _fail()
    if os.geteuid() == identity[0]:
        try:
            fd = _open_directory(collector)
        except PermissionError:
            pass
        else:
            os.close(fd)
            _fail()
    else:
        fd = _open_directory(collector)
        try:
            if os.fstat(fd).st_uid != 0:
                _fail()
        finally:
            os.close(fd)
    _same(info, _parent_stat('/ion'))
    return {'path': collector, 'private_parent': '/ion', 'parent': _metadata(info)}


def _tree(path, profile, approved_devices, seen_devices, budget, writable=False, identity=None, device_tree=False):
    """FD-relative bounded enumeration. Never opens a child-selected FIFO/device."""
    digest, count = hashlib.sha256(), 0

    def record(item):
        nonlocal count
        data = _canonical(item)
        budget[0] += 1
        budget[1] += len(data)
        count += 1
        if budget[0] > MAX_ENTRIES or budget[1] > MAX_TREE_BYTES:
            _fail()
        digest.update(data + b'\n')

    def inspect(parent_fd, name, current, depth):
        if depth > MAX_DEPTH:
            _fail()
        before = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
        if stat.S_ISLNK(before.st_mode):
            if depth == 0:
                _fail()
            if before.st_uid != 0 or writable:
                _fail()
            target = os.readlink(name, dir_fd=parent_fd)
            if current in FD_ALIASES and target == FD_ALIASES[current]:
                record({'path': current, 'alias': target, **_metadata(before)})
            else:
                resolved = str(Path(current).resolve(strict=True))
                _path(resolved, literal=True)
                if not (any(_below(resolved, p) for p in profile['read_trees'])
                        or resolved in approved_devices):
                    _fail()
                if writable:
                    _fail()
                record({'path': current, 'alias': resolved, **_metadata(before)})
            if os.readlink(name, dir_fd=parent_fd) != target:
                _fail()
            _same(before, os.stat(name, dir_fd=parent_fd, follow_symlinks=False))
            return
        if stat.S_ISCHR(before.st_mode):
            approved = approved_devices.get(current)
            if (approved is None or before.st_uid != 0
                or (os.major(before.st_rdev), os.minor(before.st_rdev)) != (approved['major'], approved['minor'])):
                _fail()
            seen_devices.add(current)
        elif not (stat.S_ISDIR(before.st_mode) or stat.S_ISREG(before.st_mode)):
            _fail()
        if writable:
            if stat.S_ISREG(before.st_mode) and before.st_nlink != 1:
                _fail()
            if (before.st_uid, before.st_gid) != tuple(identity) or before.st_mode & 0o077:
                _fail()
        elif not device_tree and (before.st_uid != 0 or before.st_mode & 0o022):
            _fail()
        if device_tree and stat.S_ISREG(before.st_mode):
            _fail()
        record({'path': current, **_metadata(before)})
        if stat.S_ISDIR(before.st_mode):
            fd = os.open(name, _flags() | os.O_DIRECTORY, dir_fd=parent_fd)
            try:
                _same(before, os.fstat(fd))
                with os.scandir(fd) as iterator:
                    names = []
                    for entry in iterator:
                        budget[2] += 1
                        if budget[2] > MAX_ENTRIES:
                            _fail()
                        child_path = current.rstrip('/') + '/' + entry.name
                        _path(child_path, literal=True)
                        budget[3] += len(child_path.encode('utf-8'))
                        if budget[3] > MAX_TREE_BYTES:
                            _fail()
                        names.append(entry.name)
                for child in sorted(names):
                    child_path = current.rstrip('/') + '/' + child
                    _path(child_path, literal=True)
                    inspect(fd, child, child_path, depth + 1)
                _same(before, os.fstat(fd))
            finally:
                os.close(fd)
        _same(before, os.stat(name, dir_fd=parent_fd, follow_symlinks=False))

    parent, name = path.rsplit('/', 1)
    parent_fd = _open_directory(parent or '/')
    try:
        inspect(parent_fd, name, path, 0)
    finally:
        os.close(parent_fd)
    return {'path': path, 'entries': count, 'sha256': digest.hexdigest()}


def verify_runtime(profile, writable_roots, collector=None, identity=None):
    """Return a bounded canonical snapshot witness, never a confinement claim."""
    _flags()
    profile = parse_profile(_canonical(profile))
    if identity is None:
        identity = (os.geteuid(), os.getegid())
    if (not isinstance(identity, (list, tuple)) or len(identity) != 2
        or any(type(v) is not int or not 0 < v < (1 << 32) for v in identity)):
        _fail()
    if os.geteuid() != 0 and (os.getresuid() != (identity[0],)*3
                             or os.getresgid() != (identity[1],)*3 or os.getgroups()):
        _fail()
    try:
        mounts = _mounts(profile)
        roots = _roots(profile, writable_roots, identity)
        collector_fact = _collector(profile, collector, identity)
        approved_devices = {v['path']: v for v in profile['devices']}
        seen_devices, budget, trees = set(), [0, 0, 0, 0], []
        for path in profile['read_trees']:
            trees.append(_tree(path, profile, approved_devices, seen_devices, budget))
        trees.append(_tree('/dev', profile, approved_devices, seen_devices, budget, device_tree=True))
        for device in profile['devices']:
            if not _below(device['path'], '/dev') and not any(_below(device['path'], p) for p in profile['read_trees']):
                _fail()
        if seen_devices != set(approved_devices):
            _fail()
        for root in roots:
            trees.append(_tree(root['path'], profile, approved_devices, seen_devices, budget,
                               writable=True, identity=identity))
        if _roots(profile, writable_roots, identity) != roots or _mounts(profile) != mounts:
            _fail()
        witness = {'version': 1, 'contract': CONTRACT, 'profile_sha256': profile_sha256(profile),
                   'identity': list(identity), 'mounts': mounts, 'devices': profile['devices'],
                   'trees': sorted(trees, key=lambda v: v['path']), 'writable_roots': roots,
                   'collector': collector_fact}
        encoded = _canonical(witness)
        if len(encoded) > MAX_WITNESS:
            _fail()
        witness['snapshot_sha256'] = hashlib.sha256(encoded).hexdigest()
        return witness
    except (OSError, TypeError, KeyError, UnicodeError, RecursionError, OverflowError):
        _fail()


STORAGE_CONTRACT='linux-private-workspace-capacity/v1'
STORAGE_FILESYSTEMS={'tmpfs':0x01021994,'ext4':0xEF53}


def storage_policy(value):
    _object(value,('version','filesystem','bytes','inodes'))
    if (type(value['version']) is not int or value['version']!=1
        or type(value['filesystem']) is not str or value['filesystem'] not in STORAGE_FILESYSTEMS
        or type(value['bytes']) is not int or not 16*1024*1024<=value['bytes']<=1024**4
        or type(value['inodes']) is not int or not 128<=value['inodes']<=16777216):
        _fail()
    return dict(value)


def storage_encoded(encoded):
    if type(encoded) is not str or not 1<=len(encoded)<=1024:
        _fail()
    try:
        return storage_policy(json.loads(encoded,object_pairs_hook=_duplicates,parse_constant=lambda _: _fail()))
    except (ValueError,TypeError,RecursionError):
        _fail()


def verify_storage(policy,profile,writable_roots):
    # The fixed /tmp mount covers work, HOME and TMPDIR. No child-selected path,
    # fstatvfs free-space guess, provider marker or background disk polling.
    _flags();policy=storage_policy(policy)
    if profile['writable_parent']!='/tmp' or len(writable_roots)!=3:
        _fail()
    mounts=_mounts(profile)
    selected=[m for m in mounts if m['point']=='/tmp']
    if len(selected)!=1:
        _fail()
    selected=selected[0]
    if (selected['root']!='/' or selected['fstype']!=policy['filesystem']
        or not {'rw','nosuid','nodev'}<=set(selected['options'])
        or 'rw' not in selected['super_options']
        or any(m['point']!='/tmp' and _below(m['point'],'/tmp') for m in mounts)
        or sum((m['major'],m['minor'])==(selected['major'],selected['minor']) for m in mounts)!=1):
        _fail()
    before=os.lstat('/tmp')
    if not stat.S_ISDIR(before.st_mode) or before.st_uid!=0:
        _fail()
    fd=os.open('/tmp',_flags()|os.O_DIRECTORY)
    try:
        info=os.fstat(fd);_same(before,info)
        if (os.major(info.st_dev),os.minor(info.st_dev))!=(selected['major'],selected['minor']):
            _fail()
        libc=ctypes.CDLL(None,use_errno=True)
        libc.fstatfs.argtypes=[ctypes.c_int,ctypes.c_void_p];libc.fstatfs.restype=ctypes.c_int
        data=ctypes.create_string_buffer(512)
        if libc.fstatfs(fd,data)!=0 or ctypes.c_long.from_buffer(data).value!=STORAGE_FILESYSTEMS[policy['filesystem']]:
            _fail()
        capacity=os.fstatvfs(fd)
        if (not 1<=capacity.f_frsize<=1024*1024 or not 1<=capacity.f_blocks<=2**63-1
            or not 1<=capacity.f_files<=policy['inodes']
            or capacity.f_blocks*capacity.f_frsize>policy['bytes']
            or capacity.f_flag & os.ST_RDONLY
            or not capacity.f_flag & os.ST_NOSUID or not capacity.f_flag & os.ST_NODEV):
            _fail()
        for root in writable_roots:
            _path(root)
            if root=='/tmp' or not _below(root,'/tmp'):
                _fail()
            root_fd=_open_directory(root)
            try:
                if os.fstat(root_fd).st_dev!=info.st_dev:
                    _fail()
            finally:os.close(root_fd)
        _same(info,os.fstat(fd));_same(info,os.lstat('/tmp'))
        again=os.fstatvfs(fd)
        stable=lambda value:(value.f_frsize,value.f_blocks,value.f_files,value.f_fsid,value.f_flag)
        if stable(capacity)!=stable(again) or _mounts(profile)!=mounts:
            _fail()
        witness={'version':1,'contract':STORAGE_CONTRACT,'policy':policy,
            'mount':selected,'dev':info.st_dev,'ino':info.st_ino,
            'bytes':capacity.f_blocks*capacity.f_frsize,'inodes':capacity.f_files,
            'fsid':capacity.f_fsid,'flags':capacity.f_flag,
            'writable_roots':sorted(writable_roots)}
        witness['sha256']=hashlib.sha256(_canonical(witness)).hexdigest()
        return witness
    finally:os.close(fd)
