# Strict GPU runtime snapshot validation

Implemented 2026-10-12. This validates a runtime snapshot before execution;
it does **not** continuously confine files/devices, prove CUDA compatibility,
settle billing or enable GPU agent submission.

## Module and APIs

[ion_job_runtime.py](../../packages/job-worker/ion_job_runtime.py) uses Python's
standard library only.

| API                                                                      | Contract                                                                                                                                                                                                                                               |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `parse_profile(bytes)`                                                   | Strict bounded JSON; returns canonical validated data. Duplicate/unknown keys, booleans used as numbers, unsupported versions and ambiguous paths fail.                                                                                                |
| `profile_sha256(profile)`                                                | SHA256 of canonical approved profile; independent of JSON key order and accepted list ordering.                                                                                                                                                        |
| `load_profile()`                                                         | Reads only /usr/local/libexec/ion-job-runtime.json. Requires root ownership, no group/world writes, one regular-file link, trusted ancestors and stable FD/path metadata.                                                                              |
| `verify_runtime(profile, writable_roots, collector=None, identity=None)` | Reads actual procfs mountinfo, inventories read trees and /dev through directory FDs, validates private writable roots and optional collector, repeats mounts/roots, then returns a bounded witness/hash. Missing support or uncertain evidence fails. |

The root supervisor supplies the exact nonzero child UID/GID. The demoted guard
supplies the same identity and repeats verification. Default identity is the
current identity; UID 0 is not an accepted child identity.

### Exact profile schema

Top-level fields are `version=1`, `mounts`, `devices`, `read_trees`,
`writable_parent` and `collector`. No additional fields are accepted.

- Each mount: `point`, `root`, `fstype`, `options`, `super_options`.
  These are exact stable topology/options; runtime mount IDs and backing
  major/minor are preserved in the witness. Mount source strings are omitted.
- Each device: `path`, `type="char"`, `major`, `minor`, `read`,
  `write`, `ioctl`. Rights are strict booleans, with at least one true.
  Device paths must be explicit leaves below /dev, with no wildcard or FD alias.
- `read_trees`: explicit nonoverlapping file/directory roots. No blanket
  /, /proc, /sys, /dev, /run, /ion or /tmp permission. Root symlinks fail;
  explicit static proc/driver and sys CPU/NUMA/NVIDIA/PCI metadata categories
  match the file-boundary parser. Dynamic process/FD/memory, BAR/config and
  broad kernel/security trees fail. Reviewed internal symlinks must resolve to another approved read tree or
  explicitly approved device.
- `writable_parent`: an approved scratch parent, such as /tmp. Caller-supplied
  writable roots must be proper descendants, nonoverlapping, actual directories
  owned by the exact child identity, mode 0700. Writable-tree files must have
  one link and private permissions; links and special objects fail.
- `collector`: null or exactly /ion/results. Collector mode requires a real
  distinct mounted filesystem there, root /, writable mount/superblock flags,
  no second mount of the same backing major/minor, and root-owned /ion mode 0700.
  The demoted caller must be unable to open the collector.

The verifier inventories **all** /dev character devices, including nodes whose
permissions normally prevent child access. The profile is a complete accepted
inventory, not merely a list of devices believed accessible. Approved nodes'
driver purpose remains owner-reviewed; no NVIDIA major number or name is guessed.
Unexpected block/character devices, FIFOs, socket nodes and device-tree regular
files fail. Only the four fixed standard FD aliases are exempt from ordinary
symlink target rules; the guard separately validates/cleans actual descriptors.

Default Docker's /dev/core -> /proc/kcore alias is deliberately rejected. The
positive local fixture removed this alias before deriving its profile. Production
validation never removes or silently hides an unsupported object.

## Bounds and race handling

| Area            | Bound                                                                                                                 |
| --------------- | --------------------------------------------------------------------------------------------------------------------- |
| Profile         | 64 KiB; 512 mounts; 64 devices; 32 read roots                                                                         |
| Mountinfo       | 256 KiB; 512 records; 4 KiB per line                                                                                  |
| Paths           | 4 KiB UTF-8; clean absolute components, no controls; owner roots reject patterns, enumerated filenames remain literal |
| Tree inspection | 100,000 entries; depth 64; 16 MiB encoded records and a separate 16 MiB enumeration-name budget                       |
| Witness         | 64 KiB before adding its 64-character snapshot hash                                                                   |

The module rejects duplicate mount IDs/points, stacked mountpoints, propagation
or unknown optional fields, unsupported filesystems, incomplete input and backing
result aliases. Only local/pseudo filesystem classes are accepted generally;
a fuse.* filesystem is allowed solely for the unreachable collector role.

Ancestor traversal uses O_PATH, O_DIRECTORY and O_NOFOLLOW. Enumeration opens
readable directory FDs only for inspected directories. This allows traversal
through the supervisor-owned 0711 parent without granting list/write access.
Before/after FD/path metadata checks detect directory swaps and observed mutation.
The snapshot excludes current PID, FD numbers, access times and collection times.
Race checks also compare mtime/ctime; tree hashes record metadata, **not all file
contents**. Immutable image provenance and the accepted host/runtime remain trusted.

These are bounds on inspected data/work, not a promise that arbitrary filesystem
syscalls have bounded latency. The collector can have a userspace filesystem
backing. Unsupported or changing state fails, and no such failure authorizes
another create or Local fallback.

## Evidence

[Test suite](../../packages/job-worker/test_runtime_boundary.py):

- **33 tests pass** on the real Linux x86_64 Docker fixture, cached image
  sha256:2200d363e2a9be95997968a5aecdc823a29d57a301b2593b2cecb76677bf4cca.
- Includes bounded parsers, exact mount matching, result backing aliases, actual
  FIFO/socket/character/block/hardlink rejection, private owner/mode checks,
  directory swap detection, tree byte/depth/entry caps, actual procfs reads,
  and identical full root/demoted-child snapshot witnesses.
- Windows runs the 16 portable parser cases; 17 Linux filesystem cases are
  explicitly skipped there. Windows checks do not establish Linux isolation.
- Fixture uses network=none, pull=never, no credentials and no GPU. MKNOD is a
  test capability for creating negative fixture nodes, not a production worker
  requirement. /dev/core is removed only by the disposable test setup.

Run the portable suite with:

```powershell
python -B packages/job-worker/test_runtime_boundary.py
```

The whole native suite requires an isolated disposable Linux root fixture with
its declared test capabilities and supported topology. It is not a host cleanup
command or a production auto-configuration step.

## Remaining integration and acceptance

Worker/guard wiring, original policy digest binding, stricter IPC syscalls and
Landlock are now implemented and verified together in the
[installed worker checkpoint](57-strict-gpu-worker-integration.md). A successful
snapshot witness alone does not install those controls. The existing socket filter cannot cover arbitrary
device write/mmap or external filesystem proxies.
[Linux RDMA interface](https://docs.kernel.org/infiniband/user_verbs.html),
[FUSE model](https://docs.kernel.org/filesystems/fuse/fuse.html).

Landlock's device rights require ABI 5, but its FD/inheritance and file-operation
limits require separate tests. Neither a profile marker nor an ABI query establishes
the installed restriction.
[Kernel Landlock contract](https://docs.kernel.org/userspace-api/landlock.html).

Real GPU/provider review must verify exact devices, driver/image provenance,
mounted aliases, kernel controls, offline CUDA computation/training, descendant
cleanup and authentic collector readback. No real CUDA or managed Nebius job ran
for this module. See [runtime design research](RESEARCH-gpu-runtime-boundary.md)
and [GPU launch gates](RESEARCH-nebius-gpu-launch-policy.md).
