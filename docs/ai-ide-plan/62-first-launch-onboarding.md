# First-launch connection setup

## Desktop experience

A fresh desktop installation opens a welcome screen followed by four optional setup screens:

1. **AI providers** — Nebius Token Factory key with the existing Save/Test actions; Configure harnesses opens the native provider installation/sign-in controls.
2. **Connected apps** — Composio scoped project key; Connect apps opens the real app connections page for individual app sign-in.
3. **Speech** — shared speech HTTPS URL and service token with Save/Test; Speech settings also provides local dictation configuration.
4. **Personal gateway** — HTTPS gateway URL and one-use owner invite with Pair/Test actions. Device and remote access grants remain managed by the gateway owner.

Every screen provides Back, Next, Skip this step and Skip for now; the final screen provides Open Ion. Keys are optional and no setup step blocks entering the workspace. Enter saves a credential field; normal Tab/Enter/Space controls and Escape are supported. Unsaved keys and invites are cleared when skipping or leaving setup.

First-launch progress persists using the nonsecret `ion_onboarding` setting: version, screen and dismissed state. An unfinished flow resumes its last screen after restart; completed/skipped setup stays dismissed. Existing installations with a saved window or started tasks keep their workspace and can open setup manually.

## Persistent setup access

Missing connection details keep **Finish setup** visible below New task/Search in the sidebar. A count appears once all four cached credential statuses have loaded. The header shows the same access when the sidebar is collapsed; the Settings sidebar also keeps it visible. **Settings → General → Connection setup** remains available even when all details have been saved.

Stored vault/private-file details are marked saved rather than claiming they were verified. A failed connection Test makes setup need attention until the details are saved or successfully tested again. Session-only credentials retain the indicator: they will not survive restart, and model/app execution services need persistent credentials in their own process.

Skipping a service does not disable configured harnesses or change permissions, model choice, sandbox settings, budgets or existing tasks. App account sign-ins, execution targets, per-agent resources/budgets and deployment infrastructure still require their own configuration. A model API key does not approve cloud allocation or spending.

## Runtime and secret boundary

Setup reuses the existing masked credential inputs, vault-backed credential service, provider controls and gateway pairing implementation. No key, token, invite or endpoint is written into the onboarding progress marker. OS-vault access, subprocess/runtime checks and connection tests run on the existing background executors. Render reads only cached statuses and in-memory inputs.

Credential generation checks preserve the latest action. Status refresh excludes saves/tests already in progress, so an older vault snapshot cannot overwrite their result. Native browser/document webviews are hidden while the setup page covers them. The flow uses Ion's existing surface/raised green gradient, text colors, typography and native window controls.

## Owner review checklist

- [ ] Open a fresh desktop profile: Welcome appears and all four steps can be visited.
- [ ] Skip the entire flow without entering a key; New task, projects and installed harnesses remain accessible.
- [ ] Confirm Finish setup is visible below Search, in the collapsed-sidebar header and in Settings.
- [ ] Save/Test Nebius, Composio and speech details; pair/Test a real gateway when these resources are available.
- [ ] Confirm a failed Test keeps the setup indicator and displays useful feedback.
- [ ] Confirm session-only credentials show their persistence limitation and keep setup pending.
- [ ] Restart during an unfinished flow; confirm the last screen resumes.
- [ ] Skip or complete setup, restart, and confirm it stays dismissed while missing details remain accessible.
- [ ] Open an existing task profile: it stays in the workspace instead of interrupting ongoing work.
- [ ] Configure credentials later from Settings or the indicator; confirm the pending count updates and disappears when all details are persistently saved.
- [ ] Check Tab, visible focus, Enter/Space, page scrolling, Escape, both themes and narrow/small windows.
- [ ] Open setup while a browser/document panel is present; confirm it does not cover the setup fields and returns normally afterwards.

Visual/live-credential acceptance is left for owner review; no keys or cloud deployment are supplied by this change. Mobile pairing/onboarding remains its existing separate phone flow.


## Implementation verification

27 client unit tests pass (7 new onboarding/persistence tests). Final locked desktop/daemon binary checks, Rust formatting, generated protocol47 freshness and source diff checks pass. The mechanical UI detector reports no findings. No protocol wire shape changed. Native visual review and real provider/app/speech/gateway acceptance remain the checklist above.
