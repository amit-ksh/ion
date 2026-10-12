#!/usr/bin/env python3
"""Installed strict worker acceptance in an owned disposable Linux fixture only."""
import base64
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import unittest
import uuid
BINARY='/usr/local/bin/ion-job-worker'
PROFILE='/usr/local/libexec/ion-job-runtime.json'

class InstalledRuntimeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if sys.platform!='linux' or os.geteuid()!=0:
            raise RuntimeError('Run only as root in the explicitly prepared disposable Linux test container')
        # This mask belongs only to the disposable acceptance fixture. The
        # production verifier rejects the standard alias, including missing policy.
        core=Path('/dev/core')
        if core.is_symlink():core.unlink()
        spec=importlib.util.spec_from_file_location('fixture_runtime','/usr/local/libexec/ion_job_runtime.py')
        cls.runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(cls.runtime)
        mounts=cls.runtime.parse_mountinfo(Path('/proc/self/mountinfo').read_bytes())
        devices=[]
        for directory,names,files in os.walk('/dev',followlinks=False):
            for name in names+files:
                path=Path(directory)/name;info=path.lstat()
                if stat.S_ISCHR(info.st_mode):
                    devices.append({'path':str(path),'type':'char','major':os.major(info.st_rdev),'minor':os.minor(info.st_rdev),'read':True,'write':True,'ioctl':False})
        cls.profile=cls.runtime.parse_profile(json.dumps({'version':1,'mounts':[{key:m[key] for key in ('point','root','fstype','options','super_options')} for m in mounts],
            'devices':devices,'read_trees':['/usr/local/bin','/usr/local/lib','/usr/lib/x86_64-linux-gnu','/etc/ld.so.cache'],'writable_parent':'/tmp','collector':None}).encode())
        Path(PROFILE).write_text(json.dumps(cls.profile),encoding='utf-8');os.chmod(PROFILE,0o444)
        cls.digest=cls.runtime.profile_sha256(cls.profile)
        cls.outside=Path('/tmp/strict-outside-readable');cls.outside.write_text('private-to-fixture');cls.outside.chmod(0o644)
    def invoke(self,script,digest=None,outputs=None,inputs=None):
        request={'version':1,'request_id':str(uuid.uuid4()),'argv':['/usr/local/bin/python3','-I','-S','-c',script],
            'timeout_seconds':12,'inputs':[] if inputs is None else inputs,'output_paths':[] if outputs is None else outputs}
        env={'PATH':'/usr/local/bin:/usr/bin:/bin','ION_JOB_ISOLATED':'1','ION_JOB_CHILD_UID':'1000','ION_JOB_CHILD_GID':'1000',
            'ION_JOB_RUNTIME_PROFILE_SHA256':self.digest if digest is None else digest}
        result=subprocess.run([BINARY],input=json.dumps(request).encode(),capture_output=True,env=env,timeout=25)
        return result,json.loads(result.stdout)
    def test_installed_guard_allows_private_files_and_denies_global_paths_devices_ipc_and_fds(self):
        script=r"""import ctypes,errno,os,pathlib,subprocess,sys,threading
assert not any(k.startswith('ION_JOB_RUNTIME_') for k in os.environ)
for folder in (str(pathlib.Path.cwd()),os.environ['HOME'],os.environ['TMPDIR']):
    p=pathlib.Path(folder)/'allowed';p.write_text('bounded');assert p.read_text()=='bounded'
def denied(path,mode):
    try:open(path,mode)
    except OSError as e:assert e.errno in (errno.EACCES,errno.EPERM),e.errno
    else:raise AssertionError('global path escaped')
denied('/tmp/strict-outside-readable','r');denied('/tmp/strict-global-write','w');denied('/proc/self/fd/2','rb')
lib=ctypes.CDLL(None,use_errno=True);lib.syscall.restype=ctypes.c_long
# Actual native x86_64 SysV, POSIX queue, mknod and mount syscalls.
for nr,args in [(29,(0,4096,0o600)),(64,(0,1,0o600)),(68,(0,0o600)),(240,(b'/ion-private-test',0,0,0)),(133,(b'/tmp/no-device',0o20600,0)),(165,(0,0,0,0,0))]:
    ctypes.set_errno(0);assert lib.syscall(nr,*args)==-1;assert ctypes.get_errno()==errno.EPERM,(nr,ctypes.get_errno())
seen=[];t=threading.Thread(target=lambda:seen.append(1));t.start();t.join();assert seen==[1]
pid=os.fork()
if pid==0:
    try:open('/tmp/strict-outside-readable','r')
    except OSError:os._exit(0)
    os._exit(8)
assert os.waitpid(pid,0)[1]==0
code="import pathlib;\ntry:pathlib.Path('/tmp/strict-outside-readable').read_text()\nexcept PermissionError:pass\nelse:raise AssertionError('exec escaped')"
assert subprocess.run([sys.executable,'-I','-S','-c',code]).returncode==0
print('strict private execution passed')
"""
        result,receipt=self.invoke(script)
        self.assertEqual(result.returncode,0,(receipt,result.stderr))
        self.assertEqual(receipt['exit_code'],0,receipt)
        self.assertIn('strict private execution passed',base64.b64decode(receipt['stdout']['content_base64']).decode())
    def test_literal_bracket_project_input_and_output_survive_snapshot_checks(self):
        script="import pathlib;assert pathlib.Path('[input].txt').read_bytes()==b'actual input';pathlib.Path('result[1].txt').write_bytes(b'literal result')"
        inputs=[{'path':'[input].txt','content_base64':base64.b64encode(b'actual input').decode()}]
        result,receipt=self.invoke(script,inputs=inputs,outputs=['result[1].txt'])
        self.assertEqual(result.returncode,0,(receipt,result.stderr))
        self.assertEqual(receipt['exit_code'],0,receipt)
        self.assertEqual(base64.b64decode(receipt['outputs'][0]['content_base64']),b'literal result')

    def test_actual_nonzero_output_and_private_collector_receipt(self):
        script="import pathlib,sys;pathlib.Path('checkpoint.bin').write_bytes(b'failed checkpoint');print('actual failure',file=sys.stderr);sys.exit(9)"
        result,receipt=self.invoke(script,outputs=['checkpoint.bin'])
        self.assertEqual(result.returncode,0,(receipt,result.stderr))
        self.assertEqual(receipt['exit_code'],9)
        self.assertEqual(base64.b64decode(receipt['outputs'][0]['content_base64']),b'failed checkpoint')
        self.assertEqual(base64.b64decode(receipt['stderr']['content_base64']).strip(),b'actual failure')
        self.assertFalse(receipt['output_errors'])

    def test_unique_mounted_collector_preserves_actual_receipt_and_refuses_unapproved_collector(self):
        if not any(m['point']=='/ion/results' for m in self.profile['mounts']):
            self.skipTest('Requires the explicitly prepared unique /ion/results test mount')
        original=Path(PROFILE).read_bytes()
        profile=dict(self.profile);profile['collector']='/ion/results'
        profile=self.runtime.parse_profile(json.dumps(profile).encode())
        Path(PROFILE).write_text(json.dumps(profile),encoding='utf-8');Path(PROFILE).chmod(0o444)
        request={'version':1,'request_id':str(uuid.uuid4()),
            'argv':['/usr/local/bin/python3','-I','-S','-c',"import pathlib,sys\ntry:pathlib.Path('/ion/results/forged').write_text('unsafe')\nexcept PermissionError:pass\nelse:raise AssertionError('collector escaped')\npathlib.Path('checkpoint.bin').write_bytes(b'private actual output');sys.exit(9)"],
            'timeout_seconds':12,'inputs':[],'output_paths':['checkpoint.bin']}
        source=Path('/ion/job-request.json');source.write_text(json.dumps(request),encoding='utf-8');source.chmod(0o400)
        env={'PATH':'/usr/local/bin:/usr/bin:/bin','ION_JOB_ISOLATED':'1','ION_JOB_CHILD_UID':'1000','ION_JOB_CHILD_GID':'1000',
            'ION_JOB_RUNTIME_PROFILE_SHA256':self.runtime.profile_sha256(profile)}
        args=[BINARY,'--request-file',str(source),'--result-volume','/ion/results']
        try:
            result=subprocess.run(args,capture_output=True,env=env,timeout=25)
            self.assertEqual(result.returncode,0,(result.stdout,result.stderr))
            ack=json.loads(result.stdout);self.assertTrue(ack['result_written'])
            target=Path('/ion/results')/request['request_id']/'receipt.json';saved=target.read_bytes();receipt=json.loads(saved)
            self.assertEqual(receipt['exit_code'],9,receipt)
            self.assertEqual(base64.b64decode(receipt['outputs'][0]['content_base64']),b'private actual output')
            self.assertFalse(Path('/ion/results/forged').exists())
            duplicate=subprocess.run(args,capture_output=True,env=env,timeout=25)
            self.assertNotEqual(duplicate.returncode,0)
            self.assertEqual(target.read_bytes(),saved)
            # The ordinary root directory is not a mounted collector profile.
            bad=dict(profile);bad['collector']=None
            Path(PROFILE).write_text(json.dumps(bad),encoding='utf-8');Path(PROFILE).chmod(0o444)
            source.write_text(json.dumps(dict(request,request_id=str(uuid.uuid4()))),encoding='utf-8')
            env['ION_JOB_RUNTIME_PROFILE_SHA256']=self.runtime.profile_sha256(bad)
            rejected=subprocess.run(args,capture_output=True,env=env,timeout=25)
            self.assertNotEqual(rejected.returncode,0);self.assertIn('error',json.loads(rejected.stdout))
        finally:
            source.unlink();Path(PROFILE).write_bytes(original);Path(PROFILE).chmod(0o444)

    def test_wrong_profile_fails_before_user_marker(self):
        marker=Path('/tmp/no-user-marker')
        result,body=self.invoke("open('/tmp/no-user-marker','w').write('unsafe')",digest='a'*64)
        self.assertNotEqual(result.returncode,0)
        self.assertIn('error',body)
        self.assertFalse(marker.exists())
    def test_missing_profile_fails_before_user_marker(self):
        content=Path(PROFILE).read_bytes();Path(PROFILE).unlink()
        try:
            result,body=self.invoke("open('/tmp/no-user-marker','w').write('unsafe')")
            self.assertNotEqual(result.returncode,0);self.assertIn('error',body);self.assertFalse(Path('/tmp/no-user-marker').exists())
        finally:Path(PROFILE).write_bytes(content);Path(PROFILE).chmod(0o444)
    def test_undeclared_device_alias_fails_before_user_marker(self):
        alias=Path('/dev/unapproved-null-alias');os.mknod(alias,stat.S_IFCHR|0o666,os.makedev(1,3))
        try:
            result,body=self.invoke("open('/tmp/no-user-marker','w').write('unsafe')")
            self.assertNotEqual(result.returncode,0);self.assertIn('error',body);self.assertFalse(Path('/tmp/no-user-marker').exists())
        finally:alias.unlink()

if __name__=='__main__':
    unittest.main()
