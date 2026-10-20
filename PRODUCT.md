# Ion

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

## Users

People coordinating coding agents, projects, teams, documents, and connected apps from a native desktop IDE or an Android companion.

## Product Purpose

Turn requests into reviewable work through explicit harness and environment choices, approvals, budgets, and inspectable results.

## Operating Context

Desktop uses Rust and GPUI. Android uses React Native and Expo. Existing Token Factory inference, harness adapters, local Parakeet speech, accounts, and connected Apps remain functional. The Android client connects to selected desktop runtimes.

## Capabilities and Constraints

- Dashboard and Inbox remain hidden. Decisions lives in Work; usage lives in Settings.
- `@` includes files, teams, and apps; `@@` filters teams; `@@@` filters apps.
- Harness selection stays per agent. Local and Nebius are explicit choices; there is no automatic environment mode.
- Publishing is static only through the existing Cloudflare Worker and private R2. Local build/export and review precede upload.
- No new adapters, backend hosting, cloud builds, paid resources, iOS implementation, or release/signing/device changes.
- Virtualization, stream cadence, background work, reduced motion, and readable light/dark themes are product requirements.

## Brand Commitments

Ion keeps its own identity. The supplied visual references and restrained Nebius lime/green inspiration guide this redesign; neither the Nebius logo nor a claim of ownership belongs in Ion's identity.

## Evidence on Hand

Seven user-supplied reference images in the task brief; existing source; inherited native and Android QA captures under `docs/qa/2026-10-18-ion`. Existing QA images document the previous build, not acceptance of this redesign.

## Product Principles

- Put the next useful action beside the object it affects.
- Show real state and contextual labels; avoid static explanation and fictional metrics.
- Preserve task continuity across navigation, streaming, and recovery.
- Keep platform conventions, readable text, and keyboard/touch operation ahead of decoration.

## Accessibility & Inclusion

Native desktop controls need keyboard operation and visible focus. Android controls need accessible names, scalable text, and at least 48dp touch targets. Meaning must not depend on color, hover, or motion alone.
