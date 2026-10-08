#!/usr/bin/env python3
# Internal JSON transport. Run only in a registered, credential-free remote image.
import base64
import ctypes
import hashlib
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys
import tempfile
import threading
import time
import uuid

VERSION = 1
MAX_INPUT = 512 * 1024
MAX_FILE = 64 * 1024
MAX_FILES = 32
MAX_TRANSFER = 256 * 1024
MAX_STREAM = 8192
PRIVATE = {'.git', '.ssh', '.aws', '.codex', '.agents', '.env', '.npmrc', '.pypirc',
           'credentials.json', 'secrets.json', 'node_modules', 'target', '.waku-cache'}


def relative(value):
    if not isinstance(value, str) or not value or len(value) > 512:
        raise ValueError('Invalid workspace path')
    parts = value.split('/')
    for part in parts:
        lower = part.lower()
        if (not part or part in {'.', '..'} or '\\' in part or ':' in part
            or part.endswith(('.', ' ')) or any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in part)
            or lower in PRIVATE or lower.startswith(('.env.', '.ion-edit-'))
            or lower.endswith(('.pem', '.p12', '.pfx'))):
            raise ValueError('Private or non-relative workspace path')
    return parts


def keys(value, allowed):
    if not isinstance(value, dict) or set(value) != set(allowed):
        raise ValueError('Unsupported job fields')


def validate(request):
    keys(request, ['version', 'request_id', 'argv', 'timeout_seconds', 'inputs', 'output_paths'])
    if type(request['version']) is not int or request['version'] != VERSION or str(uuid.UUID(request['request_id'])) != request['request_id']:
        raise ValueError('Invalid job identity')
    if uuid.UUID(request['request_id']).int == 0:
        raise ValueError('Empty job identity')
    argv = request['argv']
    if (not isinstance(argv, list) or not 1 <= len(argv) <= 64
        or any(not isinstance(v, str) or '\0' in v or len(v) > 8192 for v in argv)
        or not (argv[0].startswith('/') or (os.name == 'nt' and Path(argv[0]).is_absolute()))
        or sum(map(len, argv)) > 32 * 1024):
        raise ValueError('Use an absolute executable and bounded literal arguments')
    if type(request['timeout_seconds']) is not int or not 1 <= request['timeout_seconds'] <= 3600:
        raise ValueError('Invalid job timeout')
    if not isinstance(request['inputs'], list) or len(request['inputs']) > MAX_FILES:
        raise ValueError('Too many job inputs')
    seen, total, decoded = set(), 0, []
    for item in request['inputs']:
        keys(item, ['path', 'content_base64'])
        relative(item['path'])
        name = item['path'].casefold()
        if name in seen:
            raise ValueError('Duplicate input path')
        seen.add(name)
        if not isinstance(item['content_base64'], str) or len(item['content_base64']) > 4 * MAX_FILE // 3 + 4:
            raise ValueError('Job input exceeds its bound')
        content = base64.b64decode(item['content_base64'], validate=True)
        if len(content) > MAX_FILE:
            raise ValueError('Job input exceeds its bound')
        total += len(content)
        if total > MAX_TRANSFER:
            raise ValueError('Job transfer exceeds its bound')
        decoded.append((item['path'], content))
    outputs = request['output_paths']
    if not isinstance(outputs, list) or len(outputs) > MAX_FILES:
        raise ValueError('Too many job outputs')
    seen = set()
    for name in outputs:
        relative(name)
        if name.casefold() in seen:
            raise ValueError('Duplicate output path')
        seen.add(name.casefold())
    return decoded


def safe_file(root, name):
    current = root
    for part in relative(name):
        current = current / part
        if current.is_symlink():
            raise ValueError('Workspace path became a link')
        if current.exists() and not current.resolve().is_relative_to(root):
            raise ValueError('Workspace path escaped its root')
    return current


def require_supervisor_capabilities(mask):
    # CHOWN, KILL, SETGID and SETUID are needed to provision/demote/reap the
    # command. A DAC bypass is also needed to collect and remove child-owned
    # private files. Refuse before user code rather than lose its receipt later.
    required = sum(1 << bit for bit in (0, 5, 6, 7))
    if (type(mask) is not int or not 0 <= mask < (1 << 64)
        or mask & required != required or not mask & ((1 << 1) | (1 << 2))):
        raise ValueError('Linux supervisor permissions unavailable')


def validate_supervisor():
    with open('/proc/self/status', 'rb') as source:
        data = source.read(16 * 1024 + 1)
    if len(data) > 16 * 1024:
        raise ValueError('Linux supervisor status exceeds its bound')
    fields = dict(line.split(':', 1) for line in data.decode('ascii').splitlines() if ':' in line)
    require_supervisor_capabilities(int(fields['CapEff'].strip(), 16))


