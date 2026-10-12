"""Pure group policy validation; no Linux runtime or resource mutation."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('group_guard', Path(__file__).with_name('ion_job_guard.py'))
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)
POLICY = {'version': 1, 'cpu_millis': 500, 'memory_bytes': 128*1024*1024, 'processes': 32}

class GroupPolicyTests(unittest.TestCase):
    def test_exact_policy_fields_integer_types_and_bounds(self):
        guard.group_policy(POLICY)
        for name, value in [('version', 2), ('cpu_millis', 0), ('cpu_millis', 32001),
            ('memory_bytes', 1), ('memory_bytes', 8*1024*1024*1024+1),
            ('processes', 1), ('processes', 257), ('version', True), ('cpu_millis', 500.0)]:
            with self.assertRaises(guard.BoundaryError): guard.group_policy({**POLICY, name: value})
        for bad in [None, [], {**POLICY, 'path': '/sys/fs/cgroup'}, {k:v for k,v in POLICY.items() if k != 'version'}]:
            with self.assertRaises(guard.BoundaryError): guard.group_policy(bad)

    def test_finite_cpu_ram_swap_and_process_limits_required(self):
        guard.group_values(POLICY, '50000 100000', str(128*1024*1024), '0', '32')
        guard.group_values(POLICY, '25000 100000', str(64*1024*1024), '0', '16')
        valid = ['50000 100000', str(128*1024*1024), '0', '32']
        for index, value in [(0,'max 100000'), (0,'50001 100000'), (0,'1 0'), (0,'0 100000'),
            (0,'50000 100000 extra'), (0,'50000 1000000.0'), (0,'9'*4096),
            (1,'max'), (1,str(128*1024*1024+1)), (1,'0'), (2,'max'), (2,'1'),
            (3,'max'), (3,'33'), (3,'0'), (3,'-1')]:
            values=valid.copy(); values[index]=value
            with self.assertRaises(guard.BoundaryError): guard.group_values(POLICY, *values)

    def test_group_policy_refuses_non_linux_runtime(self):
        from unittest.mock import patch
        with patch.object(guard.sys, 'platform', 'unsupported'):
            with self.assertRaises(guard.BoundaryError): guard.verify_resource_group(POLICY)

    def test_cpu_fraction_uses_exact_integer_comparison(self):
        guard.group_values(POLICY, '50001 100002', '134217728', '0', '32')
        with self.assertRaises(guard.BoundaryError):
            guard.group_values(POLICY, '50002 100002', '134217728', '0', '32')

if __name__ == '__main__': unittest.main()
