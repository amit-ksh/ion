# Authenticated Android device presence

21 October 2026. Source implementation for UI-02, the last queued source task
in plan 146. No new native build or physical-device acceptance is claimed.

## Behavior

- Android adds its bounded printable `Platform.constants.Model` label to the
  existing native WebSocket handshake. Missing/invalid names fall back to
  **Android**. This is a client-reported model, not hardware attestation; no
  pairing token, user name or serial number is sent as the label.
- The transport registers a device only after the native handshake and valid
  daemon token. Desktop clients are excluded. Reconnecting sockets share their
  client identity for display deduplication; disconnect removes live presence.
- The desktop polls in a background task every three seconds, retains only the
  in-memory result and notifies on changes. A daemon-connection identity check
  discards stale replies after replacement. No request or lock is added to the
  header's render path.
- The header shows a phone icon plus readable connection text; a bounded,
  truncating label and tooltip retain the name in narrow windows. Multiple
  clients show a count. Failure or no connected clients removes the indicator.

## Source provenance and checks

The server, protocol, daemon command handling and generated TypeScript bindings
are selectively ported from `81827ff`. Before applying, their working bytes
matched its inherited QA baseline `c9d94d4`; after applying, they match the
pinned redesign source exactly (ignoring checkout line endings). The independent
index receives only the presence changes, preserving the existing QA overlay.
The redesign branch/worktree is not merged or edited.

The pinned source includes a real socket regression for rejected authentication,
desktop exclusion, reconnect deduplication and disconnect cleanup. Its earlier
redesign pass is recorded in plan 142; **it has not yet run against a fresh
ion-prod build**. Both generated files are copied from the corresponding
generated output, with a fresh local export check required before packaging.

New Android label tests passed: two tests, nine assertions. Mobile TypeScript
passed. Five changed Rust files parsed using Rustfmt 1.97.1/edition 2024 without
rewriting them. This is not Rust compilation or native interaction acceptance.
No native visual test, microphone use, emulator/APK build, live account action
or paid operation was performed.

At the final resource observation, available memory was approximately 1.05 GiB
physical / 1.05 GiB virtual, with no Cargo/Rust/Java build observed. This remains
below the unchanged 2.5/4 GiB guards. The next package preparation must require
the new socket regression and generated-binding check before the release build.

All six requested UI changes are now implemented in source. Their applicable
fresh native build, Light/Dark/System, keyboard/narrow-window, paired Android
and publishing interaction gates remain open in plans 120/132/142/146.
