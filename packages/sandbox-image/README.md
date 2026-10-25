# Personal Token Factory sandbox image

When someone saves a Token Factory key and AI project in Settings, Ion builds
its sandbox image inside that project through the Sandboxes API. Nobody needs
Docker, a registry, or the ConTree CLI.

1. Import the `python:3.12.14-slim-bookworm` base (once per project). The
   importer cannot pull by digest, so this is the most specific upstream tag.
2. Upload the installed static Linux `ion-harness-worker`, the command-worker
   files from `packages/job-worker`, the model manifest and `setup.sh` as
   content-addressed project files.
3. Run `setup.sh` once as root, offline, with `disposable: false`. Its result is
   a new private image.
4. Tag that image `ion-sandbox:<recipe digest>`. Any change to an installed file
   changes the tag, so an app update builds a fresh image.

The same image serves both roles. It is the sandbox agent's target (the model
worker runs as UID 1000) and its command image (the root supervisor runs commands
as UID 1000 with sockets denied). It contains no credential. Each disposable
model run receives the saved key as a mounted file at
`/home/ion/.config/ion/nebius-key`, mode 0400.

`setup.sh` records the kernel release it runs on, because the command guard
accepts only listed kernels. If Token Factory moves to a new kernel, choose
**Rebuild** in Settings.

## Shipping the worker

Every desktop installer carries `ion-harness-worker-linux-x86_64`: beside the
executables on Windows and Linux, and in `Contents/Resources` on macOS. The
packagers build it with `scripts/sandbox-worker.ts` (cargo-zigbuild, static
musl). Ion hashes the installed file into the image tag, so a new worker always
builds a new image. For a local debug build, put one beside the debug binaries:

```sh
bun scripts/sandbox-worker.ts target/debug
```
