# Redesign source review: connected-app lifecycle

21 October 2026. Direct source review of the committed redesign between
inherited QA snapshot `c9d94d437944b60a3bf497ab84180ad1e5a77d2b` and `81827ff`.
Parallel reviewers produced no findings because they hit the account usage
limit. This direct review covers app reconnect and sign-out behavior; it is
not a whole-branch approval or visual/device acceptance.

## Standards

**P2 — Preserve the remote-revocation warning.** In the redesign's
`apps/mobile/src/components/cloud-account-section.tsx`, `signOut()` ignores the
`{ revoked: false }` result. The controller deliberately returns this when
remote revocation cannot be confirmed. The previous component showed a native
alert; the redesign silently discards it. `DESIGN.md` says, “Do keep real
connection, draft, busy, error, approval, and conflict states visible.” Local
sign-out may succeed while a remote device grant still needs review. Restore
the warning without blocking local sign-out or exposing account secrets.

## Spec

**P2 — Renew the connection operation after confirmed disconnect.** In the
redesign's `apps/mobile/src/app/apps.tsx`, the successful disconnect clears its
own operation ID and continuation URL but retains `connect:<toolkit>`. A later
Connect reuses that original ID. `ManagedApps.once()` correctly replays its
completed result, including the old revoked account and redirect URL; no new
provider link is created. This breaks reconnect in the same mounted Apps
screen. `PRODUCT.md` requires “Ion account authentication and managed Apps
connections remain.” Clear that toolkit's completed connect identity only
after confirmed revocation; preserve identities during uncertain retries.

## Reproduction and proposed correction

The ignored `review-ion-redesign-app-lifecycle.ts` fixture extracts the actual
two callbacks with the installed Babel parser and Bun transpiler. Source bytes
are compared against `81827ff`. It invokes the actual `ManagedApps` and
`ServiceStore` against a local provider fixture, without rendering React Native
or contacting an external provider.

| Check | Committed redesign | Proposed patch in memory |
| --- | --- | --- |
| Connect → confirmed Disconnect → Connect | Same connect operation ID; one provider link creation; revoked URL reopened | New operation ID; two provider link creations; distinct new URL |
| Local sign-out returns `revoked: false` | Zero user notices | One native warning alert |

Sanitized [evidence](150-redesign-app-lifecycle-evidence.json) and the
[proposed patch](150-redesign-app-lifecycle-proposed.patch) are saved together.
The patch is **not applied** to either implementation tree. Its verification is
callback/backend fixture evidence, not whole-component or native UX acceptance.
Recheck source provenance before applying; never overwrite newer redesign work.

No live account was disconnected, no OAuth was opened, no physical microphone
was used, no APK was built and no redesign merge occurred. The broader source,
visual/keyboard, TalkBack and account/device gates remain open. Current package
preflight observed 1.55 GiB physical / 2.17 GiB virtual available, still below
the 2.5/4 GiB guards; no competing Cargo/Rust/Java build was observed.

Summary: **one Standards finding, one Spec finding**, both P2 lifecycle
regressions. They need correction during authorized redesign integration.
