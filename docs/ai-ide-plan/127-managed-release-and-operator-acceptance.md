# Managed release and operator acceptance

21 October 2026. Branch `ion-prod`. Windows and Android only; iOS and deferred
harness adapters remain excluded. This record distinguishes local verification
from real-provider and physical-user acceptance.

## Completed implementation commits

- `bc9d21c`: managed sandbox recovery requires the original saved payload and
  account-stamped provider witness; reconciliation retains the original project.
- `6bde270`: native chat app discovery/approvals, managed sandbox commands,
  persistent conversation/team/workflow/routine account ownership, scoped delayed
  app execution, cached agent target selection and managed usage summaries.
- Earlier managed app, budget, speech and gateway commits remain listed in
  [120](120-managed-access-completion-checklist.md).

Targeted verification: cloud backend 43 passed / 444 assertions; local workerd
seven passed; managed native regressions eight passed; command suite 22 passed /
one ignored; managed credential tests five passed / one ignored. Desktop/daemon
compilation, mobile TypeScript, isolated app-worker and controller TypeScript
passed. The broader mobile suite passed 428 tests with 27 skipped native-fixture
checks. These counts do not imply real app OAuth or live sandbox execution.

The gateway runs recovery source `bc9d21c` as Worker
`33865782-96d7-4d4b-b02d-1811d10dd615`. Public health/bootstrap succeed;
anonymous effects/sandbox requests and spoofed catalog identity are denied.
This was the pre-R2 deployment. The full storage deployment and model download
now pass production checks; see [128](128-r2-speech-and-publishing.md). Managed
sandbox and app compute remain unavailable; HTML storage is configured.

## Operator host prepared without starting cloud compute

The existing Ubuntu host now has Node 22.22.1, Python, make and the C++ toolchain.
The official Nebius SDK 0.2.81 and gRPC 1.14.6 were installed in the separate
Linux user cache `/home/ksami/.cache/ion-compute-controller`, leaving Windows
workspace dependencies intact. Imports for the actual SDK credential reader,
instance and operation APIs succeed. The compiled `fs-ext` addon acquired and
released a private disposable lock successfully. No credentials were supplied to
the probe, no provider request was made, and no controller loop/service started.

The approved project `project-e04f8x7qbc00cshy8v3t19` returned an empty instance
list during closeout. This work created no VM, sandbox, image import, disk or
app worker. There is no started resource from this increment to stop. Do not
interpret this check as a statement about an unrelated account or external invoice.

## Build provenance and acceptance

Windows and Android packaging is in progress from source `6bde270`, including
111 pre-existing dirty QA files. `.waku-cache/ion-managed-release-inputs.json`
records the tracked patch SHA-256 and hashes of untracked build-source files.
The patch and source files are preserved. Generated Android native files were
refreshed from the current config, including `RECORD_AUDIO`, the Ion display name
and the Auth0 browser dependency. Android uses the existing local debug signing
identity for a sideloadable release-variant APK; it is not a store-signed release.

Moto G32 is connected and authorized again. The earlier installation reports
microphone permission ungranted. A build or upgrade does not prove actual login,
pairing or recorded dictation. No screenshots or native visual tests were taken.

## Remaining live gates

### Task status at R2 activation

| Workstream | Implementation | Acceptance |
| --- | --- | --- |
| Managed login/account isolation | Complete | Real Rust callback/vault/refresh and scope checks passed; native GUI switching open |
| Managed inference | Complete | One live request passed earlier; native streaming/tool/cancel checks open |
| Composio connections/actions | Complete | Owner Drive consent and approved read-only action passed; native chat/Teams UI open |
| Budget/owner controls | Complete | Provider invoice reconciliation open |
| Token Factory sandbox | Complete for configured targets | Access permissions, reviewed images and live lifecycle open |
| Local speech/model validation | Complete | Fixed speech/silence and HF download passed; R2 HTTPS digest passed |
| R2 migration | Private bucket, upload, binding and routes deployed | Full HTTPS and native first-download/cache checks passed; refreshed desktop release pending |
| HTML publication | Complete | Real owner private/public publish, one-use view, revocation and cleanup passed; native interaction open |
| App deployment/controller | Implemented; Linux SDK imports/native lock verified | Isolated host/image setup and real start/stop/charges open |
| Windows installer | Packaging in progress | R2-enabled refresh and installation/runtime checks open |
| Android APK | Built, signed test package verified | In-place upgrade, user login, cold start and microphone grant passed; pairing/dictation open |
| QA handover | Prepared checklist | Conditional on agreed acceptance gates |

