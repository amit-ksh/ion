# GitLab MCP warning: endpoint diagnosis

21 October 2026. RD-03 from plan 142. Global MCP configuration was read but not
changed; no credentials were printed or sent, no login flow was started and no
MCP tool was called.

## Reproduced signal

The default local Codex configuration contains an enabled HTTP GitLab server on
`gitlab.com`, with a path different from the documented `/api/v4/mcp`. Sanitized
metadata is in `.waku-cache/ion-gitlab-mcp-config-observation-20261021.json`.
The absence of an explicit bearer/header setting does **not** establish absence
of cached OAuth authentication; the credential cache was not inspected.

At **17:53 IST**, the same fixed, unauthenticated MCP `initialize` request was
sent to the configured endpoint and the documented endpoint, with redirects
disabled and responses bounded. No configuration, tools or repository state
was changed:

| Endpoint | Observed response |
| --- | --- |
| Configured GitLab endpoint | HTTP 422, `text/html`, HTML body, no OAuth challenge |
| `https://gitlab.com/api/v4/mcp` | HTTP 401, `application/json`, OAuth challenge present |

Evidence: `.waku-cache/ion-gitlab-mcp-endpoint-comparison-20261021.json`.
The helper `.waku-cache/check-ion-gitlab-mcp-endpoint.py` refuses to overwrite
that observation. Bodies, tokens and the configured path are not copied into
the report. The 401 is expected for this unauthenticated check, not successful
authentication or tool acceptance.

## Finding and limits

The configured endpoint reproduces the reported HTTP 422 and serves an HTML
response instead of an MCP authentication challenge. Endpoint mismatch is a
concrete integration problem to correct before investigating cached OAuth or
protocol compatibility. The default config observation does not prove every
possible runtime override used during the historical Android run.

The earlier actual Android Codex chat returned `ION_REVIEW_OK` and reached idle.
Its success remains distinct from this optional integration's startup warning.
Do not mark Codex broken, silently disable GitLab, remove authentication caches
or repeat model/tool calls to hide the warning.

## Owner-controlled next step

Review correction of the GitLab server URL to `https://gitlab.com/api/v4/mcp`,
preserving other intentional settings, then perform the approved OAuth flow and
one read-only MCP acceptance check. This session's goal explicitly withholds
global MCP changes, so the correction has **not** been applied.

GitLab documents the endpoint, OAuth access with `mcp` scope, and supported
fine-grained personal-token permission. Legacy/project/group access tokens are
not substitutes for those methods. See the
[official server setup](https://docs.gitlab.com/user/model_context_protocol/mcp_server/).
Do not delete credential caches just because the
[troubleshooting guide](https://docs.gitlab.com/user/model_context_protocol/mcp_server_troubleshooting/)
lists cache reset as an option; it requires separate owner review here.

RD-03 remains open for owner-approved correction/login and authenticated
acceptance. RD-04 warning-copy integration/package acceptance is separate.
