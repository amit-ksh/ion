# Milestone 4: local speech and personal gateway

Milestone 4 adds local NVIDIA Nemotron 3.5 ASR Streaming 0.6B through
NeMo-Speech.cpp and a guided setup for a gateway owned by the user. The
gateway's Composio key must be scoped to the tools and session operations it
needs. Speech audio stays on the desktop. The gateway and Composio credentials
must not enter model context, task journals, or project files.

Implementation can proceed before the user has a Nebius key. Acceptance stays
open until the user completes the real-provider checks. The earlier
[Milestone 2](09-native-nebius-runtime.md#milestone-2-acceptance-checklist) and
[Milestone 3](10-workflows-github.md#milestone-3-review-checklist) lists remain
open too.

## Milestone 4 review checklist

### Your setup before review

- [x] Install Inno Setup 6 locally. `ISCC.exe` was found at
      `C:\Program Files (x86)\Inno Setup 6\ISCC.exe` on September 27, 2026.
- [x] Build the Windows release with `bun scripts/bundle-windows.ts`.
      Inno Setup 6.7.1 compiled `Waku-0.1.19-x86_64-Setup.exe` and the portable
      ZIP in `target/release/`; the script verified the bundled NeMo 0.1.0
      executable. The ZIP contains the app, daemon, NeMo executable, and model
      index. This local build is unsigned.
- [ ] Install the locally built Windows release package on a separate test
      machine or after saving any running work. The installer closes an existing
      ion process during installation. Do not install over the active debug app
      while its development watcher owns it.
- [ ] When available, save and **Test** your Nebius Token Factory key in
      **Settings → Models & Speech**. This unlocks the deferred Milestone 2 and
      3 live reviews. Keep the key out of chat, source files, and screenshots.
- [ ] Later, create your own Nebius cloud project with billing and choose a
      region and personal budget. Review the exact VM/service, estimated cost,
      network exposure, and deletion plan before provisioning the gateway.
- [ ] Later, create a Composio project and connect only a disposable test
      account with the OAuth scopes needed for a read and one reviewable write.
      Create a scoped project key for the gateway only after its required
      session/tool permissions are finalized. Do not paste it into chat.

The Nebius Token Factory key is for model inference. The later Nebius cloud
project runs the personal gateway. The Composio project key grants connected
app access. These are separate resources and credentials.

### Local speech

- [ ] On Windows, install the release package on a machine without a separate
      NeMo-Speech.cpp installation. Confirm **Dictate** can find the bundled
      runtime. On macOS/Linux, install NeMo-Speech.cpp separately for now.
- [ ] While online, start **Dictate** and let ion download the verified
      `nemotron-3.5` model on first use. Then disconnect the network for the
      repeat dictation check.
- [ ] In **Settings → Models & Speech**, confirm local speech reports the
      runtime and model state, offers an explicit language choice, and asks for
      no API key.
- [ ] Dictate an English prompt from the default microphone. Confirm a final
      transcript preview appears, then **Insert dictation** puts editable text
      in the composer. Nothing should be sent until you press Send.
- [ ] Repeat with Hindi (`hi-IN`). Check correction in the composer, cancel,
      and a second recording. Switch tasks before inserting and confirm the
      transcript cannot enter the wrong task.
- [ ] With the network disconnected, confirm both languages still work and
      audio is not uploaded. Check microphone-denied and missing-model errors
      lead to usable setup steps.
- [ ] Measure first-use download/setup time, final transcription latency, and
      peak memory on target hardware; record hardware and model build.
- [ ] Once live partial text is implemented, measure its update cadence and
      confirm partial text can be corrected before sending.

### Personal gateway

- [x] Local authorization core rejects expired/reused pairing invites,
      revoked devices, out-of-grant tools/scopes, mismatched write approval,
      and duplicate execution claims in automated policy checks.
- [x] Persist device/proposal state locally with transactional single-use
      pairing and write claims. Restart and separate-connection checks pass.
- [ ] Add TLS service, owner approval UI, Composio adapter, and deployment
      instructions. Review these locally before provisioning any cloud resource.

- [ ] In your own Nebius account, review the proposed resource, region,
      expected user-borne cost, and removal steps before provisioning.
- [ ] Create a Composio **scoped project API key** with only the session and
      tool permissions the gateway needs. Connect a test account with the
      narrow OAuth scopes needed for the demonstration.
- [ ] Complete the guided gateway setup and pair this desktop with a
      short-lived token. Confirm a bad or expired token is rejected and a
      revoked device cannot reconnect.
- [ ] List the connected account, toolkit, and requested scopes. Try a safe
      read tool and inspect its auditable result without any raw credential.
- [ ] Propose an external write, deny it, and confirm no write occurred.
      Repeat and approve the exact target, body, and scope; confirm the
      external result and no duplicate replay after reconnect.
- [ ] Disconnect the gateway. Confirm dependent work pauses and queued
      decisions remain reviewable. Reconnect and confirm it resumes without
      silently broadening grants or replaying a write.
- [ ] Delete the test gateway resource and rotate or delete the Composio key.
      Confirm the desktop can remove its pairing token.

### Gate decision

- [ ] Review the real local ASR and paired gateway evidence, then explicitly
      approve Milestone 4 or request fixes.

The user has no Nebius key yet. Live Milestone 2 and 3 provider checks and the
paired Nebius gateway demonstration remain pending; unchecked items are not
treated as passed.

## Implementation status and setup

Milestone 4 is **in progress**. Windows release packaging now includes the
verified NeMo-Speech.cpp 0.1.0 CPU runtime and its notices; local development
also detects the standard per-user installation when PATH has not refreshed.
The composer has a **Dictate** control that
prepares the `nemotron-3.5` model on first use, records the default microphone
locally, and stages the final transcript for review. **Stop dictation** ends
recording and starts transcription; **Cancel** discards a recording. The
selected language is Auto, English (`en-US`), or Hindi (`hi-IN`) in **Settings
→ Models & Speech**. The same page can transcribe a selected local WAV. The
app inserts neither transcript into the task until the user chooses **Insert
dictation** or **Insert into composer** and then presses Send. The Composio
**Test** control now
makes a live, read-only request to the Tools API; it requires a scoped project
key with Tools read permission. The gateway pairing token is still only stored
in the desktop vault.

For a packaged Windows release, install ion and use **Dictate** directly. The
first use downloads the model while online, then the microphone works offline.
The OS may prompt for microphone permission. A separate NeMo-Speech.cpp install
is not needed for that Windows package. The release CI fetches the pinned 0.1.0
CPU archive with upstream SHA-256 verification before packaging it.

For development builds on Windows and current macOS/Linux packages, install
NeMo-Speech.cpp using the [official installation guide](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/install.md).
Then run:

```text
nemo-speech --version
nemo-speech doctor
nemo-speech pull nemotron-3.5
```

The model pull needs internet access once and verifies the roughly 742 MB GGUF
artifact. The app also runs the same preparation when **Dictate** is used,
showing a preparation message before it turns on the microphone. After it finishes,
recording and transcription can work offline. In ion, open **Settings → Models
& Speech** to choose the language. Then use **Dictate** in the composer, or
select **Transcribe WAV…** for an existing file. Review the transcript and
insert it into the editable composer.

The Windows installer and portable ZIP include the runtime but not the model.
macOS and Linux runtime bundling is still pending. The installer was compiled
but not installed, and no live microphone recording was exercised in this
development session, so the direct-install experience still needs the checklist
review above.

Editable live partial text, gateway deployment, pairing, connected-account
listing, and approval-gated remote tool execution are not implemented yet.
The new `packages/personal-gateway` policy core has transactional local SQLite
storage. It checks pairing and write decisions across restarts and separate
connections, but is not yet connected to the app, a network listener, or
Composio. Its local tests do not count as a paired gateway demonstration.
The microphone path has not been exercised with a real model or mic in this
development session, so its checklist items remain unchecked.

## Primary references

- [NeMo-Speech.cpp installation](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/install.md),
  [CLI](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/cli.md), and
  [Nemotron 3.5 model](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/asr/models.md)
- [Composio scoped project key permissions](https://docs.composio.dev/reference/authenticating-to-composio/project-api-key-permissions)
- [Composio session configuration](https://docs.composio.dev/docs/configuring-sessions)
