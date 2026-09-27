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

### Local speech

- [ ] Install the official NeMo-Speech.cpp runtime for this OS and run
      `nemo-speech doctor`. Download `nemotron-3.5` while online with
      `nemo-speech pull nemotron-3.5`; then disconnect the network for the
      dictation check.
- [ ] In **Settings → Models & Speech**, confirm local speech reports the
      runtime and model state, offers an explicit language choice, and asks for
      no API key.
- [ ] Dictate an English prompt from the default microphone. Confirm the
      transcript appears in the composer as editable text and is not sent to
      the agent until you press Send.
- [ ] Repeat with Hindi (`hi-IN`). Check partial and final text, correction,
      cancel, and a second recording. Confirm the first recording cannot
      overwrite a newer draft.
- [ ] With the network disconnected, confirm both languages still work and
      audio is not uploaded. Check microphone-denied and missing-model errors
      lead to usable setup steps.
- [ ] Measure first-use startup time, live text cadence, and peak memory on
      target hardware; record hardware and model build alongside results.

### Personal gateway

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

Milestone 4 is **in progress**. The desktop currently detects the installed
`nemo-speech` executable and can transcribe a selected local WAV with
`nemotron-3.5` in Auto, English (`en-US`), or Hindi (`hi-IN`) mode. The full
transcript is reviewable before it is inserted into the editable composer. The
app does not send it as a task automatically. The Composio **Test** control now
makes a live, read-only request to the Tools API; it requires a scoped project
key with Tools read permission. The gateway pairing token is still only stored
in the desktop vault.

Install NeMo-Speech.cpp using the [official installation guide](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/install.md).
For Windows, the installer is a PowerShell script; review it before running it.
For macOS or Linux, use the documented shell installer. Then run:

```text
nemo-speech --version
nemo-speech doctor
nemo-speech pull nemotron-3.5
```

The pull needs internet access once and verifies the model artifact. After it
finishes, a local WAV can be transcribed while offline. In ion, open
**Settings → Models & Speech**, choose the language, and select **Transcribe
WAV…**. Review the transcript and choose **Insert into composer**.

Live microphone capture, editable partial text, gateway deployment, pairing,
connected-account listing, and approval-gated remote tool execution are not
implemented yet. Their checklist items stay unchecked.

## Primary references

- [NeMo-Speech.cpp installation](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/install.md),
  [CLI](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/cli.md), and
  [Nemotron 3.5 model](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/asr/models.md)
- [Composio scoped project key permissions](https://docs.composio.dev/reference/authenticating-to-composio/project-api-key-permissions)
- [Composio session configuration](https://docs.composio.dev/docs/configuring-sessions)
