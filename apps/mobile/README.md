# ion Mobile

## Android download

The current sideload build is [Ion 1.0.1 for ARM64](https://github.com/amit-ksh/ion/releases/download/v0.1.24/ion-android-1.0.1-arm64.apk), version code 2, for Android API 24+. Its installation as an update and nonvisual startup were checked on a Moto G32 running Android 13. The launcher name is **Ion** and the existing Ion icon and signing identity are retained. Open the downloaded APK in Files and install it over the existing app; no uninstall or data clearing is needed.

The APK uses the existing test certificate. Verify it against the adjacent [SHA-256 file](https://github.com/amit-ksh/ion/releases/download/v0.1.24/ion-android-1.0.1-arm64.apk.sha256). The internal package ID remains `sh.waku.mobile` for update and data compatibility. See [release verification](../../docs/android-release-1.0.1.md).

Expo client for connecting to one or more remote ion daemons from iOS,
Android.

## Run

From the repository root:

```sh
bun install
bun --filter @waku/mobile ios
bun --filter @waku/mobile android
```

## Connect

In ion Desktop, enable the remote daemon and copy its WebSocket address and
token. Add those values in the mobile app. Use `wss://` outside a trusted LAN
or private tailnet; the token grants full control of the daemon host.

Saved profile metadata stays in app storage. On iOS and Android, daemon tokens
are stored separately in the device keychain through Expo SecureStore.

Phone microphone dictation sends a short recording to the **selected daemon**
for local NeMo-Speech.cpp transcription, then inserts editable text. To use
desktop ASR, select the desktop daemon and install its ASR runtime/model. The
phone asks for microphone and transfer consent when you tap the mic; Discard
sends no audio. See the [Milestone 7 setup and review](../../docs/ai-ide-plan/14-phone-asr-cloud-execution.md).

An owner-managed Linux host can run `waku-daemon` headlessly for cloud tasks
while the desktop is off. This uses a full-control daemon token and a
host-local Nebius key file; see the [cloud host guide](../../docs/ai-ide-plan/14-phone-asr-cloud-execution.md#personal-cloud-execution).

## Personal gateway

Open **Personal gateway** from the start or Daemons screen. At the gateway's
`/owner` page, create a one-use invite with **Review approvals**, **Read tasks**,
and **Control tasks** as needed, then pair the phone using the trusted HTTPS
URL. Use a separate **Publish desktop state** invite for the desktop. The
phone can inspect tasks, ordered activity, artifact metadata, diffs, and exact
pending provider requests; answer, steer, and stop commands pass through the
gateway's durable single-claim inbox. Stale feeds disable controls. A phone
can review Composio actions, while the gateway owner executes an approved
external action separately. The phone token stays in the native keychain.
Removing the local pairing does not revoke the device: revoke it in `/owner`
too.

The existing direct daemon connection still offers its full daemon controls;
the personal gateway path uses separate device grants. The [Milestone 5
checklist](../../docs/ai-ide-plan/12-mobile-polish.md) records live checks.

### Optional task notifications

The phone asks for notification permission only after **Enable task
notifications** is pressed. The gateway sends a generic “Task needs input”
notice for a new provider question or permission; no task title, arguments,
transcript, or diff is included in the push payload. Disabling notifications
or revoking the device removes its gateway subscription.

Push needs an Expo EAS project ID in `expo.extra.eas.projectId`, platform push
credentials (FCM v1 for Android, APNs for iOS), and an installed development
or release build. Android push is unavailable in Expo Go. Follow the official
[Expo push setup guide](https://docs.expo.dev/push-notifications/push-notifications-setup/)
to supply those account-owned resources. The app shows a setup error instead
of requesting a token when the project ID is missing. The gateway checks Expo
push tickets and later receipts; a failed or uncertain send never authorizes
or repeats a task command.

### Installable phone builds

`eas.json` has a `preview` profile for an installable test build. With the
Expo EAS CLI installed and signed into the owner's account, run these from
`apps/mobile` after linking the project with `eas init`:

```sh
eas build --platform android --profile preview
eas device:create
eas build --platform ios --profile preview
```

The iOS device registration is needed only for an iOS preview build; register
the device before starting that build. EAS will ask the account owner to set
up signing credentials. Follow its build link to install on the phone. The
`production` profile is reserved for a later store/submission build. See the
[Expo internal distribution guide](https://docs.expo.dev/build/internal-distribution/)
for platform requirements.

ion is licensed under the repository's GNU GPL v3.0-only license.

## Managed Ion Cloud account

Native onboarding and Daemons settings include cloud sign-in when the operator sets EXPO_PUBLIC_ION_CLOUD_ORIGIN to the deployed HTTPS origin. Register ion://auth/callback on the Auth0 Native mobile client and build a new native app. Sign-in does not overwrite a self-hosted pairing. See the [configuration and device checklist](../../docs/ai-ide-plan/109-mobile-cloud-login.md).

Managed speech, HTML publication and deployment controls: [implementation and deferred acceptance 111](../../docs/ai-ide-plan/111-managed-services-app-integration.md). Project source snapshots are submitted from desktop. No new APK or device acceptance is included in this increment.

## Local dictation

The task microphone records up to 60 seconds and sends the WAV to the selected paired desktop for local NVIDIA Parakeet TDT-CTC 110M recognition. Keep that desktop online. Prepare the approximately 146 MB model in desktop Settings → Models & Speech, or wait for the first-use download. No speech-service endpoint or token is required. See [local dictation scope and acceptance](../../docs/ai-ide-plan/116-110m-and-serverless-implementation.md).
