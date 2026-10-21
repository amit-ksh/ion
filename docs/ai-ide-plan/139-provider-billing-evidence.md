# Provider billing evidence and remaining access gates

21 October 2026, 11:35–11:38 IST. Branch: `ion-prod`.

These are read-only provider observations. No app tool execution, inference,
compute launch, service-control change or credential/permission change was
performed. The completed Windows/Android packages in plans 137–138 are unchanged.

## Composio

The existing saved operator project key returned HTTP 200 for:

- The original execution receipt `log_cOdFFJRrO_pn`. Its ID matches the native
  acceptance receipt; status is `success`, with credential source
  `connected_account`. There is no per-call `instant_charge` field.
- Project usage summary for 19 October 00:00 UTC through the observation time:
  nine tool-call events, two sessions and top-level `instant_charge: "0"` USD.
- The tool-name breakdown: one `GOOGLEDRIVE_GET_ABOUT`, four
  `GMAIL_GET_PROFILE`, three `SLACK_TEST_AUTH` and one `SLACK_API_TEST`.

This reconciles the single Drive acceptance with the project aggregate. The
other eight events are not attributed to this Ion acceptance run; the query
does not establish their caller, execution path or ordinary-plan cost.

The **1,000 micro-USD ($0.001) Drive reservation remains held**. An absent
per-call Instant charge and a zero aggregate Instant charge do not settle the
subscription, ordinary tool-call pricing or separate connected-provider bill.
No budget settlement was fabricated or submitted.

Only the sanitized status, counters, documented charge fields and tool names
were saved. No credentials, connection IDs, identities, tool arguments, Drive
quota values, file contents or provider response payloads were persisted.

Evidence:

- `.waku-cache/ion-account-bound-provider-billing-authenticated-read.json`
- `.waku-cache/ion-account-bound-composio-usage-breakdown.json`
- Original action: `.waku-cache/ion-native-drive-quota-action.json`

The first unprivileged local attempt could not read the project key or reach
the network. Its failure report is preserved separately as
`.waku-cache/ion-account-bound-provider-billing-read.json`. The authenticated
read above is the authoritative provider result.

## Cloudflare

The documented account billing-history endpoint returned **HTTP 403, error
10000**, using the existing DNS token. No alternative credentials were created
and no token permissions were widened. This denial is not evidence of zero
Workers/R2 charges. Unbilled current-period usage is also not reconciled by an
invoice-history read alone.

Remaining evidence: billing/usage access that covers the account's Workers/R2
charges, or an operator-provided redacted current-period billing export. Keep
the existing retained-cost allowance and the invoice gate open.

## Nebius and interactive gates

The current Token Factory billing documentation points to Organisation → Usage
for spending and Billing/Transactions for billing evidence. No supported
invoice API was established from that documentation. The monitoring endpoint
documents metrics, not an invoice total; it was not substituted for billing.
There is no existing provider browser session available to continue without
interactive login.

Sandbox permissions remain the separate blocker recorded in plan 120: the
11:27 IST authenticated discovery denied spawn/list/cancel/import in both
supplied projects. This increment did not repeat discovery or attempt creation.
Compatible immutable images and actual price bounds must also be reviewed
before a real lifecycle test. Application-host VMs remain outside the current
static-only publishing scope.

Physical microphone/paired-phone dictation, native account switching and actual
chat/team/workflow interactions remain pending in plan 132. Billing reads do
not verify those gates, and the final QA handoff condition is not yet met.

## Primary references checked

- [Composio project usage queries](https://docs.composio.dev/reference/api-reference/organization)
- [Composio execution-log receipt](https://docs.composio.dev/reference/api-reference/logs/getLogsToolExecutionById)
- [Instant Tool billing and unknown charges](https://docs.composio.dev/docs/instant-tools)
- [Cloudflare account billing history](https://developers.cloudflare.com/api/resources/billing/subresources/history/)
- [Token Factory billing and consumption](https://docs.tokenfactory.nebius.com/other-capabilities/billing-new)
- [Token Factory monitoring integration](https://docs.tokenfactory.nebius.com/ai-models-inference/observability-api-integrations)