1. **Publishing acceptance:** R2 is active; private binding, routes, DNS/TLS,
   model hash/native download and real owner private/public publication,
   one-use viewing, replay denial, revocation and cleanup pass. Native publishing
   interaction and a second-account live denial check remain open.
2. **Sandbox access and images:** the saved key is accepted by `whoami`, but spawn,
   list, cancel and import permissions are false. Confirm the Token Factory Project settings ID (which must not be assumed from
   the Compute profile alone) and project/caller access,
   reviewed immutable images, manifests, credential bootstrap and price/retention
   ceilings before enabling any sandbox policy. Do not substitute an unverified
   image or raw operation-cost units.
3. **Isolated app runtime:** provide a budget-approved dedicated VM/image with the
   required gVisor, quota-aware storage and networking, plus scoped controller
   credentials. Do not install the app worker on the user's computer. The Linux
   SDK prerequisite above is complete; provider start/stop/billing acceptance is not.
4. **Native interactions:** real Rust PKCE/vault, refresh, managed controls/catalog,
   owner Drive consent and one approved read-only provider action passed. Actual
   chat/Teams/workflow UI and account-switch acceptance remain open; see [129](129-native-account-drive-and-android-acceptance.md).
5. **User/device acceptance:** real microphone permission, recorded speech,
   paired-desktop dictation, installer clean-install/upgrade and account switching.
   Existing fixture tests and shared TypeScript Auth0 checks do not replace these.
6. **Billing:** reconcile actual retained/provider charges before releasing unknown
   holds. Existing combined reservations and owner controls remain in force.

No additional paid inference or app action was run for these changes. Keep the
USD 10 target and USD 15 cumulative ceiling; no pricing/policy placeholder is an
authorization to provision a resource. Final QA handover remains conditional on
the agreed live gates rather than being declared complete from local tests.

## Read-only provisioning price check

Current [Nebius Compute pricing](https://docs.nebius.com/compute/resources/pricing)
was checked before considering any VM setup. In eu-west2 the documented non-GPU
AMD rates are USD 0.015 per vCPU-hour and USD 0.0045 per GiB-hour from 1 October
2026. Thus 4 vCPU / 16 GiB would be USD 0.132/hour before tax (derived estimate,
not a reserved capacity quote). Stopped compute is not charged, but disks remain
billable: non-replicated SSD is USD 0.053/GiB per 730 hours. Actual region/preset,
minimum boot/data disks, runtime image setup, taxes and retained duration still
need verification before provisioning; no VM was created based on this estimate.

## Android build-tool recovery

The first Android attempt found the Auth0 browser dependency missing from the
local offline cache. It was fetched through the normal Gradle dependency resolver.
The subsequent native build exposed Ninja 1.10.2's Windows path limit: an existing
React Native header path was 262 characters. A short cache alias alone did not
remove old paths from generated Ninja dependency databases. Three generated
metadata files were archived under `.waku-cache/ion-old-ninja-deps`; source and
Gradle downloads were preserved. Reanimated then passed; Expo core exposed the
same old-tool limit in its own cached dependencies.

The official [Ninja 1.13.2 Windows archive](https://github.com/ninja-build/ninja/releases/tag/v1.13.2)
was downloaded after matching the release asset's published SHA-256:
`07fc8261b42b20e71d1720b39068c2e14ffcee6396b76fb7a795fb460b78dc65`.
Only the workspace Android SDK CMake 3.22.1 `ninja.exe` was replaced; the old
binary is backed up. The installed executable SHA-256 is
`e52a7ad9538d9618c67a0bd777964e2eec8a30f68b810a2f6adce1f2daf847b8`.
Against the identical Expo core graph, the old executable exits 1 with the path
error and the new executable exits 0. Full APK acceptance is recorded after the
complete build finishes. No Windows registry or app-source change was required.

The final retry uses a temporary `I:` alias for the verified existing Gradle cache
to reuse current build outputs. The build wrapper removes only its own alias in
`finally`. When rebuilding elsewhere, use a Windows Ninja version with long-path
support; the temporary alias does not become an application/runtime requirement.

Read-only Compute discovery also confirms `cpu-d3` is offered in the approved
project with a `2vcpu-8gb` preset. Applying the documented component prices yields
USD 0.066/hour before tax for that preset. This is an estimate, not a capacity
reservation or a substitute for verifying boot/data disk and retention charges.

The native read-only gateway acceptance check was repeated using the actual
host-user credential-vault context, not only the restricted test context. It
still stops immediately because the native app is signed out, before making an
HTTP request. R2 readiness was also rechecked and still reports error 10042.
These gates therefore remain real external prerequisites rather than inferred
failures from the local test sandbox.
