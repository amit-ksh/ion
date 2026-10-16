# Kimi compiled host packaging

Follow-up to the [production source-task bridge](93-kimi-source-team-bridge.md). The three deferred harness paths remain outside scope. The Windows/x64 compiled package is implemented and verified; the broader document/harness goal stays open.

## Package architecture

The builder verifies 1,960 pinned source inputs from the independently SHA256-verified official Kimi 2.2.0 archive. It compiles the six production workspaces using the frozen `esbuild` 0.27.7 transformer resolved from Kimi's locked `tsx` dependency tree. Relative source imports, package import/export maps and raw/JSON assets become executable JavaScript without a TypeScript runtime loader.

Each native module retains its directory position. This preserves native DI identity, `import.meta` asset paths and platform-specific loaders. The existing additive Teams registration function is shared by source and compiled hosts. Both use the official ACP composition; neither implements another model loop or substitutes a provider account.

The package includes the installed production dependency/peer graph, its package metadata/notices, native prebuilds and auxiliary assets, a Node 24.19.0 runtime and its official license. The builder refuses dependencies outside the frozen checkout. It copies required assets without running install scripts or building a different native backend. MiniDB's packaged text worker is configured explicitly to its compiled entry.

A per-file SHA256/byte inventory covers packaged code, assets, notices and the bundled executable. The short version/login/configuration probes verify the manifest schema/platform and executable bootstrap; full runtime verification happens before ACP loads its engine, authentication or tools, and through `--verify`. Full verification rejects changed/missing/extra files, duplicate/escaping entries, links and foreign platform/architecture packages. The separate owner login configuration is excluded from that immutable inventory. These checks detect package damage and local edits; they are not a signature from a trusted release publisher. Final signed release/download authenticity remains a release gate.

## Build and configure

```powershell
node scripts/kimi-host/build-compiled-host.mjs --source D:/path/to/pinned-kimi-source --node C:/path/to/node-24.19.0.exe --output D:/path/to/new-ion-kimi-package
D:/path/to/new-ion-kimi-package/ion-kimi-host.cmd --verify
D:/path/to/new-ion-kimi-package/ion-kimi-host.cmd configure --native-command C:/path/to/kimi-2.2.0.exe --output D:/path/to/ion-kimi-owner.json
$env:ION_KIMI_HOST_CONFIG='D:/path/to/ion-kimi-owner.json'
$env:ION_KIMI_SOURCE_HOST='D:/path/to/new-ion-kimi-package/ion-kimi-host.cmd'
```

A JavaScript native CLI can use `configure --native-command <node executable> --native-entry <native CLI JavaScript entry> --output <new owner config>`. Native login is delegated to the explicitly selected native executable; `KIMI_CODE_HOME`, native account/model configuration and permissions remain owned by Kimi. Configure and version probes make no authenticated provider call. Configure preserves a differing existing file and supports repeating identical configuration.

The package's relative launcher uses the bundled runtime and can be relocated. It needs the native CLI for owner login delegation and the normal local shell/tool prerequisites. It does not need the original Kimi checkout, pnpm store, tsx loader or machine-wide Node installation after building. The deployed native tests below prove this path with a nonexistent source directory and a hook refusing every module outside the deployed package or the disposable fixture.

## Verification

**37 distinct executed checks pass**, with zero final failures/skips:

| Gate                                               | Cases |
| -------------------------------------------------- | ----: |
| Kimi shared-worker/state/HTTP cases                |    16 |
| Actual compiled native ACP/Teams/Bash driver cases |     7 |
| Compiled package/CLI/isolation/tamper cases        |     8 |
| Existing source-host CLI/Windows-PTY regressions   |     6 |

The two Bun suites have 37 and 46 assertions respectively. Strict fixture TypeScript, Node syntax, affected Rust formatting, Prettier/diff checks and locked offline desktop/both-daemon compiler gates pass. Optional native cases were explicitly run, not counted from ignored results. The final Rust native suite took **82.91 seconds**; eight connections across its seven cases took **7.259–9.503 seconds**. These are observed fixture timings, not a general startup guarantee.

