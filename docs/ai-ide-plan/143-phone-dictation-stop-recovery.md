# Phone dictation: native Stop failure recovery

21 October 2026. This is partial progress on RD-02 in
[142](142-redesign-remaining-acceptance.md), not a physical recording pass or a
resolution of the emulator's unexplained return to idle during startup.

## Reproduced defect

An isolated lifecycle fixture executes the actual `usePhoneDictation` hook with
native audio, file transfer and lifecycle doubles. No microphone or provider is
used. After successful start, a native `stream.stop()` exception left the hook
in `recording`, with its shared recording mode/timer not cleaned up. The focused
test failed before the fix with:

```text
AssertionError: Native stop failure must not strand the hook in recording
actual: recording
expected: idle
```

The failure was an exception escaping before cleanup, rather than a stale UI
state or late status callback. Discard had the same escaping-stop risk.

## Source changes

- Stop failure discards buffered audio, clears timer/target and resets the shared
  audio mode. It reports a bounded recovery message and never sends unfinished
  audio to transcription.
- Discard completes cleanup even when the native stop call throws.
- A stream still reporting itself active blocks a new start and instructs the
  user to close/reopen the screen. An idle UI is not evidence of native release;
  the message does not claim hardware stopped successfully.
- No native package, microphone permission, connection, global MCP setting or
  redesign source was modified. Existing unrelated QA edits are preserved.

## Verification

```powershell
bun test apps/mobile/src/hooks/use-phone-dictation.test.ts apps/mobile/src/lib/phone-dictation-wav.test.ts
node node_modules/typescript/bin/tsc --noEmit -p apps/mobile/tsconfig.json
```

Three Bun tests pass, including a separate-process hook fixture with ten
scenarios: successful capture/transcription; discard; denied permission;
backgrounding; task/context replacement; sign-out during pending startup;
native startup rejection; stale transcription after context replacement;
native Stop failure; and native Discard failure. WAV format/bounds tests pass.
Mobile TypeScript passes. Module doubles stay in the child process so they
cannot replace React/native modules in other test files. The hook runner models
state/ref/effect commits; it is not real React Native or Android hardware.

## Remaining RD-02 acceptance

The original emulator symptom did not reproduce in the normal mocked startup
scenario. There is no captured native failure/lifecycle trace proving its cause.
It must remain open for an authorized emulator run, followed by separately
authorized physical microphone/ARM acceptance. Disabled host audio is not an
established cause. The latest redesign allowance was one APK and emulator only;
this code change does not authorize another build or Moto test.

Next release provenance must include this new mobile hook change as well as the
earlier account-boundary fixes. Existing APKs predate this fix. Keep the separate
redesign warning-copy/package gate RD-04 pending until deliberate integration
and an independently authorized refresh.