def enable_subreaper():
    # Linux adopts orphaned grandchildren even when a command calls setsid().
    # Only the isolated CLI entrypoint enables this; host fixture callers do not.
    libc = ctypes.CDLL(None, use_errno=True)
    libc.prctl.argtypes = [ctypes.c_int, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_ulong]
    libc.prctl.restype = ctypes.c_int
    # PR_SET_CHILD_SUBREAPER, PR_SET_NO_NEW_PRIVS, PR_SET_DUMPABLE.
    # The no-new-privileges bit is inherited by the demoted command on exec.
    for option, value in [(36, 1), (38, 1), (4, 0)]:
        if libc.prctl(option, value, 0, 0, 0) != 0:
            raise OSError('Linux child supervision unavailable')


def reap_descendants():
    deadline = time.monotonic() + 2
    while True:
        parents, alive = {}, set()
        for entry in Path('/proc').iterdir():
            if not entry.name.isdigit():
                continue
            try:
                lines = (entry / 'status').read_text(errors='replace').splitlines()
                fields = dict(line.split(':', 1) for line in lines if ':' in line)
                pid = int(entry.name)
                parents[pid] = int(fields['PPid'].strip())
                if not fields['State'].strip().startswith('Z'):
                    alive.add(pid)
            except (OSError, ValueError, KeyError):
                continue
        descendants = {os.getpid()}
        previous = 0
        while len(descendants) != previous:
            previous = len(descendants)
            descendants.update(pid for pid, parent in parents.items() if parent in descendants)
        descendants.remove(os.getpid())
        for pid in descendants & alive:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        while True:
            try:
                child, _ = os.waitpid(-1, os.WNOHANG)
                if child == 0:
                    break
            except ChildProcessError:
                break
        if not descendants & alive:
            return
        if time.monotonic() >= deadline:
            raise ValueError('Command descendants did not stop')
        time.sleep(0.02)


def stop_tree(process):
    if os.name == 'posix':
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    elif process.poll() is None:
        process.kill()
    process.wait()
    if sys.platform == "linux" and os.environ.get("ION_JOB_ISOLATED") == "1":
        reap_descendants()


