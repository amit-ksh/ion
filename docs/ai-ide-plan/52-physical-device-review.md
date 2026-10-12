# Physical device setup and Ion review

Status on 2026-10-12: the owner has an **iPhone 15** and **Moto G32**. No device was accessed or marked accepted. Nebius VM/SSH target and HTTPS hostname are unavailable. Additional harness adapters remain deferred.

## What is available now

The Windows development machine has Java 17 and the declared Expo 57.0.18 dependencies. No Android SDK/platform-tools was found in the configured environment or default install location. A native phone build, signing account, device OS versions and USB authorization have not been verified. Java availability alone does not prove compatibility with the generated Android Gradle configuration.

Run from the implementation checkout:

```powershell
node scripts/mobile-check.mjs check
# Use only if this checkout shares another checkout's installed dependencies:
node scripts/mobile-check.mjs check --dependencies D:/personal/apps/waku
# Once your SDK and USB debugging are configured:
node scripts/mobile-check.mjs check --sdk "<absolute-Android-SDK-directory>" --devices
```

The default command reads bounded local files and SDK folders. `--devices` optionally runs a five-second ADB inventory against the local server at port 5037. It can start that server, which may communicate with already authorized devices; redirected ADB server environment settings are excluded. It prints counts, never device serials. Exit 0 means the inspection succeeded; `acceptance: pending` remains until actual device review. It installs nothing, opens no microphone, starts no Ion watcher. Default inspection uses no network; optional ADB uses its local server transport. Twelve tests cover missing/wrong dependencies, configuration bounds, argument rejection, device bounds and sanitized errors.

## Moto G32

- [ ] Install Android Studio/SDK command-line tools and platform-tools. Install the platform/build-tools required by this project's generated Android configuration; do not infer them from Java alone.
- [ ] Set `ANDROID_HOME` to the actual SDK directory, and configure the matching JDK. Enable Developer options and USB debugging, connect Moto G32 by USB and accept its computer authorization.
- [ ] Run the preflight with `--devices`; verify the phone's exact model/OS privately. Counts alone do not identify the phone.
- [ ] From `apps/mobile`, install a native development build with the existing package script:

  ```powershell
  bun run android --device
  ```

  Expo may generate native build files and start Metro as part of this explicit build. Use the current Metro session when it already exists. The installed Expo CLI's `run:android --help` confirms `--device` support. No build was launched during this audit.

- [ ] Alternatively use the existing signed **preview** build profile after configuring your EAS account:

  ```powershell
  bunx eas-cli build --platform android --profile preview
  ```

  Install the resulting authenticated APK on the phone and record its SHA/source revision. This command has not been run. Internal distribution supports Android APK installation. [Expo APK builds](https://docs.expo.dev/build-reference/apk/).

USB debugging and device authorization are required for ADB access. [Android ADB](https://developer.android.com/tools/adb).

## iPhone 15

- [ ] Record the exact iOS version and identify the signed native build privately.
- [ ] For a local build, use a Mac with Xcode/signing configured, then from `apps/mobile` run `bun run ios --device`. Windows cannot compile this local iOS target.
- [ ] Or configure an authorized EAS/Apple Developer account, register the actual phone and create its ad hoc preview:

  ```powershell
  bunx eas-cli device:create
  bunx eas-cli build --platform ios --profile preview
  ```

- [ ] Install from the authenticated build link, retaining build SHA, source revision and signing context privately. These commands have not been run and no signing credentials are requested in chat.

Native modules require the actual Ion native build. Expo Go/browser previews cannot establish terminal, paste-input, SecureStore or microphone acceptance. [Expo native development builds](https://docs.expo.dev/develop/development-builds/introduction/). Ad hoc iOS installation depends on registered devices and signing. [Expo internal distribution](https://docs.expo.dev/build/internal-distribution/).

The source currently retains compatibility identifiers `sh.waku.mobile` and scheme `waku`; the displayed app name is Ion. No installation identifier migration is part of this review.

## Connect and review the same revision

Upgrade phone, desktop and daemon together using the current protocol bindings. Use the actual reachable private daemon/personal gateway and existing pairing screen; do not enter a guessed address. `127.0.0.1` on a phone denotes that phone. Keep pairing tokens, ASR tokens, device IDs and raw audio/transcripts out of public logs and commits. Connection setup is described in [phone ASR/cloud execution](14-phone-asr-cloud-execution.md) and [headless gateway owner](31-headless-gateway-owner.md).

Execute on **both** phones:

- [ ] Ion launches and its primary navigation, settings, Inbox and task draft work.
- [ ] Pairing authorization/revocation works; background/foreground and changed-owner views do not reuse stale responses.
- [ ] Direct Artifacts: HTML Reading/Source, Markdown/JSON, PDF page/zoom/scroll and exact original export. Review [HTML](47-phone-html-reading.md) and [PDF](48-phone-pdf-pages.md).
- [ ] Personal gateway: workflow evidence uses the accepted original run; a PDF page changes and zooms correctly. Interrupted reads, expired/revoked grants and changed evidence refuse stale results. Preserve exact exported original bytes.
- [ ] Cost and remote execution history use the selected original owner/project; read access does not silently authorize Stop/Cleanup or external actions.
- [ ] Text scaling, VoiceOver/TalkBack labels, focus, hit targets, both themes and reduce-motion remain usable. Large documents and rapid navigation stay responsive.
- [ ] Record failures and actual observations with revision/build/OS/time/evidence identity; do not turn pending checks into passes from automated transport results.

## Speech: what remains required

A cloud speech deployment cannot start until an actual Linux/Nebius target and trusted HTTPS hostname exist. Use [the deployment package and 46-case device review](50-speech-deployment-device-review.md). All 46 initial cases remain pending, including readiness without a microphone prompt, real dictation, wrong token/origin, cancellation/background, TLS/socket redirects and concurrent desktop/phone recognition.

The installed Windows CPU binary and 741,548,352-byte Nemotron model passed integrity/metadata inspection. Subsequent [actual local CPU synthetic recognition and concurrent streams](54-native-speech-recognition.md) passed; no human microphone, phone or cloud recognition is established. The ignored smoke WAV is empty; supply reviewed nonempty mono PCM16 fixtures and sufficient free RAM before loading the actual model. Phone dictation through the paired desktop daemon can be reviewed separately from cloud speech after that daemon's local speech service is working. Existing setup: [speech acceptance](41-speech-service-acceptance.md).

## Outstanding owner inputs

- [ ] Android SDK/build prerequisites and native builds for both phones.
- [ ] Authorized Apple/EAS signing route or a Mac for iPhone installation.
- [ ] Actual reachable paired daemon/gateway and private authorization.
- [ ] Reviewed speech fixtures and future Nebius target/HTTPS hostname.
- [ ] Physical observations for each checklist item and the 46 speech cases.

These are acceptance/setup gates. This checklist and preflight do not claim a cloud deployment or a physical-device pass.