The artifact was built at one cache directory, then moved to `D:/personal/apps/waku/.waku-cache/ion-kimi-compiled-0.2.0-relocated`. Its final inventory has **23,141 files / 322,675,147 bytes**, **1,221 compiled native modules**, **276 dependency installations** and **274 license/notice paths**. Full deployed CLI verification measured 4.259 seconds in the final complete package suite (an earlier complete run measured 3.560 seconds). An isolated diagnostic measured 36 ms manifest loading, 194 ms metadata validation, 2.472 seconds through hashes and 3.147 seconds through complete inventory validation. The final module-graph probe loaded in 2.774 seconds (an earlier complete run measured 5.939 seconds). Earlier full-check/cold-module deadlines expired under load; these timings do not establish broad startup reliability.

The compiled runtime source path was set to an explicitly nonexistent directory. Native child-module hooks refused paths outside the package and private fixture, with no source checkout, pnpm store or machine-wide loader available to resolve missing modules. The real native cases cover owner approval and durable run/result return, denial, Stop, changed-definition refusal, built-in Read, new-driver load/resume and native Bash. Bash executes a literal print and asserts the private callback endpoint/token are absent from its environment; no owner command, credential or paid model is used. The preceding Windows-PTY probe additionally verifies the deployed dependency's native ABI/assets separately from native Bash, which uses the official process service.

Package cases prove changed/missing/extra-file refusal, duplicate/foreign-platform/path-escape refusal, separate owner configuration and login argument/home forwarding, complete deployed inventory, isolated native module resolution, bootstrap-only probe scope, and refusal of changed engine bytes before authentication/tools. A fake login records forwarding only and does not prove authenticated owner access.

### Investigation and retained limits

- The first compile needed TypeScript's `.js`-to-`.ts` relative resolution. The compiler now resolves that explicitly.
- Isolated native loading exposed extensionless published CJS subpaths such as `ajv/dist/2019`. The compiler adds a suffix only when the frozen resolver proves it names the exact same physical file; conditional package exports stay intact.
- After that source correction, native modules were recompiled using the production compiler while reusing dependency bytes only after full inventory verification. The final bootstrap was synchronized with the repository and its entries rehashed. Dependency copying was proven by the preceding fresh build; no source/dependency substitution was used.
- Node has no `--license` option. The package now includes the [official pinned Node 24.19.0 license](https://raw.githubusercontent.com/nodejs/node/v24.19.0/LICENSE), SHA256 `148eacf7863ef4329224a29398623077200a27194aa075569faf4a0a85566ca5`.
- A full-file audit during short native version/login probes exceeded their 15-second bound. Those probes now verify the executable bootstrap and metadata. Full auditing remains required before the native engine starts; a tampered-runtime ACP case proves the boundary. Production deadlines were not increased.
- The standalone import-only fixture now flushes its result and exits after completed imports. It does not claim to have performed a native task or authentication.

### Rerun

```powershell
$env:ION_TEST_KIMI_PACKAGE='D:/path/to/compiled-package'
$env:ION_TEST_KIMI_SOURCE='D:/path/that/intentionally/does/not/exist'
$env:ION_TEST_KIMI_NODE='C:/path/to/verified/node.exe'
bun test scripts/fixtures/kimi-compiled-host.test.ts --timeout 180000
cargo test --locked --offline -p waku-core --lib kimi_source_native_driver_ -- --ignored --nocapture --test-threads=1
```

The actual deployed host uses its bundled runtime. The explicit fixture Node path is used for the fake native-login executable; it does not provide the engine/loader. Existing source-host CLI regressions require the real pinned source path instead of the nonexistent one above.

- [x] Fresh dependency/asset/runtime packaging and pinned source compilation.
- [x] Deployed inventory, external owner configuration and login forwarding.
- [x] Relocation, nonexistent source path and outside-import refusal.
- [x] Actual native ACP/Teams/Bash acceptance with callback credentials absent from Bash.
- [x] Package bounds/notices, startup measurements and desktop/daemon compiler gates.

## Remaining owner/release gates

Authenticated native owner accounts, real model catalog/configuration migration, OAuth/device-login acceptance, full interactive terminal behavior, additional platforms and signed release/update distribution remain separate. The fake HTTP model and fake login can prove execution/wiring/preservation but do not prove authenticated owner access. Phone document/browser/device acceptance and remaining CSS work stay in the current release checklist.