def run(request, workspace):
    # The remote VM/container is the isolation boundary. No local runtime invokes this worker.
    inputs = validate(request)
    root = workspace.resolve()
    root.mkdir(mode=0o700, parents=True, exist_ok=False)
    identity = None
    if sys.platform == 'linux' and os.environ.get('ION_JOB_ISOLATED') == '1':
        uid, gid = int(os.environ['ION_JOB_CHILD_UID']), int(os.environ['ION_JOB_CHILD_GID'])
        if os.geteuid() != 0 or uid <= 0 or gid <= 0:
            raise ValueError('Use a trusted root supervisor and nonroot command identity')
        identity = (uid, gid)

    home = root.parent / 'home'
    home.mkdir(mode=0o700)
    for name, data in inputs:
        path = safe_file(root, name)
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        with path.open('xb') as target:
            target.write(data)
        path.chmod(0o600)
    child_options = {}
    if identity:
        # The child cannot signal/read the supervisor or inject its outer receipt.
        root.parent.chmod(0o711)
        for owned in [root, home, *root.rglob('*')]:
            os.chown(owned, *identity)
        child_options = {'user': identity[0], 'group': identity[1], 'extra_groups': []}
    env = {'PATH': '/usr/local/bin:/usr/bin:/bin', 'HOME': str(home),
           'TMPDIR': str(root.parent), 'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8'}
    argv = request['argv']
    if identity:
        # A fixed trusted isolated interpreter loads the kernel boundary before user
        # code. Never use preexec_fn or load it from the command's working directory.
        argv = [sys.executable, '-I', '-S', '/usr/local/libexec/ion-job-guard.py',
                str(identity[0]), str(identity[1]), '--', *argv]
    process = subprocess.Popen(argv, cwd=root, env=env, stdin=subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               start_new_session=os.name == 'posix', close_fds=True, **child_options)
    captured = {}
    def drain(name, pipe):
        content, count = bytearray(), 0
        while True:
            data = pipe.read(4096)
            if not data:
                break
            count += len(data)
            if len(content) < MAX_STREAM:
                content.extend(data[:MAX_STREAM-len(content)])
        pipe.close()
        captured[name] = {'content_base64': base64.b64encode(content).decode(),
                          'bytes': count, 'truncated': count > MAX_STREAM}
    readers = [threading.Thread(target=drain, args=(name, pipe), daemon=True)
               for name, pipe in [('stdout', process.stdout), ('stderr', process.stderr)]]
    for reader in readers:
        reader.start()
    started, timed_out, canceled = time.monotonic(), False, False
    try:
        process.wait(timeout=request['timeout_seconds'])
    except subprocess.TimeoutExpired:
        timed_out = True
    except BaseException:
        canceled = True
        raise
    finally:
        # Reap the whole process group even after the parent exits; orphaned descendants
        # cannot keep the output pipes or mutate collection while files are read.
        stop_tree(process)
        for reader in readers:
            reader.join(timeout=5)
        if any(reader.is_alive() for reader in readers):
            raise ValueError('Job output pipes did not close')
    outputs, failures, total = [], [], 0
    for name in request['output_paths']:
        try:
            path = safe_file(root, name)
            info = path.lstat()
            if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_size > MAX_FILE:
                raise ValueError('Output must be a bounded regular file without links')
            with path.open('rb') as source:
                data = source.read(MAX_FILE+1)
            if len(data) > MAX_FILE or total + len(data) > MAX_TRANSFER:
                raise ValueError('Job output exceeds its transfer bound')
            total += len(data)
            outputs.append({'path': name, 'content_base64': base64.b64encode(data).decode(),
                            'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)})
        except (OSError, ValueError):
            failures.append({'path': name, 'reason': 'Missing, unsafe or oversized output'})
    return {'version': VERSION, 'request_id': request['request_id'], 'exit_code': process.returncode,
            'timed_out': timed_out, 'canceled': canceled,
            'elapsed_milliseconds': int((time.monotonic()-started)*1000),
            'stdout': captured['stdout'], 'stderr': captured['stderr'],
            'outputs': outputs, 'output_errors': failures}


def prepare_result_directory(volume, request_id):
    # The caller secures /ion before launching the demoted child. It cannot reach
    # this mount even if the backing object filesystem does not implement chmod.
    if str(uuid.UUID(request_id)) != request_id or uuid.UUID(request_id).int == 0:
        raise ValueError('Invalid result identity')
    if volume.is_symlink() or not volume.is_dir():
        raise ValueError('Result volume must be a directory without links')
    folder = volume / request_id
    folder.mkdir(mode=0o700, exist_ok=False)
    if folder.is_symlink() or not folder.is_dir():
        raise ValueError('Result directory is unsafe')
    return folder


def write_result(folder, result):
    # This is the trusted supervisor's output, after every command descendant stops.
    # A crashed/partial write stays unusable; the owner never repeats the command.
    if folder.is_symlink() or not folder.is_dir() or folder.name != result['request_id']:
        raise ValueError('Result destination changed')
    data = json.dumps(result, separators=(',', ':')).encode('utf-8')
    if len(data) > 512 * 1024:
        raise ValueError('Result exceeds its bound')
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, 'O_NOFOLLOW', 0)
    fd = os.open(folder / 'receipt.json', flags, 0o600)
    with os.fdopen(fd, 'wb') as target:
        info = os.fstat(target.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
            raise ValueError('Result must be an exclusive regular file')
        target.write(data)
        target.flush()
        os.fsync(target.fileno())


def secure_result_parent():
    parent = Path('/ion')
    info = parent.lstat()
    if parent.is_symlink() or not stat.S_ISDIR(info.st_mode) or info.st_uid != 0:
        raise ValueError('Result parent must belong to the trusted supervisor')
    parent.chmod(0o700)
    if stat.S_IMODE(parent.stat().st_mode) != 0o700:
        raise ValueError('Result parent isolation unavailable')


def main():
    if sys.argv[1:] == ['--help']:
        print('ion-job-worker: versioned JSON request on stdin; isolated remote images only.\n'
              'Options: --help, --version, --request-file /ion/job-request.json [--result-volume /ion/results]')
        return 0
    if sys.argv[1:] == ['--version']:
        print('ion-job-worker/1')
        return 0
    if sys.argv[1:] not in ([], ['--request-file', '/ion/job-request.json'],
                           ['--request-file', '/ion/job-request.json', '--result-volume', '/ion/results']):
        print(json.dumps({'error': 'Unsupported arguments', 'help': 'Run ion-job-worker --help'}))
        return 2
    if sys.platform != 'linux' or os.environ.get('ION_JOB_ISOLATED') != '1' or os.geteuid() != 0:
        print(json.dumps({'error': 'Registered isolated Linux supervisor required'}))
        return 2
    stream = None
    try:
        validate_supervisor()
        enable_subreaper()
        stream = open('/ion/job-request.json', 'rb') if len(sys.argv) > 1 else sys.stdin.buffer
        data = stream.read(MAX_INPUT+1)
        if len(data) > MAX_INPUT:
            raise ValueError('Job input exceeds its bound')
        request = json.loads(data)
        validate(request)
        result_folder = None
        if len(sys.argv) == 5:
            secure_result_parent()
            result_folder = prepare_result_directory(Path('/ion/results'), request['request_id'])
        with tempfile.TemporaryDirectory(prefix='ion-job-') as temporary:
            result = run(request, Path(temporary) / 'work')
            if result_folder is not None:
                write_result(result_folder, result)
                sys.stdout.write(json.dumps({'version': VERSION, 'request_id': request['request_id'],
                                            'result_written': True}, separators=(',', ':')))
            else:
                sys.stdout.write(json.dumps(result, separators=(',', ':')))
        return 0
    except (ValueError, OSError, TypeError, KeyError):
        print(json.dumps({'error': 'Job contract or execution failed',
                          'help': 'Check the pinned worker version, executable and file limits'}))
        return 1
    finally:
        if stream is not None and stream is not sys.stdin.buffer:
            stream.close()


if __name__ == '__main__':
    raise SystemExit(main())
