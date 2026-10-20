# Managed app actions

20 October 2026. Implementation milestone; live app OAuth and provider-action
acceptance remain pending user authorization of an app connection.

## Execution boundary

The gateway persists a proposal containing the exact connected account, action,
published version and arguments. It verifies connection ownership and active
status during discovery, preparation and immediately before dispatch. Approval
expires after five minutes. Execution accepts only the saved proposal ID.

Native Teams and workflows use the existing coordinator's exact user decision,
grant intersection and effect journal. When no personal Composio key is present,
the credential service prepares a managed proposal. Only consuming the prepared
effect after the coordinator approves it can approve and execute that proposal.
The captured workspace is rechecked before every authenticated request, including
before refresh. Provider project keys never reach a client or model.

Connections use per-user Composio sessions. Existing pinned direct actions retain
the v3.1 execution endpoint during migration because the session execution schema
does not establish a published-version pin. No automatic migration to latest,
Instant Tools, hosted accounts, proxy execution or meta-tool execution is allowed.

The owner can enable or stop shared app actions separately in Cloud Account
settings. Upgraded deployments default this permission OFF. Connection discovery
and user OAuth remain usable while effects are OFF. Personal-key behavior remains
available through the existing optional advanced setting.

## Durability and spending

The backend commits an uncertain state and USD 0.001 reservation before the single
provider POST. Restart, duplicate requests and lost acknowledgements cannot repeat
an effect. A completed receipt is returned from storage; an uncertain outcome needs
operator reconciliation. Connection disconnection or changed ownership prevents
dispatch. Only bounded result data and a provider log ID leave the backend;
credential/session envelopes are removed.

The reservation conservatively covers the ordinary Pro tool-call, managed OAuth
and direct execution list rates on the [Composio pricing page](https://composio.dev/pricing).
Absence of an Instant charge is not proof of zero cost. The hold remains until a
trusted billing receipt is reconciled. Per-account and global request limits,
one request per account in flight, history bounds and the combined Ion spending
envelope apply. This does not upgrade the operator's plan or enable paid add-ons.

## Verification

- Seven backend action/control tests pass: 89 assertions covering denial,
  expiration, changed arguments, foreign accounts, disconnection, owner stop,
  redaction, uncertain outcomes, duplicate submission and persisted request limits.
- Six real local workerd tests pass, including authenticated public routing,
  cross-account denial, owner-only enablement, exact payload dispatch and
  durable receipts after restart. Fixtures do not prove a live provider action.
- Native credential and desktop compilation are recorded in the completion
  checklist after their gates finish.

Still required: authorize a real user-owned app, exercise its read-only action
through Ion, record its actual Composio log ID, and verify account switching.
Normal chat can invoke a configured team to use its granted app actions. Direct
solo-chat action discovery/execution is a separate path and is not claimed here.
