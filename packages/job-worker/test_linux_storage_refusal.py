#!/usr/bin/env python3
"""Actual unsuitable filesystem refusal in one explicitly prepared disposable fixture."""
import importlib.util
import os
from pathlib import Path
import sys
import unittest
SCENARIOS={'oversize-blocks','oversize-inodes','no-private-mount','nested-mount'}
class NativeStorageRefusalTests(unittest.TestCase):
    def test_unsuitable_actual_mount_never_runs_user_marker(self):
        scenario=os.environ.get('ION_JOB_STORAGE_REFUSAL_SCENARIO')
        if sys.platform!='linux' or os.geteuid()!=0 or scenario not in SCENARIOS:
            self.fail('Requires the explicitly prepared disposable native refusal fixture and named scenario')
        spec=importlib.util.spec_from_file_location('fixture',Path(__file__).with_name('test_linux_runtime.py'))
        fixture_module=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture_module)
        fixture_module.InstalledRuntimeTests.setUpClass()
        fixture=fixture_module.InstalledRuntimeTests()
        entries=fixture.runtime.parse_mountinfo(fixture.runtime._kernel_mountinfo())
        selected=[m for m in entries if m['point']=='/tmp']
        if scenario=='no-private-mount':self.assertFalse(selected)
        else:
            self.assertEqual(len(selected),1)
            self.assertEqual(selected[0]['fstype'],'tmpfs')
            actual=os.statvfs('/tmp')
            if scenario=='oversize-blocks':self.assertGreater(actual.f_blocks*actual.f_frsize,fixture_module.GPU_STORAGE['bytes'])
            if scenario=='oversize-inodes':self.assertGreater(actual.f_files,fixture_module.GPU_STORAGE['inodes'])
            if scenario=='nested-mount':self.assertTrue(any(m['point']=='/tmp/bypass' for m in entries))
        result,receipt=fixture.invoke("print('MUST-NOT-EXECUTE')",storage=fixture_module.GPU_STORAGE,group=fixture_module.GPU_GROUP)
        self.assertNotEqual(result.returncode,0,(receipt,result.stderr))
        self.assertIn('error',receipt)
        self.assertNotIn(b'MUST-NOT-EXECUTE',result.stdout+result.stderr)
if __name__=='__main__':unittest.main()
