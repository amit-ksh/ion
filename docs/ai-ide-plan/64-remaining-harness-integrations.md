# Remaining harness integrations

Updated 2026-10-14. The active user goal reopens the remaining harness integrations previously deferred on 2026-10-08.

## Pi and Oh My Pi normal-task Teams bridge

The native session now loads a scoped extension exposing the five existing Ion Teams actions: list, invoke, inspect runs/run, and read accepted team documents. Native streamed tool-call/session IDs witness each request, and changed arguments, failed tool callbacks, terminal events, Stop and bridge closure revoke pending authorization. Exact invocation replay uses the same durable source-task/tool-call identity across native restart.

Native account/model/settings stay selected. Source tool invocation uses the existing owner approval and scope intersection, with bounded bridge workers and no bypass of pinned run limits. Production availability follows the existing Full access mode.

Twelve shared source-bridge regressions pass, including changed/replayed arguments, Stop, failed native callbacks and delayed headers on explicitly nonblocking sockets. Actual installed **Pi 0.84.1** and **Oh My Pi 18.6.1** pass the combined native RPC fixture: the native extension calls `list_teams`, returns the saved Engineering team and sends that result back in a second provider request. Exactly the five Ion tools are advertised. This uses isolated fake accounts and localhost Responses, with no paid provider call.

The fixture now distinguishes Pi's `--no-tools` (which also disables extensions) from `--no-builtin-tools`, and handles OMP's `GET /models` catalog preflight separately. The real Windows bridge explicitly restores blocking mode on accepted sockets before bounded background reads. An intermittent Pi startup timeout was observed during investigation; a final run with the installed npm launcher passed in 25.47 seconds for both dialects. Broad startup reliability and owner-account acceptance remain review items.

## Amp

The official plugin AI helper can select an exact native model and set `maxTokens`, but `generate` returns only string/object without a metered usage receipt. Native threaded modes do not independently establish a pinned model/token ceiling. Current plugin tool context exposes UI/logger/thread without a native tool-call ID, so normal-task callback authorization also needs a real witnessed identity route. No substitute provider/account is used. Teams completion remains an implementation gap until that native allowance/account/usage contract is verified.

## Cursor

Current official SDK 1.0.37 metadata and complete declarations were inspected. Its 150 declaration files (6,656,963 bytes) expose no `maxOutputTokens`, `maxTokens`, `maxSteps`, `maxTurns` or `maxCost` completion control. Archive SHA-256: `992e4889e298f1ac5dfeeda6cf4db5eb1008aa3b1ea03883a17c2ea052f1efd2`; npm SHA512 integrity verified. Disabling tools/retries and eventual usage accounting cannot prove a preauthorized spending ceiling. Native Cursor account/model stays selected; full Teams completion remains open.

[Official SDK metadata](https://registry.npmjs.org/@cursor%2fsdk/1.0.37), [official SDK documentation](https://cursor.com/docs/sdk/typescript), [Amp plugin reference](https://ampcode.com/manual#plugins), and the earlier [Amp](RESEARCH-amp-teams-contract.md)/[Cursor](RESEARCH-cursor-teams-contract.md) contract investigations record the native seams and missing proof. No live account or paid provider call was used.

## Review and remaining work

- [x] Fresh native Pi/Oh My Pi fixture acceptance after final compilation.
- [ ] Review Pi cold startup under memory pressure and slow native catalog/extension initialization.
- [ ] Verify native authenticated accounts, delegation/result review, Stop and restart on actual owner projects.
- [ ] Complete Amp native model/allowance/usage proof and source callback identity.
- [ ] Complete Cursor native completion ceiling and account/catalog limits, with adversarial dispatch/cancel fixtures.
- [ ] Broader normal-task bridges for other selected installed harnesses where native routes are available.

These are current goal requirements, not deferred successes. The full goal remains active.
