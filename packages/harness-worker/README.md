# Native Linux harness worker image

This recipe builds the production `ion-harness-worker` with the repository's
unchanged release profile and lockfile. It keeps source/build caches out of the
runtime image and executes the worker as UID/GID 1000. It does not register,
deploy, select a cloud account or enable unsupported adapters.

## Build context and bases

Supply two reviewed Linux image references pinned by SHA256. `BUILD_BASE` needs
the matching Rust toolchain, native build dependencies and Cargo access (or a
reviewed cached/vendored builder). `RUNTIME_BASE` must provide the selected exact
CLI/runtime, compatible system libraries, CA certificates and Debian-compatible
`getent`, `groupadd`, `useradd` and `install` utilities. It must not already own
UID/GID 1000. The final ABI check executes no model request.

Do not send the entire checkout as a Docker context. Export one reviewed Git
revision into a new staging directory. This includes tracked files only; inspect
that revision for secrets before sharing it with an external builder. Local
ignored caches, vaults, native login files and untracked configuration are excluded.
For example, from the repository (replace the base references and paths):

```powershell
$ionRevision = git rev-parse HEAD
$ionContext = Join-Path $env:TEMP ('ion-worker-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $ionContext | Out-Null
$ionArchive = Join-Path $ionContext 'source.tar'
git archive --format=tar "--output=$ionArchive" $ionRevision
Copy-Item -LiteralPath packages/harness-worker/Dockerfile -Destination $ionContext
docker build --file (Join-Path $ionContext 'Dockerfile') `
  --build-arg BUILD_BASE=YOUR_REVIEWED_BUILDER@sha256:EXACT_DIGEST `
  --build-arg RUNTIME_BASE=YOUR_REVIEWED_HARNESS_RUNTIME@sha256:EXACT_DIGEST `
  --build-arg SOURCE_REVISION=$ionRevision `
  --tag ion-harness-worker:$ionRevision $ionContext
```

`CARGO_OFFLINE=true` is optional for a builder whose exact locked dependencies
are already present. It does not change the manifest or relax lockfile checking.
Pin and record the resulting runtime image digest before import/registration.
This multi-stage recipe never copies the builder's HOME or Cargo cache to runtime.

## Native host bootstrap and capability manifest

Install each selected native CLI and its verified runtime in `RUNTIME_BASE` using
that CLI's reviewed public release. Keep inference credentials outside build
arguments, image layers, manifests, prompts and requests. Configure accounts with
the private registered host's bootstrap. Supply the native model catalogs and
`/ion/host.json` only after checking exact model input/output limits and adapter
readiness. The manifest grants no additional filesystem/app/command access.

ConTree's `shell:false` environment does not inherit image environment variables.
Use host manifest **version 2** with `execution` set to
`{"uid":1000,"gid":1000,"home":"/home/ion"}`. Ion sends these UID/GID fields
explicitly in the ConTree instance request, along with only PATH and HOME; OCI
`USER` alone is insufficient. The worker checks its effective UID/GID, HOME and
`/ion/work` before any native authentication/catalog lookup. The image creates
that private working directory and removes inherited setuid/setgid bits.

A host-owned bootstrap wrapper or the native user's private home must supply the
selected login where required; validate that on the target before registering an
immutable UUID. The wrapper must preserve the registered user/home/cwd. Legacy
version-1 manifests and saved requests keep their original identity and omitted
`execution` field for exact recovery; do not use them for new nonroot images.
This base recipe does not embed an unverified bootstrap or make local credentials
available remotely.

## Acceptance

- Check the image runs as 1000:1000, includes only the release executable and
  runtime dependencies, and contains no source/build cache or account secret.
- Run `python3 test_contract.py /usr/local/bin/ion-harness-worker` against the
  release image. These actual-binary boundary checks use isolated empty homes,
  no credentials and no live inference; errors must have empty stdout and bounded
  sanitized diagnostics. In a disposable, offline Linux container with user
  1000:1000, HOME `/home/ion`, cwd `/ion/work` and writable private `/tmp`, add
  `--registered-runtime`. This also checks that wrong UID/GID/HOME/cwd cannot reach
  native discovery and the valid context reaches only a temporary fake Codex
  help probe. Its catalog, executable and marker are private fixture files; no
  installed native CLI, authentication or model request is used.
- Configure a supported exact harness/model privately, inspect `/ion/host.json`,
  and run a complete native worker request with usage and action validation.
- Test stop/restart/lost acknowledgement against the registered host. Retain the
  original request and token reservation through uncertain outcomes; no Local retry.
- Accept Linux native CLI versions and additional account routes independently.
  A local release build does not establish live provider or cloud acceptance.

See [registered execution](../../docs/ai-ide-plan/22-harness-adapters-remote-execution.md)
and [release checklist](../../docs/ai-ide-plan/20-agent-teams-release-checklist.md).

## Recorded Linux release acceptance (2026-10-07)

Source `ad21a87178555ef52cf6a6e5074a97039becee29` built offline with the
unchanged locked release profile. **Ten actual-executable test groups passed**
in a network-disabled, read-only container with all capabilities dropped and
no-new-privileges. UID/GID, HOME/cwd, 0700 owner directories, root-owned 0555 ELF,
absence of build/source caches and removed `/usr` setuid/setgid bits were checked.

- Local release image ID: `sha256:d65a15825be163e9b86757090d3ff9d601525c9b5183d9a75dc90a6eb49f4170`.
- Worker ELF SHA256: `6d404557a22f03e719b16d112a4269175ba5550003572e82448a36aee27c5d5c` (7,565,808 bytes).
- Pinned test runtime: `python@sha256:05cda9777409a9c3ffddd94a4c476b79f0769a0b4857f0c7ed9226b6800b0d6f`.
- Verified local release builder: `sha256:7e2b81307b90c7dd13e8f3d293fb23af558bed6e3a4fd63b9f3264ca45a8d57b`.

The test runtime contains no supported provider installation/login/host manifest
and is not registered as a cloud execution target. Install and accept the exact
native runtime/account privately before producing a deployment image; its digest
will differ. Local image IDs are recorded evidence, not a published registry URL.
