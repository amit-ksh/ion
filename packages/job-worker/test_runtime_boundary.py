"""Strict runtime fixtures; native filesystem checks are separately Linux-only."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('runtime_boundary', Path(__file__).with_name('ion_job_runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def profile():
    return {'version': 1, 'mounts': [
        {'point': '/', 'root': '/', 'fstype': 'ext4', 'options': ['rw', 'relatime'], 'super_options': ['rw']},
        {'point': '/dev', 'root': '/', 'fstype': 'tmpfs', 'options': ['rw', 'nosuid'], 'super_options': ['rw']},
    ], 'devices': [{'path': '/dev/null', 'type': 'char', 'major': 1, 'minor': 3,
                    'read': True, 'write': True, 'ioctl': False}],
        'read_trees': ['/usr', '/etc'], 'writable_parent': '/tmp', 'collector': None}


def encoded(value):
    return json.dumps(value, separators=(',', ':')).encode()


def mountinfo(extra=''):
    return ('1 999 8:1 / / rw,relatime - ext4 /dev/root rw\n'
            '2 1 0:2 / /dev rw,nosuid - tmpfs tmpfs rw\n' + extra).encode()


class RuntimeParserTests(unittest.TestCase):
    def rejected(self, value):
        with self.assertRaises(runtime.BoundaryError):
            runtime.parse_profile(encoded(value))

    def test_canonical_policy_hash_ignores_json_key_and_list_order(self):
        value = profile()
        first = runtime.parse_profile(encoded(value))
        value['mounts'].reverse()
        value['read_trees'].reverse()
        value['mounts'][1]['options'].reverse()
        second = runtime.parse_profile(encoded(value))
        self.assertEqual(first, second)
        self.assertEqual(runtime.profile_sha256(first), runtime.profile_sha256(second))
        value['devices'][0]['ioctl'] = True
        self.assertNotEqual(runtime.profile_sha256(first), runtime.profile_sha256(runtime.parse_profile(encoded(value))))

    def test_duplicate_json_keys_and_nonfinite_numbers_rejected(self):
        raw = encoded(profile())
        for data in [b'{"version":1,"version":1}', raw.replace(b'"version":1', b'"version":NaN'),
                     raw.replace(b'"version":1', b'"version":Infinity'), b'\xff', b'{}', b'[]']:
            with self.subTest(data=data[:40]), self.assertRaises(runtime.BoundaryError):
                runtime.parse_profile(data)

    def test_profile_byte_bound_and_nested_json_never_escape_as_recursion_error(self):
        for data in [b'', b' '*(runtime.MAX_PROFILE+1), b'['*2000+b'0'+b']'*2000]:
            with self.assertRaises(runtime.BoundaryError):
                runtime.parse_profile(data)

    def test_unknown_required_fields_and_types_rejected(self):
        for value in [None, [], {**profile(), 'environment': {}},
                      {k:v for k,v in profile().items() if k!='collector'}]:
            self.rejected(value)
        for key, bad in [('version', True), ('version', 2), ('mounts', {}),
                         ('devices', []), ('read_trees', None), ('collector', '/ion/results-other')]:
            value = profile()
            value[key] = bad
            self.rejected(value)

    def test_path_traversal_control_characters_and_ambiguous_separators_rejected(self):
        for path in ['/', 'relative', '/usr/', '/usr//bin', '/usr/../etc', '/usr/./bin',
                     '/usr\\bin', '/usr\x00bin', '/usr\nbin', '/usr/'+('x'*4096)]:
            value=profile()
            value['read_trees']=[path]
            self.rejected(value)

    def test_protected_or_overlapping_read_and_write_trees_rejected(self):
        for trees in [['/proc'], ['/dev/null'], ['/run/app'], ['/ion/results'],
                      ['/tmp/work'], ['/usr','/usr/lib'], ['/usr','/usr']]:
            value=profile()
            value['read_trees']=trees
            self.rejected(value)
        for parent in ['/proc', '/sys/private', '/dev/shm', '/ion/scratch', '/run']:
            value=profile()
            value['writable_parent']=parent
            self.rejected(value)

    def test_static_driver_and_metadata_categories_match_file_boundary_contract(self):
        accepted=['/proc/cpuinfo','/proc/meminfo','/proc/devices','/proc/version',
                  '/proc/uptime','/proc/stat','/proc/driver/nvidia',
                  '/proc/driver/nvidia/gpus/0000:01:00.0/information',
                  '/proc/driver/nvidia-caps/mig/config','/sys/module/nvidia/initstate',
                  '/sys/devices/system/cpu/online','/sys/devices/system/node/node0/cpulist',
                  '/sys/devices/soc0/family','/sys/devices/pci0000:00/0000:00:01.0/vendor']
        for path in accepted:
            value=profile()
            value['read_trees']=[path]
            self.assertEqual(runtime.parse_profile(encoded(value))['read_trees'],[path])
        rejected=['/home','/proc/self/status','/proc/123/mem','/proc/net','/proc/sys',
                  '/proc/driver','/proc/driver/nvidia-other','/proc/driver/nvidia/fd/0',
                  '/proc/driver/nvidia/mem','/sys/devices','/sys/module','/sys/kernel/debug',
                  '/sys/fs/cgroup','/sys/devices/pci-foreign/vendor',
                  '/sys/devices/pci0000:00/0000:00:01.0',
                  '/sys/devices/pci0000:00/0000:00:01.0/resource0',
                  '/sys/devices/pci0000:00/0000:00:01.0/config',
                  '/sys/class/drm/card0/device/vendor','/'+'/'.join('x' for _ in range(65))]
        for path in rejected:
            value=profile()
            value['read_trees']=[path]
            with self.subTest(path=path):
                self.rejected(value)

    def test_device_policy_cannot_use_wildcards_wrong_type_or_boolean_numbers(self):
        for key, bad in [('type','block'), ('major',True), ('minor',-1),
                         ('major',1<<32), ('read',1), ('path','/dev/stdout'), ('path','/dev/nvidia*'), ('path','/usr/device'),
                         ('path','/dev/../infiniband/uverbs0')]:
            value=profile()
            value['devices'][0][key]=bad
            self.rejected(value)
        value=profile()
        value['devices'].append(copy.deepcopy(value['devices'][0]))
        self.rejected(value)
        value=profile()
        for k in ('read','write','ioctl'):
            value['devices'][0][k]=False
        self.rejected(value)

    def test_mount_policy_requires_one_rw_ro_state_and_known_filesystem(self):
        for key,bad in [('fstype','nfs'), ('fstype','fuse.s3fs'), ('point','/../mount'),
                        ('root','relative'), ('options',['rw','ro']), ('options',['rw','rw']),
                        ('super_options',['noexec']), ('options',['rw','x,unknown'])]:
            value=profile()
            value['mounts'][0][key]=bad
            self.rejected(value)
        value=profile()
        value['mounts'].append(copy.deepcopy(value['mounts'][0]))
        self.rejected(value)
        value=profile()
        value['mounts']=value['mounts'][1:]
        self.rejected(value)

    def test_collector_is_only_network_filesystem_exception_and_requires_mount(self):
        value=profile()
        value['collector']='/ion/results'
        self.rejected(value)
        value['mounts'].append({'point':'/ion/results','root':'/','fstype':'fuse.s3fs',
                               'options':['rw','nosuid','nodev'],'super_options':['rw']})
        accepted=runtime.parse_profile(encoded(value))
        self.assertEqual(accepted['collector'],'/ion/results')
        value['mounts'][-1]['point']='/usr/data'
        self.rejected(value)

    def test_mountinfo_missing_visible_parent_is_valid_and_source_not_exposed(self):
        entries=runtime.parse_mountinfo(mountinfo())
        self.assertEqual(entries[0]['parent'],999)
        self.assertNotIn('source',entries[0])
        self.assertEqual(entries[0]['point'],'/')

    def test_mountinfo_rejects_shared_unknown_or_stacked_topology(self):
        for data in [mountinfo().replace(b' - ext4',b' shared:5 - ext4'),
                     mountinfo().replace(b' - ext4',b' unbindable - ext4'),
                     mountinfo().replace(b' - ext4',b' unknown:1 - ext4'),
                     mountinfo('3 1 0:3 / /dev rw - tmpfs tmpfs rw\n'),
                     mountinfo('2 1 0:3 / /scratch rw - tmpfs tmpfs rw\n')]:
            with self.assertRaises(runtime.BoundaryError):
                runtime.parse_mountinfo(data)

    def test_mountinfo_octals_and_malformed_identity_are_bounded(self):
        good=mountinfo('3 1 0:3 / /scratch\\040data rw - tmpfs tmpfs rw\n')
        self.assertEqual(runtime.parse_mountinfo(good)[-1]['point'],'/scratch data')
        for data in [mountinfo().replace(b'8:1',b'-1:1'), mountinfo().replace(b'8:1',b'4294967296:1'),
                     mountinfo().replace(b'/dev rw',b'/dev\\999 rw'), mountinfo().replace(b'1 999',b'0 999'),
                     mountinfo().replace(b'/ / rw',b'/ / rw,ro'), mountinfo()[:-1],
                     b'x'*(runtime.MAX_MOUNT_BYTES+1), b'1 1 0:1 / / rw - tmpfs none rw\n'*513]:
            with self.assertRaises(runtime.BoundaryError):
                runtime.parse_mountinfo(data)

    def test_policy_mount_match_does_not_accept_extra_readonly_or_alias_mount(self):
        accepted=runtime.parse_profile(encoded(profile()))
        with patch.object(runtime,'_kernel_mountinfo',return_value=mountinfo()):
            self.assertEqual(len(runtime._mounts(accepted)),2)
        with patch.object(runtime,'_kernel_mountinfo',return_value=mountinfo('3 1 8:1 /host /host ro - ext4 /dev/root ro\n')):
            with self.assertRaises(runtime.BoundaryError):
                runtime._mounts(accepted)

    def test_collector_backing_alias_and_nonroot_mount_are_rejected(self):
        value=profile()
        value['collector']='/ion/results'
        value['mounts'].append({'point':'/ion/results','root':'/','fstype':'fuse.s3fs','options':['rw'],'super_options':['rw']})
        accepted=runtime.parse_profile(encoded(value))
        good=mountinfo('3 1 0:3 / /ion/results rw - fuse.s3fs bucket rw\n')
        with patch.object(runtime,'_kernel_mountinfo',return_value=good):
            self.assertEqual(len(runtime._mounts(accepted)),3)
        for bad in [good.replace(b'0:3',b'8:1'), good.replace(b'0:3 / /ion',b'0:3 /prefix /ion')]:
            with patch.object(runtime,'_kernel_mountinfo',return_value=bad), self.assertRaises(runtime.BoundaryError):
                runtime._mounts(accepted)
        value['mounts'].append({'point':'/alias','root':'/','fstype':'tmpfs','options':['ro'],'super_options':['ro']})
        accepted=runtime.parse_profile(encoded(value))
        bad=good+b'4 1 0:3 / /alias ro - tmpfs none ro\n'
        with patch.object(runtime,'_kernel_mountinfo',return_value=bad), self.assertRaises(runtime.BoundaryError):
            runtime._mounts(accepted)

    def test_unsupported_platform_never_returns_runtime_evidence(self):
        with patch.object(runtime.sys,'platform','unsupported'), self.assertRaises(runtime.BoundaryError):
            runtime.verify_runtime(profile(),['/tmp/work'],identity=(1000,1000))


@unittest.skipUnless(sys.platform=='linux' and os.geteuid()==0,
                     'Real FD-relative filesystem checks require isolated Linux root fixture')
class NativeFilesystemTests(unittest.TestCase):
    def setUp(self):
        self.temporary=tempfile.TemporaryDirectory(prefix='ion-runtime-fixture-')
        self.root=Path(self.temporary.name)
        self.read=self.root/'read'
        self.read.mkdir(mode=0o755)
        self.value=profile()
        self.value['writable_parent']='/scratch'
        self.value['read_trees']=[str(self.read)]

    def tearDown(self):
        self.temporary.cleanup()

    def walk(self,path=None,writable=False,identity=(1000,1000)):
        return runtime._tree(str(self.read if path is None else path), self.value,
                             {},set(),[0,0,0,0],writable=writable,identity=identity)

    def test_real_filesystem_inventory_is_stable_and_changes_with_metadata(self):
        path=self.read/'library'
        path.write_bytes(b'one')
        path.chmod(0o644)
        first=self.walk()
        self.assertEqual(first,self.walk())
        path.write_bytes(b'larger content')
        self.assertNotEqual(first['sha256'],self.walk()['sha256'])

    def test_actual_image_filenames_are_literal_not_profile_patterns(self):
        for name in ('[','literal*name','question?mark'):
            path=self.read/name
            path.write_bytes(b'reviewed image file')
            path.chmod(0o644)
        (self.read/'alias').symlink_to('[')
        self.assertEqual(self.walk()['entries'],5)
        # The same spelling in an owner-selected root stays explicitly rejected.
        for name in ('[','literal*name','question?mark'):
            value=profile()
            value['read_trees']=[str(self.read/name)]
            with self.assertRaises(runtime.BoundaryError):
                runtime.parse_profile(encoded(value))

    def test_fifo_and_socket_nodes_fail_without_opening_or_waiting(self):
        import socket
        pipe=self.read/'proxy'
        os.mkfifo(pipe,0o600)
        with self.assertRaises(runtime.BoundaryError):
            self.walk()
        pipe.unlink()
        with socket.socket(socket.AF_UNIX) as endpoint:
            endpoint.bind(str(pipe))
            with self.assertRaises(runtime.BoundaryError):
                self.walk()

    def test_world_writable_image_file_and_device_alias_are_rejected(self):
        path=self.read/'unsafe'
        path.write_bytes(b'x')
        path.chmod(0o666)
        with self.assertRaises(runtime.BoundaryError):
            self.walk()
        path.unlink()
        path.symlink_to('/dev/null')
        with self.assertRaises(runtime.BoundaryError):
            self.walk()

    def test_reviewed_in_tree_symlink_is_valid_but_external_target_is_not(self):
        path=self.read/'target'
        path.write_bytes(b'x')
        path.chmod(0o644)
        link=self.read/'link'
        link.symlink_to('target')
        self.assertEqual(self.walk()['entries'],3)
        link.unlink()
        link.symlink_to('/etc/passwd')
        with self.assertRaises(runtime.BoundaryError):
            self.walk()

    def test_unapproved_real_device_and_block_device_are_rejected(self):
        path=self.read/'device'
        os.mknod(path,stat.S_IFCHR|0o600,os.makedev(1,3))
        with self.assertRaises(runtime.BoundaryError):
            self.walk()
        path.unlink()
        os.mknod(path,stat.S_IFBLK|0o600,os.makedev(7,0))
        with self.assertRaises(runtime.BoundaryError):
            self.walk()

    def test_approved_exact_character_device_identity_is_verified(self):
        path=self.read/'device'
        os.mknod(path,stat.S_IFCHR|0o600,os.makedev(1,3))
        approved={str(path):{'major':1,'minor':3}}
        seen=set()
        runtime._tree(str(self.read),self.value,approved,seen,[0,0,0,0])
        self.assertEqual(seen,{str(path)})
        approved[str(path)]['minor']=4
        with self.assertRaises(runtime.BoundaryError):
            runtime._tree(str(self.read),self.value,approved,set(),[0,0,0,0])

    def test_real_private_roots_require_exact_owner_mode_and_disjoint_paths(self):
        path=self.root/'work'
        path.mkdir(mode=0o700)
        os.chown(path,1000,1000)
        value=profile()
        value['writable_parent']=str(self.root)
        value['read_trees']=['/usr']
        self.assertEqual(runtime._roots(value,[str(path)],(1000,1000))[0]['path'],str(path))
        os.chown(path,0,0)
        path.chmod(0o755)
        os.chown(path,1000,1000)
        with self.assertRaises(runtime.BoundaryError):
            runtime._roots(value,[str(path)],(1000,1000))
        os.chown(path,0,0)
        path.chmod(0o700)
        os.chown(path,1000,1000)
        for paths,identity in [([str(path)],(1001,1000)),([str(path),str(path)],(1000,1000))]:
            with self.assertRaises(runtime.BoundaryError):
                runtime._roots(value,paths,identity)

    def test_symlink_directory_root_cannot_replace_checked_directory(self):
        link=self.root/'alias'
        link.symlink_to(self.read,target_is_directory=True)
        with self.assertRaises(OSError):
            fd=runtime._open_directory(str(link))
            os.close(fd)
        self.value['read_trees'].append(str(link))
        with self.assertRaises(runtime.BoundaryError):
            self.walk(link)

    def test_tree_overflow_and_parent_mutation_fail_closed(self):
        (self.read/'one').write_bytes(b'x')
        (self.read/'two').write_bytes(b'x')
        with patch.object(runtime,'MAX_ENTRIES',1),self.assertRaises(runtime.BoundaryError):
            self.walk()
        actual=os.scandir
        def changed(fd):
            (self.read/'three').write_bytes(b'x')
            return actual(fd)
        with patch.object(runtime.os,'scandir',side_effect=changed),self.assertRaises(runtime.BoundaryError):
            self.walk()

    def test_actual_tree_depth_and_encoded_byte_budgets_fail_closed(self):
        current=self.read
        for part in ('a','b','c'):
            current=current/part
            current.mkdir(mode=0o755)
        with patch.object(runtime,'MAX_DEPTH',2),self.assertRaises(runtime.BoundaryError):
            self.walk()
        with patch.object(runtime,'MAX_TREE_BYTES',1),self.assertRaises(runtime.BoundaryError):
            self.walk()

    def test_profile_loader_rejects_symlinks_nonregular_and_mutable_sources(self):
        # /tmp is not a trusted profile ancestor even when the leaf looks safe.
        path=self.read/'profile.json'
        path.write_bytes(encoded(profile()))
        path.chmod(0o444)
        with patch.object(runtime,'PROFILE_PATH',str(path)),self.assertRaises(runtime.BoundaryError):
            runtime.load_profile()

    def test_real_bounded_fd_read_refuses_oversize_before_parsing(self):
        path=self.read/'large'
        path.write_bytes(b'x'*101)
        fd=os.open(path,os.O_RDONLY)
        try:
            with self.assertRaises(runtime.BoundaryError):
                runtime._read_fd(fd,100)
        finally:
            os.close(fd)

    def test_private_writable_tree_rejects_external_hardlinks(self):
        directory=self.root/'work'
        directory.mkdir(mode=0o700)
        os.chown(directory,1000,1000)
        outside=self.root/'outside'
        outside.write_bytes(b'external')
        outside.chmod(0o600)
        os.chown(outside,1000,1000)
        os.link(outside,directory/'alias')
        with self.assertRaises(runtime.BoundaryError):
            self.walk(directory,writable=True)

    def test_directory_swap_between_stat_and_open_is_detected(self):
        nested=self.read/'nested'
        nested.mkdir(mode=0o755)
        original=os.open
        swapped=[]
        def raced(path,flags,*args,**kwargs):
            if path=='nested' and not swapped:
                nested.rename(self.read/'old')
                nested.mkdir(mode=0o755)
                swapped.append(True)
            return original(path,flags,*args,**kwargs)
        with patch.object(runtime.os,'open',side_effect=raced),self.assertRaises(runtime.BoundaryError):
            self.walk()
        self.assertTrue(swapped)

    def test_native_root_and_demoted_child_get_identical_full_snapshot(self):
        import subprocess
        accepted=runtime.parse_mountinfo(runtime._kernel_mountinfo())
        keys=('point','root','fstype','options','super_options')
        value=profile()
        value['mounts']=[{k:e[k] for k in keys} for e in accepted]
        # The read fixture is outside /tmp and has no inherited channels.
        image_read=Path('/runtime-boundary-read-fixture')
        image_read.mkdir(mode=0o755)
        try:
            (image_read/'library').write_bytes(b'reviewed image fixture')
            (image_read/'library').chmod(0o644)
            value['read_trees']=[str(image_read)]
            devices=[]
            for directory,_,files in os.walk('/dev',followlinks=False):
                for name in files:
                    path=os.path.join(directory,name)
                    info=os.lstat(path)
                    if stat.S_ISCHR(info.st_mode):
                        devices.append({'path':path,'type':'char','major':os.major(info.st_rdev),
                                        'minor':os.minor(info.st_rdev),'read':True,'write':True,'ioctl':True})
            value['devices']=devices
            value=runtime.parse_profile(encoded(value))
            self.root.chmod(0o711)
            roots=[]
            for role in ('work','home','tmp'):
                path=self.root/role
                path.mkdir(mode=0o700)
                os.chown(path,1000,1000)
                roots.append(str(path))
            first=runtime.verify_runtime(value,roots,identity=(1000,1000))
            helper=Path(runtime.__file__).resolve()
            arguments=json.dumps([value,roots])
            code=('import importlib.util,json,sys; '
                  's=importlib.util.spec_from_file_location("runtime",sys.argv[1]); '
                  'm=importlib.util.module_from_spec(s); s.loader.exec_module(m); '
                  'v,r=json.loads(sys.argv[2]); '
                  'print(json.dumps(m.verify_runtime(v,r,identity=(1000,1000)),sort_keys=True))')
            child=subprocess.run([sys.executable,'-I','-B','-S','-c',code,str(helper),arguments],
                user=1000,group=1000,extra_groups=[],stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,stderr=subprocess.PIPE,env={'PATH':'/usr/local/bin:/usr/bin:/bin'},
                close_fds=True,timeout=20)
            self.assertEqual(child.returncode,0,child.stderr.decode())
            self.assertEqual(first,json.loads(child.stdout))
        finally:
            import shutil
            shutil.rmtree(image_read)

    def test_kernel_mountinfo_read_is_actual_procfs(self):
        data=runtime._kernel_mountinfo()
        self.assertGreater(len(runtime.parse_mountinfo(data)),0)


if __name__=='__main__':
    unittest.main()
