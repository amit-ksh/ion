# Cloudflare dashboard metrics supplied by the owner

22 October 2026, Asia/Kolkata. This records the owner's screenshot without
converting its counters into an invoice or claiming live fix attribution.

## Visible evidence

| Dashboard counter | Displayed value | Interpretation |
| --- | --- | --- |
| Requests | Approximately 91k | Invocation count in the selected dashboard window |
| Errors | 0 | No invocation errors shown in that window |
| Storage operations | Approximately 783k | Separate storage activity counter; not 783k SQL rows written or stored bytes |
| Request wall time | 1.34 ms | A latency metric, not a request count or storage measurement |
| Billable duration | 30.55 GB-sec | Memory-weighted compute duration, not GB of stored data |

The active deployment identifier starts `8c731253`, matching the deployed
recovery Worker recorded in plan 162. The visible chart's request bars stop
around the evening of 21 October; the later visible period has no bars. This
suggests reduced or absent traffic, but does not identify whether a client was
stopped, disconnected or updated. It cannot establish the new native client's
before/after rates: current packages have not yet passed the new build gates.

The blue upward percentages compare usage with an earlier period; they are not
percentages of quota consumed. The screenshot does not show the selected range,
namespace heading, plan, a 90% quota meter or the SQL read/write split.

## Conditional quota interpretation

The current official [Durable Objects pricing](https://developers.cloudflare.com/durable-objects/platform/pricing/)
lists Free-plan allowances of 100,000 requests/day, 5 million SQL rows read/day,
100,000 SQL rows written/day and 13,000 GB-sec/day. These are separate limits;
storage activity cannot be substituted for rows written. Limits reset at
00:00 UTC, which is 05:30 IST.

If approximately 91k requests belong to one Free-plan UTC quota day, they are
approximately 91% of that daily request allowance. Neither the account plan nor
that exact window is verified by this crop. Cloudflare's
[metrics documentation](https://developers.cloudflare.com/durable-objects/observability/metrics-and-analytics/)
says the namespace Metrics view defaults to the last 24 hours, which is not
necessarily the same as the UTC quota day.

## Fresh API observation and remaining verification

A bounded read using the saved Wrangler OAuth session refused to send an API
request because its credential was expired. No token or credential body was
printed, no permission expanded, and no service/data changed. Preserve the
observation at `.waku-cache/ion-cloudflare-metrics-20261022.json`; it is not a
fresh zero-usage report. A standard CLI session check is separate from that
original observation and must be inspected before claiming refreshed access.

Required live evidence remains: exact UTC window and account plan; AccountRelay
versus Platform counts; separate SQL rows read/written and stored bytes; actual
client version/running state; bounded idle/active rates after new-client rollout.
Do not delete Durable Object data, disable security or upgrade the plan to
resolve this task.

## Successful live follow-up at 05:24 IST

The normal `wrangler whoami` session check returned exit 0 and reported an OAuth
login without browser interaction or new permissions. A separately recorded
follow-up GraphQL query then returned **HTTP 200, no GraphQL errors**. The
original expired-session observation is preserved, not overwritten.

Query range: **20 October 23:54:24.886 UTC to 21 October 23:54:24.886 UTC**,
which ends **22 October 05:24:24.886 IST**. These are reported analytics counts,
not an invoice or proof of the screenshot's selected window.

| Namespace | Requests | SQL rows read | SQL rows written | Maximum stored bytes | Duration GB-sec |
| --- | ---: | ---: | ---: | ---: | ---: |
| AccountRelay | 88,662 | 745,325 | 22,082 | 212,992 | 30.1914 |
| Platform | 1,674 | 40,978 | 46 | 36,864 | 18.8298 |

All reported invocation statuses are successful. AccountRelay's request counts
are entirely under earlier script versions `0f0f8849` (78,146) and `b20ab52f`
(10,516). No request is reported under `8c731253` for AccountRelay. Its last
nonempty hourly bucket begins 14:00 UTC (19:30 IST); all following buckets from
15:00 UTC (20:30 IST) onward have no reported AccountRelay requests. Platform
reports six requests under the current version, consistent with narrow backend
checks rather than proving a paired client's new relay behavior.

This establishes a quiet reported period and separates requests, SQL row
operations and bytes. It does not attribute the drop to the current client or
settle the 10–50-user pilot's live idle/active rates. The updated native client
still needs packaging and the agreed actual acceptance.

Evidence: `.waku-cache/ion-cloudflare-metrics-followup-20261022.json`,
`ion-cloudflare-metrics-summary-20261022.json` and
`ion-cloudflare-cli-session-check-20261022.json`. Hourly groups are well below
the query's 1,000-row limits; neither invocation nor periodic results were
truncated by those limits. The separate quota-day summary excludes the partial
previous UTC day; it must not be confused with a rolling dashboard window.
