# Personal-project binding validation passed

23 October 2026, `ion-prod`. This extends onboarding source from plan 178; it
does not complete personal sandbox execution or native acceptance.

## Actual test scope

The five previously declared binding tests have now **executed and passed**.
Production validation lives in
`crates/waku-credentials/src/nebius_project/binding.rs`, imported by the real
credential transport. The credential kind uses that module's namespace constant
so account storage cannot drift between duplicated prefixes.

The small harness imports that exact production file and existing cached
`anyhow`, `serde`, `serde_json` and `sha2` libraries. It supplies no replacement
credential service, vault or account model. Tested behavior:

1. Serialized metadata excludes the raw key and round-trips a valid binding.
2. Another account, signed-out scope and a rotated key fail the match.
3. Compute IDs, URLs, invalid AI IDs and header injection are rejected.
4. Invalid account/key input and future binding versions fail closed.
5. Valid accounts use distinct vault namespaces; invalid scopes are rejected.

`.waku-cache/ion-personal-project-binding-tests.json` preserves source/dependency
hashes, commands, output and terminal exits. Compilation took 3.111 seconds; test
execution took 0.406 seconds: **5 passed, 0 failed**. No actual vault, network,
provider, app/device or package operation was performed.

## Resource and release boundary

The whole desktop/credentials/mobile pipeline retains its 2.5 GiB free physical /
4 GiB commit-headroom gate. Opening observation: 1.63/4.16 GiB, no Cargo/rustc;
that whole pipeline did not start. The small binding-only compile uses a separate
768 MiB / 1.5 GiB guard and observed 1.70/4.78 GiB. It does not establish vault,
HTTP transport, account-store, GUI or mobile type correctness. No process was
stopped and the whole-pipeline guard was not lowered.

Source advanced after plan 179's deferred whole-pipeline receipt; its original
hashes remain preserved. A future Cargo test must use the new filter
`nebius_project::binding::tests` and verify five executions. The old
`nebius_project::tests` filter would select zero, which is not acceptance. Include
the new binding file in the next source snapshot.

Remaining: full credential/desktop/mobile type checks, real vault save/restart/
account switching and read-only project access, scoped native onboarding/team/
logo interactions, packages/installs and sandbox price/image/lifecycle/budget
gates. The binding module and related new source remain uncommitted. Existing
asset/report milestone commits 5e39303 and eb7ee9b remain intact.

## Required next gate remains blocked

The final whole-pipeline retry was deferred before starting any check at
**0.72 GiB free physical / 4.12 GiB commit**. No Cargo/rustc or Ion app/daemon
process was observed in the concluding authorized census. There is no live job
to wait on. Three preserved guarded observations across consecutive goal turns
show the same unmet resource gate. The source/tests above were independent
progress, not proof that the release blocker disappeared.

`.waku-cache/ion-release-blocked-audit-20261023.json` records the receipts and
remaining resource/runtime, native/device and sandbox image/pricing gates. No
process was stopped, exposure enabled, credential changed or paid resource
started. The broad objective remains incomplete; final QA handoff is unsent.
Resume at the full current-source checks when sufficient memory is available;
also include the new binding file and verify the new five-test Cargo filter.
