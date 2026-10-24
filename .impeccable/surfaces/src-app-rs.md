---
version: 1
slug: "src-app-rs"
primary_target: "src/app.rs"
related_targets: ["src/theme.rs","src/app/composer.rs","src/app/sidebar.rs","src/app/workspace_control.rs"]
---

## Scope

Ion desktop app (Rust/GPUI), Operate mode: home, composer, sidebar, Work pages (Tasks, Teams, Decisions, Artifacts, Workflows), settings and onboarding. Mobile and website inherit the same world later.

Pinned by the user: forge reference aesthetics (airy surfaces, chip composer, soft mint glow), Ion lime kept as brand and action color with a mint glow, light and dark designed with equal care (System default). Paperclip informs the team view: a live org chart with roles, budgets and heartbeats.

## Direction contract

THESIS: Ion is the pass for agent work: every request fires a ticket onto a live rail, and you stand at the pass sending results out or back. Refuses the chat clone where a transcript and session list are the whole product.

OWN-WORLD: White ticket stock on a pale cool canvas (dark: graphite tickets on near-black); one mint-to-lime glow field behind the prompt; lime only for fire, approve and selection; pill chips with icon, label and chevron; mono for order numbers, timers and the home headline; hairline-ruled compact tickets.

STORY: The user sees who is working on what and what waits at the pass, fires a request, then approves or sends back results.

FIRST VIEWPORT: Home: mono headline centred near 30% height; 720px chip composer below; glow across the lower third; under the composer a rail of live tickets (station, agent, timer, state); sidebar left.

FORM: Ticket Rail, #3 of my list, seed 1e3917e2.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Signature interaction

A ticket changes station in place: its state stamp and timer update without the row leaving the rail; tickets waiting at the pass hold a lime edge until opened.

## Open decisions

- Whether full-colour SVG figures (anatomy) render in GPUI empty states; otherwise they live on the website only.
