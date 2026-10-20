# Managed app connection foundation

20 October 2026. This milestone covers account-scoped discovery and connection
management. It does not complete the separate approved managed-action milestone.

## Implemented

- Operator project key stays in encrypted Worker secret `COMPOSIO_PROJECT_KEY`.
  No app binary, model prompt or public bootstrap contains it.
- The authenticated Ion workspace hash maps to a stable `ion_<hash>` Composio
  user. The backend stores/resumes its session. Caller user/session overrides,
  operator connections and provider credential/MCP envelopes are not forwarded.
- The catalog returns bounded safe pages, server search and owned account
  summaries. Connection details are independently checked against that user.
- Connect Links use session authorization. Revoke operations re-check ownership;
  redirects are restricted to Composio HTTPS hosts.
- Connect/revoke dispatches retain exact operation identities. Unknown outcomes
  are recorded durably and not retried automatically. Requests have persistent
  per-account/global rate limits and bounded operation history.
- Composio Instant Tools, proxy execution and its own sandbox are disabled.
  These settings do not configure a Nebius Token Factory sandbox.
- Native Apps supports backend pages/search, connect/reconnect/revoke, owned app
  mentions and background loads. Account changes clear cached accounts and grant
  selection; managed requests verify the original workspace before refresh and
  dispatch. Optional existing personal projects remain compatible.
- Onboarding presents user app sign-in rather than requiring a project key;
  personal project configuration is an Advanced option.

## Verified

- Operator key read-only catalog/auth-config/account checks: all 200.
- Real owner-scoped Composio session creation: 201; catalog: 200, 50 items and
  zero connected accounts. The returned configuration confirms disabled Instant
  usage, workbench and proxy execution. No real app OAuth has completed yet.
- Six backend regressions, 120 assertions; four targeted native managed-boundary
  checks. Full native credential suite: 49 pass, two separately exercised live
  speech tests ignored in the ordinary suite.
- Five real local workerd tests pass, including public JWT routing, rejected
  identity overrides, hiding operator accounts and session reuse after restart.
  These use fixture provider responses; they do not replace real app acceptance.
- TypeScript Worker checking and desktop/daemon offline compilation pass.
- The new Worker test caught `redirect: "error"` being unsupported in workerd.
  Manual redirect handling fixed that boundary; redirects are rejected by status
  before provider response decoding. No automatic effect retry was introduced.
- Deployed Worker version `6d055bcd-a3c6-42f7-9349-41a7878bb8ea` advertises
  `managedApps: true`; public health/bootstrap return 200; an unauthenticated
  app-catalog request with spoofed identity headers returns 401.

Logs: `.waku-cache/ion-composio-key-check.json`,
`.waku-cache/ion-composio-live-check.json`,
`.waku-cache/ion-managed-apps-live-public.json`,
`.waku-cache/ion-managed-apps-worker-tests.log`,
`.waku-cache/ion-managed-apps-targeted.log`,
`.waku-cache/ion-managed-apps-check.log`.

## Remaining acceptance and next milestone

- Real authenticated live-gateway catalog/connect/revoke/reconnect and actual
  native UI acceptance, with a user authorizing their own app connection.
- One safe real provider tool call with a non-empty Composio execution log ID.
- Managed tool discovery, versioned approvals and exact action dispatch through
  native chats, teams and workflows. Connection availability does not imply those
  action paths are complete.
- Operator reconciliation of ambiguous session/link operations remains explicit;
  the server does not silently recreate sessions after uncertain initialization.
- No native visual tests, new installer, Android build or device OAuth acceptance
  have been performed for this foundation. Existing QA changes remain unstaged.

The combined budget implementation was included in the deployment above. The
additional aggregate-usage response field is a subsequent source change and
needs the next deployment; it is not claimed as verified in that version.

No Nebius VM or sandbox was provisioned for this milestone. Ordinary users still
need to authorize their own app account, but do not receive the operator's key or
personal connected accounts.
