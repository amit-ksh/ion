#!/usr/bin/env python3
# Internal JSON transport. Run only in a registered, credential-free remote image.
import base64
import ctypes
import hashlib
import importlib.util
import json
import os
import re
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
MAX_PROJECT = 34 * 1024 * 1024
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
    allowed=['version', 'request_id', 'argv', 'timeout_seconds', 'inputs', 'output_paths']
    if 'project_snapshot' in request:
        allowed.append('project_snapshot')
        facts=request['project_snapshot']
        keys(facts,['version','sha256','bytes','files','source_bytes'])
        if (any(type(facts[k]) is not int for k in ('version','bytes','files','source_bytes'))
            or facts['version']!=1 or not 12<=facts['bytes']<=MAX_PROJECT
            or not 1<=facts['files']<=4096 or not 0<=facts['source_bytes']<=32*1024*1024
            or not isinstance(facts['sha256'],str) or len(facts['sha256'])!=64
            or any(c not in '0123456789abcdef' for c in facts['sha256']) or request.get('inputs')!=[]):
            raise ValueError('Invalid project snapshot')
    keys(request,allowed)
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



def project_inputs(data, facts):
    if (len(data)!=facts['bytes'] or hashlib.sha256(data).hexdigest()!=facts['sha256']
        or data[:8]!=b'IONPROJ1'):
        raise ValueError('Project archive identity changed')
    length=int.from_bytes(data[8:12],'big')
    if not 1<=length<=2*1024*1024 or length+12>len(data):
        raise ValueError('Invalid project index')
    entries=json.loads(data[12:12+length])
    if not isinstance(entries,list) or len(entries)!=facts['files']:
        raise ValueError('Project file count changed')
    seen=set(); offset=12+length; decoded=[]; total=0
    for entry in entries:
        keys(entry,['path','bytes','sha256','executable'])
        relative(entry['path'])
        name=entry['path'].casefold()
        if (name in seen or type(entry['bytes']) is not int or not 0<=entry['bytes']<=8*1024*1024
            or type(entry['executable']) is not bool or not isinstance(entry['sha256'],str)):
            raise ValueError('Invalid project entry')
        seen.add(name); end=offset+entry['bytes']; content=data[offset:end]
        if end>len(data) or hashlib.sha256(content).hexdigest()!=entry['sha256']:
            raise ValueError('Project file hash changed')
        total+=len(content); offset=end
        if total>32*1024*1024:
            raise ValueError('Project bytes exceed their bound')
        decoded.append((entry['path'],content,entry['executable']))
    if offset!=len(data) or total!=facts['source_bytes']:
        raise ValueError('Project archive length changed')
    # Detect a file used as another file's parent before creating any workspace.
    if any('/'.join(parts[:i]).casefold() in seen
           for name in seen for parts in [name.split('/')] for i in range(1,len(parts))):
        raise ValueError('Project paths conflict')
    return decoded


def read_project_file():
    flags=os.O_RDONLY|os.O_NOFOLLOW|os.O_CLOEXEC
    fd=os.open('/ion/job-project.ion',flags)
    with os.fdopen(fd,'rb') as source:
        info=os.fstat(source.fileno())
        if (not stat.S_ISREG(info.st_mode) or info.st_nlink!=1 or info.st_uid!=0
            or stat.S_IMODE(info.st_mode)!=0o400 or info.st_size>MAX_PROJECT):
            raise ValueError('Project archive must be a private supervisor file')
        data=source.read(MAX_PROJECT+1)
    if len(data)>MAX_PROJECT:
        raise ValueError('Project archive exceeds its bound')
    return data

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


