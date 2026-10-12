#!/usr/bin/env python3
"""Native cgroup2 acceptance in a bounded disposable Docker sandbox, no cloud."""
import base64
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
import uuid

spec = importlib.util.spec_from_file_location('group_guard', '/usr/local/libexec/ion-job-guard.py')
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)
POLICY = {'version':1,'cpu_millis':500,'memory_bytes':128*1024*1024,'processes':32}
LIMITS = {'version':1,'cpu_seconds':5,'address_space_bytes':256*1024*1024,
          'max_file_bytes':1024*1024,'processes':64,'open_files':64,'group':POLICY}

class LinuxGroupTests(unittest.TestCase):
    def invoke(self, script, group=POLICY):
        request={'version':1,'request_id':str(uuid.uuid4()),'argv':[sys.executable,'-I','-S','-c',script],
                 'timeout_seconds':8,'inputs':[],'output_paths':[]}
        env={'PATH':'/usr/local/bin:/usr/bin:/bin','ION_JOB_ISOLATED':'1','ION_JOB_CHILD_UID':'1000',
             'ION_JOB_CHILD_GID':'1000','ION_JOB_RESOURCE_LIMITS':json.dumps({**LIMITS,'group':group})}
        process=subprocess.run(['/usr/local/bin/ion-job-worker'],input=json.dumps(request).encode(),
            capture_output=True,env=env,timeout=18)
        return process, json.loads(process.stdout)

    def test_actual_kernel_limits_and_child_identity(self):
        guard.verify_resource_group(POLICY)
        script="import os,pathlib; assert os.geteuid()==1000; assert 'ION_JOB_RESOURCE_LIMITS' not in os.environ; assert pathlib.Path('/proc/self/cgroup').read_text()=='0::/\\n'; print('group-ok')"
        process,result=self.invoke(script)
        self.assertEqual(process.returncode,0,process.stdout+process.stderr)
        self.assertEqual(result['exit_code'],0,base64.b64decode(result['stderr']['content_base64']))
        self.assertEqual(base64.b64decode(result['stdout']['content_base64']).strip(),b'group-ok')

    def test_unapproved_actual_limit_refuses_before_user_code(self):
        for group in [{**POLICY,'cpu_millis':100},{**POLICY,'memory_bytes':64*1024*1024},{**POLICY,'processes':16}]:
            process,result=self.invoke("print('MUST-NOT-RUN')",group)
            self.assertEqual(process.returncode,1)
            self.assertIn('error',result)
            self.assertNotIn(b'MUST-NOT-RUN',process.stdout+process.stderr)

    def test_forked_processes_cannot_exceed_group_pid_limit(self):
        script="""import errno,os,time
children=[]
try:
 for i in range(50):
  try: pid=os.fork()
  except OSError as e:
   assert e.errno==errno.EAGAIN
   print('group-pids-denied',flush=True)
   break
  if pid==0:
   time.sleep(3); os._exit(0)
  children.append(pid)
 else: raise AssertionError('group process ceiling was not enforced')
finally:
 for pid in children:
  try: os.kill(pid,9)
  except ProcessLookupError: pass
 for pid in children:
  try: os.waitpid(pid,0)
  except ChildProcessError: pass
"""
        process,result=self.invoke(script)
        self.assertEqual(process.returncode,0,process.stdout+process.stderr)
        self.assertEqual(result['exit_code'],0,base64.b64decode(result['stderr']['content_base64']))
        self.assertIn(b'group-pids-denied',base64.b64decode(result['stdout']['content_base64']))

    def test_group_memory_ceiling_covers_forked_children(self):
        before = dict(line.split() for line in Path('/sys/fs/cgroup/memory.events').read_text().splitlines())
        script="""import os,signal,time
children=[]
for i in range(4):
 pid=os.fork()
 if pid==0:
  data=bytearray(40*1024*1024)
  time.sleep(1); os._exit(0)
 children.append(pid)
statuses=[os.waitpid(pid,0)[1] for pid in children]
assert any(os.WIFSIGNALED(status) and os.WTERMSIG(status)==signal.SIGKILL for status in statuses)
print('group-memory-enforced')
"""
        process,result=self.invoke(script)
        self.assertEqual(process.returncode,0,process.stdout+process.stderr)
        self.assertEqual(result['exit_code'],0,base64.b64decode(result['stderr']['content_base64']))
        after = dict(line.split() for line in Path('/sys/fs/cgroup/memory.events').read_text().splitlines())
        self.assertGreater(int(after['oom_kill']),int(before['oom_kill']))

    def test_cpu_controller_throttles_busy_child(self):
        before=dict(line.split() for line in Path('/sys/fs/cgroup/cpu.stat').read_text().splitlines())
        script="""import time
end=time.monotonic()+1.0
while time.monotonic()<end: pass
print('busy-finished')
"""
        process,result=self.invoke(script)
        self.assertEqual(process.returncode,0,process.stdout+process.stderr)
        self.assertEqual(result['exit_code'],0,base64.b64decode(result['stderr']['content_base64']))
        after=dict(line.split() for line in Path('/sys/fs/cgroup/cpu.stat').read_text().splitlines())
        self.assertGreater(int(after['nr_throttled']),int(before['nr_throttled']))

    def test_child_cannot_change_or_escape_group_limits(self):
        script="""import errno,os
for name,value in [('cpu.max','max 100000'),('memory.max','max'),('pids.max','max'),('cgroup.procs',str(os.getpid())),('cgroup.subtree_control','+cpu')]:
 try:
  with open('/sys/fs/cgroup/'+name,'w') as file: file.write(value)
 except OSError as e: assert e.errno in (errno.EPERM,errno.EACCES,errno.EROFS)
 else: raise AssertionError('child modified group control')
print('group-controls-denied')
"""
        process,result=self.invoke(script)
        self.assertEqual(process.returncode,0,process.stdout+process.stderr)
        self.assertEqual(result['exit_code'],0,base64.b64decode(result['stderr']['content_base64']))

if __name__ == '__main__':
    if sys.platform!='linux' or os.geteuid()!=0 or os.environ.get('ION_JOB_GROUP_ACCEPTANCE')!='1':
        raise SystemExit('Use the bounded disposable Linux group test container')
    unittest.main()
