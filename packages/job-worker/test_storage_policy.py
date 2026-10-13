"""Pure strict storage approval and capacity verification failure tests."""
import importlib.util
from pathlib import Path
import unittest,json
spec=importlib.util.spec_from_file_location('storage_runtime',Path(__file__).with_name('ion_job_runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
POLICY={'version':1,'filesystem':'tmpfs','bytes':16*1024*1024,'inodes':128}
class StoragePolicyTests(unittest.TestCase):
    def test_exact_types_filesystems_and_bounds(self):
        for fs in ('tmpfs','ext4'):runtime.storage_policy({**POLICY,'filesystem':fs})
        for name,value in [('version',True),('version',2),('filesystem','overlay'),('filesystem','xfs'),('filesystem','fuse.s3'),('filesystem',[]),('bytes',True),('bytes',16*1024*1024-1),('bytes',1024**4+1),('inodes',127),('inodes',16777217),('inodes',128.0)]:
            with self.assertRaises(runtime.BoundaryError):runtime.storage_policy({**POLICY,name:value})
        for value in [None,[],{**POLICY,'path':'/chosen'},{k:v for k,v in POLICY.items() if k!='bytes'}]:
            with self.assertRaises(runtime.BoundaryError):runtime.storage_policy(value)
    def test_encoded_duplicate_unknown_oversize_and_nonfinite_rejected(self):
        self.assertEqual(runtime.storage_encoded(json.dumps(POLICY)),POLICY)
        for value in ['', ' '*1025, '{','[]','null',json.dumps(POLICY)[:-1]+',"bytes":16777216}',json.dumps({**POLICY,'bytes':float('inf')})]:
            with self.assertRaises(runtime.BoundaryError):runtime.storage_encoded(value)
    def test_unsupported_platform_never_claims_storage_verification(self):
        from unittest.mock import patch
        with patch.object(runtime.sys,'platform','unsupported'),self.assertRaises(runtime.BoundaryError):
            runtime.verify_storage(POLICY,{},[])
if __name__=='__main__':unittest.main()