def verify_group_approval():
    encoded = os.environ.get('ION_JOB_RESOURCE_LIMITS')
    gpu_encoded = os.environ.get('ION_JOB_GPU_GROUP_LIMITS')
    limits = {}
    if encoded is not None:
        if len(encoded) > 1024:
            raise ValueError('Child resource policy exceeds its bound')
        limits = json.loads(encoded)
        if not isinstance(limits, dict):
            raise ValueError('Invalid child resource policy')
    if 'group' not in limits and gpu_encoded is None:
        return
    if gpu_encoded is not None and 'ION_JOB_RUNTIME_PROFILE_SHA256' not in os.environ:
        raise ValueError('GPU resource caps require the approved strict runtime')
    guard = Path('/usr/local/libexec/ion-job-guard.py')
    info = guard.lstat()
    if (guard.is_symlink() or not stat.S_ISREG(info.st_mode) or info.st_uid != 0
        or info.st_nlink != 1 or stat.S_IMODE(info.st_mode) & 0o022):
        raise ValueError('Trusted resource guard unavailable')
    for parent in guard.parents:
        info = parent.lstat()
        if parent.is_symlink() or not stat.S_ISDIR(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
            raise ValueError('Trusted resource guard directory unavailable')
    spec = importlib.util.spec_from_file_location('ion_fixed_group_guard', guard)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    try:
        if 'group' in limits:module.verify_resource_group(limits['group'])
        if gpu_encoded is not None:module.verify_resource_group(module.gpu_group_encoded(gpu_encoded),gpu=True)
    except (module.BoundaryError, OSError, ValueError):
        raise ValueError('Approved sandbox resource group unavailable') from None


def run(request, workspace, project_data=None):
    # The remote VM/container is the isolation boundary. No local runtime invokes this worker.
    inputs = [(name,data,False) for name,data in validate(request)]
    if 'project_snapshot' in request:
        if project_data is None:
            raise ValueError('Project archive missing')
        inputs=project_inputs(project_data,request['project_snapshot'])
    elif project_data is not None:
        raise ValueError('Unexpected project archive')
    strict_digest=os.environ.get('ION_JOB_RUNTIME_PROFILE_SHA256')
    if any(k in os.environ for k in ('ION_JOB_GPU_GROUP_LIMITS','ION_JOB_GPU_STORAGE_LIMITS')) and strict_digest is None:
        raise ValueError('GPU resource caps require the approved strict runtime')
    if strict_digest is not None and not re.fullmatch('[a-f0-9]{64}',strict_digest):
        raise ValueError('Invalid strict runtime approval')
    if strict_digest is not None and (sys.platform!='linux' or os.environ.get('ION_JOB_ISOLATED')!='1'):
        raise ValueError('Strict runtime requires isolated Linux execution')
    root = workspace.resolve()
    root.mkdir(mode=0o700, parents=True, exist_ok=False)
    identity = None
    if sys.platform == 'linux' and os.environ.get('ION_JOB_ISOLATED') == '1':
        uid, gid = int(os.environ['ION_JOB_CHILD_UID']), int(os.environ['ION_JOB_CHILD_GID'])
        if os.geteuid() != 0 or uid <= 0 or gid <= 0:
            raise ValueError('Use a trusted root supervisor and nonroot command identity')
        identity = (uid, gid)
        verify_group_approval()

    home = root.parent / 'home'
    home.mkdir(mode=0o700)
    scratch = root.parent / 'tmp' if strict_digest is not None else None
    if scratch is not None:scratch.mkdir(mode=0o700)
    for name, data, executable in inputs:
        path = safe_file(root, name)
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        with path.open('xb') as target:
            target.write(data)
        path.chmod(0o700 if executable else 0o600)
    child_options = {}
    if identity:
        # The child cannot signal/read the supervisor or inject its outer receipt.
        root.parent.chmod(0o711)
        for owned in [root, home, *root.rglob('*')]:
            os.chown(owned, *identity)
        if scratch is not None:os.chown(scratch,*identity)
        child_options = {'user': identity[0], 'group': identity[1], 'extra_groups': []}
    env = {'PATH': '/usr/local/bin:/usr/bin:/bin', 'HOME': str(home),
           'TMPDIR': str(scratch or root.parent), 'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8'}
    argv = request['argv']
    if strict_digest is not None:
        # A profile cannot be chosen by child code: image-fixed file plus exact
        # owner launch digest, verified before any user process is started.
        guard=Path('/usr/local/libexec/ion-job-guard.py')
        # Verify before importing the fixed module, matching group preflight.
        info=guard.lstat()
        if guard.is_symlink() or not stat.S_ISREG(info.st_mode) or info.st_uid!=0 or info.st_nlink!=1 or info.st_mode & 0o022:
            raise ValueError('Trusted runtime guard unavailable')
        for parent in guard.parents:
            info=parent.lstat()
            if parent.is_symlink() or not stat.S_ISDIR(info.st_mode) or info.st_uid!=0 or info.st_mode & 0o022:
                raise ValueError('Trusted runtime guard path unavailable')
        spec=importlib.util.spec_from_file_location('ion_supervisor_runtime_guard',guard)
        helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
        try:
            helper.trusted_file(guard)
            runtime=helper.trusted_module('ion_supervisor_runtime','ion_job_runtime.py')
            profile=runtime.load_profile()
            collector='/ion/results' if os.environ.get('ION_JOB_RUNTIME_COLLECTOR')=='/ion/results' else None
            roots=[str(root),str(home),str(scratch)]
            witness=runtime.verify_runtime(profile,roots,collector=collector,identity=identity)
            encoded_storage=os.environ.get('ION_JOB_GPU_STORAGE_LIMITS')
            storage=runtime.verify_storage(runtime.storage_encoded(encoded_storage),profile,roots) if encoded_storage is not None else None
        except Exception:
            raise ValueError('Approved strict runtime unavailable') from None
        if witness['profile_sha256']!=strict_digest:
            raise ValueError('Strict runtime profile changed')
        if storage is not None:
            env['ION_JOB_GPU_STORAGE_LIMITS']=encoded_storage
            env['ION_JOB_GPU_STORAGE_SNAPSHOT_SHA256']=storage['sha256']
        env['ION_JOB_RUNTIME_PROFILE_SHA256']=strict_digest
        env['ION_JOB_RUNTIME_SNAPSHOT_SHA256']=witness['snapshot_sha256']
        if collector:env['ION_JOB_RUNTIME_COLLECTOR']=collector
    if identity:
        if 'ION_JOB_GPU_GROUP_LIMITS' in os.environ:
            env['ION_JOB_GPU_GROUP_LIMITS']=os.environ['ION_JOB_GPU_GROUP_LIMITS']
        if 'ION_JOB_RESOURCE_LIMITS' in os.environ:
            limits = os.environ['ION_JOB_RESOURCE_LIMITS']
            if len(limits) > 1024:
                raise ValueError('Child resource policy exceeds its bound')
            env['ION_JOB_RESOURCE_LIMITS'] = limits
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
    result = {'version': VERSION, 'request_id': request['request_id'], 'exit_code': process.returncode,
            'timed_out': timed_out, 'canceled': canceled,
            'elapsed_milliseconds': int((time.monotonic()-started)*1000),
            'stdout': captured['stdout'], 'stderr': captured['stderr'],
            'outputs': outputs, 'output_errors': failures}
    if 'project_snapshot' in request:
        result['project_sha256']=request['project_snapshot']['sha256']
    return result


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
              'Options: --help, --version, --request-file /ion/job-request.json [--result-volume /ion/results], --project-file /ion/job-project.ion')
        return 0
    if sys.argv[1:] == ['--version']:
        print('ion-job-worker/1')
        return 0
    if sys.argv[1:] not in ([], ['--project-file', '/ion/job-project.ion'], ['--request-file', '/ion/job-request.json'],
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
        project_mode=sys.argv[1:]==['--project-file','/ion/job-project.ion']
        stream = open('/ion/job-request.json', 'rb') if len(sys.argv)>1 and not project_mode else sys.stdin.buffer
        data = stream.read(MAX_INPUT+1)
        if len(data) > MAX_INPUT:
            raise ValueError('Job input exceeds its bound')
        request = json.loads(data)
        validate(request)
        if project_mode != ('project_snapshot' in request):
            raise ValueError('Project snapshot mode changed')
        project_data=None
        if project_mode:
            secure_result_parent()
            project_data=read_project_file()
        result_folder = None
        if len(sys.argv) == 5:
            if 'ION_JOB_RUNTIME_PROFILE_SHA256' in os.environ:os.environ['ION_JOB_RUNTIME_COLLECTOR']='/ion/results'
            secure_result_parent()
            result_folder = prepare_result_directory(Path('/ion/results'), request['request_id'])
        with tempfile.TemporaryDirectory(prefix='ion-job-') as temporary:
            result = run(request, Path(temporary) / 'work',project_data)
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
