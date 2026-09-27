# Native Nebius runtime and manual check

Milestone 2 adds a daemon-owned Token Factory chat completion driver. It
verifies the selected API model ID against `GET /v1/models` on each start,
prefers NVIDIA Nemotron 3 Super when present, and streams text and function
calls in the provider's sequence. The supported local tools are project file
listing, UTF-8 file reading, and approved file replacement. File edits require
an exact old-content match and a one-time approval showing the proposed new
content. The driver does not run arbitrary shell commands.

The task journal is under the user's local data directory in
`ion/nebius-sessions`. It stores conversation messages and tool results, not
the API key. A write is marked in that journal before it touches a file. On
resume, the driver compares the file with the previous and proposed content
and records the observed outcome without applying the write again. An
ambiguous file state blocks resume for manual review.

The desktop and daemon access one OS vault entry through the shared
`waku-credentials` crate. When the vault is unavailable, Settings can hold a
key in desktop memory for a catalog test, but a daemon task cannot start. The
runtime returns an explicit setup error in that case.

## Milestone 2 acceptance checklist

Use a disposable local project with a small text file. The existing
`bun ./scripts/dev.ts` watcher should own the debug app; start it only if it
is absent.

- [ ] Confirm the watcher successfully built and relaunched both the app and
      daemon after the latest source change.
- [ ] In **Settings → Models & Speech**, save your own Nebius Token Factory key
      and click **Test**. Confirm **Stored in OS vault** and a nonempty live model
      catalog. Do not paste the key into chat or a project file.
- [ ] Create a Nebius task in the disposable project. Ask it to read the text
      file, make a small edit, and explain the change.
- [ ] Check that streamed text and tool activity appear in the order received,
      with no private provider markers shown in the transcript.
- [ ] Check that the edit pauses for approval, the complete proposed content is
      readable, and **Allow once** and **Deny** work with keyboard as well as
      mouse.
- [ ] Allow the edit once. Inspect the local diff and transcript; confirm the
      file contains exactly the approved content.
- [ ] Stop and reopen the task, then send a follow-up prompt. Confirm it
      continues under the same task and does not apply the edit again.
- [ ] In a separate edit request, choose **Deny** and confirm the file stays
      unchanged. With no key saved, confirm starting a Nebius task gives a
      setup error and makes no file change; restore the key afterward.
- [ ] Review the real provider result and explicitly approve Milestone 2 before
      work starts on Milestone 3.

The live check is pending because no Nebius key is available in this
development environment. Milestone 3 remains gated on the user's review.

[Nebius function calling documentation](https://docs.tokenfactory.nebius.com/ai-models-inference/function-calling)
describes the OpenAI-compatible tool message loop used here.
