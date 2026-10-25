---
name: Ion
description: A native workbench where requests become tickets on a live rail, and lime marks the one action that moves work forward.
colors:
  primary: "#c2ef68"
  on-primary: "#1b2510"
  dark-canvas: "#0f1211"
  dark-sidebar: "#0c0f0e"
  dark-surface: "#121615"
  dark-raised: "#1a1f1d"
  dark-composer: "#171c1a"
  dark-inset: "#0d100f"
  dark-ticket: "#181d1b"
  dark-text: "#eef2ec"
  dark-text-secondary: "#bac2b7"
  dark-text-tertiary: "#9da69a"
  dark-text-ghost: "#8a9487"
  dark-border: "hsla(150, 10%, 90%, 0.075)"
  dark-border-strong: "hsla(150, 10%, 90%, 0.14)"
  dark-ticket-border: "hsla(150, 10%, 90%, 0.09)"
  dark-overlay: "hsla(150, 10%, 90%, 0.05)"
  dark-overlay-strong: "hsla(150, 10%, 90%, 0.09)"
  dark-accent: "#c2ef68"
  dark-accent-soft: "hsla(80, 81%, 67%, 0.13)"
  dark-glow-mint: "hsla(162, 55%, 42%, 0.20)"
  dark-glow-lime: "hsla(80, 78%, 58%, 0.12)"
  dark-success: "#62c987"
  dark-warning: "#e0b36a"
  dark-danger: "#e2726a"
  dark-code: "#e0a882"
  dark-shadow: "hsla(0, 0%, 0%, 0.42)"
  light-canvas: "#f6f7f5"
  light-sidebar: "#f3f5f2"
  light-surface: "#fbfcfa"
  light-raised: "#ffffff"
  light-composer: "#ffffff"
  light-inset: "#f1f3ef"
  light-ticket: "#ffffff"
  light-text: "#121712"
  light-text-secondary: "#465045"
  light-text-tertiary: "#5a6457"
  light-text-ghost: "#626c5f"
  light-border: "hsla(150, 8%, 12%, 0.08)"
  light-border-strong: "hsla(150, 8%, 12%, 0.14)"
  light-ticket-border: "hsla(150, 8%, 12%, 0.09)"
  light-overlay: "hsla(150, 8%, 12%, 0.05)"
  light-overlay-strong: "hsla(150, 8%, 12%, 0.085)"
  light-accent: "#3d5a12"
  light-accent-soft: "hsla(80, 72%, 55%, 0.18)"
  light-glow-mint: "hsla(160, 62%, 80%, 0.85)"
  light-glow-lime: "hsla(80, 80%, 78%, 0.42)"
  light-success: "#237640"
  light-warning: "#875014"
  light-danger: "#ac3933"
  light-code: "#9a5528"
  light-shadow: "hsla(150, 25%, 10%, 0.08)"
  selection-blue: "#3b82f6"
  site-ink: "#121712"
  site-ink-2: "#3f463e"
  site-quiet: "#6b7268"
  site-rule: "#dde0da"
  site-paper: "#ffffff"
  site-lime-hover: "#b3e352"
  site-night: "#0f1211"
  site-night-2: "#181d1b"
  site-night-text: "#e8ece6"
  site-night-quiet: "#9aa39a"
typography:
  desktop-headline:
    fontFamily: "JetBrains Mono"
    fontSize: "28px"
    fontWeight: 400
    lineHeight: "38px"
  desktop-ticket-title:
    fontFamily: ".SystemUIFont"
    fontSize: "13.5px"
    fontWeight: 500
  desktop-body:
    fontFamily: ".SystemUIFont"
    fontSize: "13px"
  desktop-control:
    fontFamily: ".SystemUIFont"
    fontSize: "12.5px"
    fontWeight: 500
    lineHeight: "16px"
  desktop-field-comfortable:
    fontFamily: ".SystemUIFont"
    fontSize: "14px"
    lineHeight: "22px"
  desktop-meta:
    fontFamily: ".SystemUIFont"
    fontSize: "11.5px"
    fontWeight: 500
  desktop-ticket-number:
    fontFamily: "JetBrains Mono"
    fontSize: "11.5px"
  mobile-headline:
    fontFamily: "monospace"
    fontSize: "28px"
    fontWeight: 500
    lineHeight: "36px"
  mobile-title:
    fontFamily: "normal"
    fontSize: "26px"
    fontWeight: 600
  mobile-section:
    fontFamily: "normal"
    fontSize: "17px"
    fontWeight: 600
  mobile-ticket-title:
    fontFamily: "normal"
    fontSize: "15.5px"
    fontWeight: 500
    lineHeight: "21px"
  mobile-body:
    fontFamily: "normal"
    fontSize: "15px"
    lineHeight: "22px"
  mobile-control:
    fontFamily: "normal"
    fontSize: "13.5px"
    fontWeight: 500
    lineHeight: "18px"
  mobile-meta:
    fontFamily: "normal"
    fontSize: "12px"
    fontWeight: 600
    lineHeight: "16px"
  mobile-ticket-number:
    fontFamily: "monospace"
    fontSize: "12px"
    lineHeight: "16px"
  site-display:
    fontFamily: "Geist Variable"
    fontSize: "clamp(3rem, 9.2vw, 8.75rem)"
    fontWeight: 760
    lineHeight: 0.88
    letterSpacing: "-0.045em"
  site-hero-prompt:
    fontFamily: "JetBrains Mono"
    fontSize: "clamp(1.375rem, 3.4vw, 2.5rem)"
    fontWeight: 500
    lineHeight: 1.18
    letterSpacing: "-0.02em"
  site-cta:
    fontFamily: "Geist Variable"
    fontSize: "13px"
    fontWeight: 650
    letterSpacing: "0.06em"
rounded:
  tiny: "6px"
  small: "8px"
  medium: "12px"
  large: "16px"
  composer: "18px"
  pill: "999px"
  site-block: "2px"
spacing:
  half: "2px"
  one: "4px"
  two: "8px"
  ticket-gap: "12px"
  three: "16px"
  page-gutter: "20px"
  four: "24px"
  five: "32px"
  six: "64px"
components:
  desktop-ticket-dark:
    backgroundColor: "{colors.dark-ticket}"
    textColor: "{colors.dark-text}"
    typography: "{typography.desktop-ticket-title}"
    rounded: "{rounded.medium}"
    padding: "9px 12px 12px"
  desktop-ticket-needs-you-light:
    backgroundColor: "{colors.light-ticket}"
    textColor: "{colors.light-text}"
    typography: "{typography.desktop-ticket-title}"
    rounded: "{rounded.medium}"
    padding: "9px 12px 12px"
  desktop-pass-allow:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    typography: "{typography.desktop-control}"
    rounded: "{rounded.small}"
    height: "30px"
  desktop-menu-chip-filled-dark:
    backgroundColor: "{colors.dark-overlay}"
    textColor: "{colors.dark-text}"
    typography: "{typography.desktop-control}"
    rounded: "{rounded.pill}"
    padding: "0 10px"
    height: "28px"
  desktop-composer-dark:
    backgroundColor: "{colors.dark-composer}"
    textColor: "{colors.dark-text}"
    rounded: "{rounded.composer}"
    padding: "12px 0 10px"
  desktop-send:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.pill}"
    size: "32px"
  desktop-field-dark:
    backgroundColor: "{colors.dark-inset}"
    textColor: "{colors.dark-text}"
    typography: "{typography.desktop-control}"
    rounded: "{rounded.tiny}"
    padding: "0 8px"
    height: "28px"
  desktop-field-toolbar-dark:
    backgroundColor: "{colors.dark-inset}"
    textColor: "{colors.dark-text}"
    typography: "{typography.desktop-control}"
    rounded: "{rounded.large}"
    padding: "0 12px"
    height: "32px"
  desktop-field-comfortable-dark:
    backgroundColor: "{colors.dark-inset}"
    textColor: "{colors.dark-text}"
    typography: "{typography.desktop-field-comfortable}"
    rounded: "{rounded.small}"
    padding: "0 12px"
    height: "42px"
  desktop-toggle-on:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.pill}"
    width: "38px"
    height: "22px"
  desktop-segment-active-dark:
    backgroundColor: "{colors.dark-raised}"
    textColor: "{colors.dark-text}"
    typography: "{typography.desktop-control}"
    rounded: "{rounded.pill}"
    padding: "0 12px"
    height: "26px"
  desktop-menu-dark:
    backgroundColor: "{colors.dark-raised}"
    textColor: "{colors.dark-text}"
    typography: "{typography.desktop-control}"
    rounded: "{rounded.medium}"
    padding: "6px"
  mobile-ticket-light:
    backgroundColor: "{colors.light-ticket}"
    textColor: "{colors.light-text}"
    typography: "{typography.mobile-ticket-title}"
    rounded: "{rounded.medium}"
    padding: "10px 14px 14px"
  mobile-chip-dark:
    backgroundColor: "{colors.dark-overlay}"
    textColor: "{colors.dark-text}"
    typography: "{typography.mobile-control}"
    rounded: "{rounded.pill}"
    padding: "0 8px 0 10px"
    height: "36px"
  mobile-chip-active-light:
    backgroundColor: "{colors.light-accent-soft}"
    textColor: "{colors.light-accent}"
    typography: "{typography.mobile-control}"
    rounded: "{rounded.pill}"
    height: "36px"
  mobile-segment-active-light:
    backgroundColor: "{colors.light-ticket}"
    textColor: "{colors.light-text}"
    rounded: "{rounded.pill}"
    padding: "0 14px"
    height: "38px"
  site-block-link-lime:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    typography: "{typography.site-cta}"
    rounded: "{rounded.site-block}"
    padding: "0 14px 0 18px"
    height: "52px"
  site-ticket:
    backgroundColor: "{colors.site-paper}"
    textColor: "{colors.site-ink}"
    rounded: "14px"
---

# Design System: Ion

## Overview

**Creative North Star: "The Ticket Rail"**

Ion turns a request into a ticket and keeps every ticket on a live rail until it is settled. The home is a question set in mono ("What should we build in waku?"), one composer under it, and a grid of ticket cards rising out of a mint-to-lime glow field at the bottom of the window. Everything else (Decisions, Teams, Routines, Workflows, Artifacts, Apps, Harnesses) is a Paperclip-like workspace in a quiet sidebar: an org canvas for teams, pipelines for workflows, rows for routines, cards with real brand marks for apps.

Lime `#c2ef68` is the signal, not the paint. It fills the one action that moves work forward (send, Allow, an enabled switch) and outlines the ticket that needs you. Broad surfaces stay neutral green-grey in both themes; light and dark are designed with equal care and share one token schema.

Three clients, one vocabulary:

- **Desktop** (Rust/GPUI): compact chrome, `sp()` scaled type, the full workspace. Tokens in `src/theme.rs`; shared parts in `src/ui/`.
- **Android** (Expo/React Native): the same palette mirrored in `apps/mobile/src/constants/theme.ts`, with 36-48dp touch targets and larger type.
- **Website** (TanStack Start + Tailwind, `website/src/styles.css`): the "forge" register. Light only, oversized uppercase Geist display, square 2px CTA blocks and dark "night" bands. It reuses the app's ink, canvas, lime, leaf and glow, and shows the product through a hero demo of the composer and rail.

**Key Characteristics:**

- One lime action per context; status keeps its own hue and always a label.
- Tickets: mono `#XXXX` number in a stub, a dashed perforation, then title, station and state pill.
- Mono for identity and the home question; system sans for everything operational.
- Hairline translucent borders for structure; layered soft shadows for elevation.
- Pills for controls (chips, segments, toggles, send); 12px corners for objects.

## Colors

A neutral green-grey ramp (hue ~150) carries every surface; lime and its legible ink carry the brand; status hues are separate.

### Primary

- **Ion Lime** (`primary`): filled send, Allow, enabled toggle, NeedsYou ticket border. Identical in both themes.
- **Lime Ink** (`on-primary`): glyphs and labels on lime.
- **Accent** (`dark-accent` = lime, `light-accent` = leaf `#3d5a12`): legible brand foreground for focus rings, active chips, lit legend keys, working status. Lime is never light-theme text.
- **Accent Soft** (`*-accent-soft`): NeedsYou stub and pill, active chip fill, lit `@` legend key.

### Neutral

- **Canvas / Sidebar / Surface / Raised / Composer / Inset** (`*-canvas` ... `*-inset`): the layer stack from page to well. Dark is near-black green (`#0f1211` canvas, `#0d100f` inset); light is warm paper (`#f6f7f5` canvas, white raised). On macOS the desktop sidebar is transparent over native vibrancy.
- **Ticket** (`*-ticket`, `*-ticket-border`): ticket stock. White in light; `#181d1b` in dark, one step above the canvas.
- **Ink** (`*-text`, `-secondary`, `-tertiary`, `-ghost`): four steps of emphasis.
- **Border / Overlay** (`*-border*`, `*-overlay*`): translucent hairlines and hover/pressed fills (5% and 8.5-9%).

### Glow

- **Mint and Lime Glow** (`*-glow-mint`, `*-glow-lime`): two vertical gradients from clear to colour, rising from the bottom edge of home (mint over 62% of the height, lime over 30%). Strong in light (0.85 / 0.42 alpha), a faint cast in dark (0.20 / 0.12). Website `.glow-field` renders the same pair as two radial blooms over a canvas-to-`#eef6ea` gradient.

### Functional

- **Success / Warning / Danger**: Ready, caution, Failed and destructive actions; always with an icon and label. Danger has a 10% soft fill.
- **Selection Blue** (`selection-blue`): text selection, resize handle and usage gauges on desktop (`#2563eb` in light), Android native tint. Desktop text selection is `hsla(211, 100%, 50%, 0.55 / 0.35)`.
- **Code** (`*-code`): inline code foreground on a neutral 7-8% wash.

### Website

The site keeps the app's ink, canvas, lime and leaf but adds its own: `site-ink-2`, `site-quiet`, a solid `site-rule`, `site-lime-hover` and the `night` band set. `::selection` is lime on lime ink on the site only.

**The One Lime Action Rule.** Lime fills at most one control per context: the action that moves this ticket or this prompt forward. Everything beside it is outlined or quiet.

**The Label Carries Meaning Rule.** Every state colour ships with an icon and a word (Needs you, Working, Failed, Ready). Hue is never the only signal; toggles also move their knob.

**The Glow Is One Field Rule.** The mint-to-lime glow is painted once, as a large field behind home (and the site hero). Never as scattered tints on cards.

## Typography

**Display Font:** JetBrains Mono on desktop, platform `monospace` on Android (the home question and onboarding headings only)
**Body Font:** native system UI (`.SystemUIFont` on desktop, `normal` on Android)
**Website:** Geist Variable for display and body, JetBrains Mono for the hero prompt, numbers and labels

Desktop chrome sizes are authored at the 14px default and pass through `sp()`, so the UI font size setting scales them; content surfaces (markdown, editor, diff, terminal) follow their own settings. Android sizes are RN font units and respect system font scale.

### Hierarchy

- **Headline** (mono, 28px; desktop 400 / 38px line, Android 500 / 36px): the home question and onboarding. One per screen.
- **Ticket title** (desktop 13.5px / 500; Android 15.5px / 500 / 21px): two lines max.
- **Control** (desktop 12.5px / 500, the workhorse: 329 uses; Android 13.5px / 500): chips, segments, menu items, fields.
- **Body** (desktop 13px; Android 15px / 22px): rows, descriptions, rail empty state.
- **Meta** (desktop 11.5px / 500; Android 12px / 600): status pills, rail counters, legend labels.
- **Ticket number** (mono 11.5px desktop, 12px Android, tabular): `#` plus the first four hex digits of the id, stable across renders.
- **Website display** (Geist 760, uppercase, -0.045em, 0.88 line): `display-xl/lg/md` clamps from 2rem to 8.75rem, with one bracketed word in `--bracket` grey (lime in night bands).

**The Mono Means Identity Rule.** Mono marks things with an identity (ticket numbers, the home question, `@` sigils, code). It is not used for ordinary labels.

## Layout

- Home: question at 16% of the height, composer (max 720px) below with a 26px gap, then the rail 44px below. The rail is max 1040px; tickets are at least 220px wide, 12px apart, and the column count is whole tickets only, never a card cut at the edge. Order: NeedsYou, Working, then settled by recency.
- Desktop page gutter 20px; settings content max 760px; Android content max 800px (home 760px) with a 16px gutter.
- Spacing: Android uses the `Spacing` scale (2/4/8/16/24/32/64). Desktop uses literal px on a 4px rhythm with frequent 6/10/12/20 steps inside controls.
- Teams org canvas: 24px dot grid, 320x92 nodes on desktop, zoom 0.4-1.6 (Android 0.35-2), floating controls and legend.
- Sidebar: when more rows exist below, the last row fades over 32px into the footer instead of being sliced.
- Android switches to a navigation rail at 600dp window width.

## Elevation & Depth

Borders carry structure; shadows carry elevation. Every elevation is two layers offset downward: a tight contact shadow and a soft ambient one with -4px spread, tinted with `*-shadow` (green-black in light, black in dark).

| Level | Contact (y / blur / strength) | Ambient (y / blur / strength) | Used by |
|---|---|---|---|
| Rest | 1 / 2 / 0.9 | 6 / 18 / 0.7 | tickets, toggle knob, active segment, canvas pills |
| Lifted | 2 / 4 / 1.0 | 10 / 28 / 1.0 | composer, question and permission cards, org nodes |
| Float | 4 / 10 / 1.0 | 18 / 44 / 1.3 | menus, popovers, Android composer and request panel |

Android encodes Rest and Float as `elevationRest` / `elevationFloat` box-shadow strings; it has no Lifted level. The website uses its own ring-plus-drop shadows (`--shadow-ticket`, `--shadow-float`, `--shadow-night`).

## Shapes

- **Pill** (999): chips, segments and their track, toggles, send, status pills, icon discs.
- **12px** (`medium`): tickets, menus, cards, permission card. Ticket stub top corners are 11px inside the 1px border.
- **8px** (`small`): ticket pass buttons, icon buttons (28px), comfortable fields.
- **6-7px** (`tiny`): default fields, menu items, inline composer controls.
- **16px / 18px**: toolbar search field and org canvas frame / the desktop composer.
- **Website**: 2px CTA blocks, 14px tickets, 20-22px hero composer and night panels.

## Components

### Ticket card

Stub, perforation, body. The stub holds the mono number and an age or running clock; a 1px dashed perforation (3px dash, 3px gap on desktop) separates it from the title, a station line (provider or app mark plus project) and a state pill. NeedsYou: lime 1px border, accent-soft stub, lime-ink number on Android, and an inline pass of two 30px buttons (lime Allow, outlined Deny) answering the pending permission from the rail. Hover lifts Rest to Lifted. Android adds a selected state with an accent border. The website ticket marks NeedsYou with a 4px inset lime bar and a filled lime pill instead.

### Chips

Desktop `MenuChip`: 28px pill (30px outlined), 10px padding, 12.5px medium. `.filled()` sits on `overlay` with a hairline border; selected moves to `overlay_strong` and `border_strong`. Composer model, effort and access chips are filled. Android `Chip`: 36px pill in a 48px target, hairline border, active = accent-soft fill and accent text, quiet = transparent.

### Composer

Desktop: 18px card on `composer`, hairline border, Lifted, max 720px; chips row and a 32px round send that is lime when sendable and `overlay_strong` otherwise. Files dragged in light the card with an accent ring. Android: white/charcoal card with Float, 36px icon discs in 48px targets (active = accent-soft).

### Mention picker

A fixed 18px icon column so files, teams and apps line up. A 32px footer legend names the three sigils `@` (files, teams and apps), `@@` (teams), `@@@` (apps); the active key is lit with accent border, accent-soft fill and accent mono text.

### Fields, toggles, segments, menus

- Text field: inset fill, `border_strong` border turning accent on focus; default 28px / 6px radius, `.toolbar()` 32px pill-ish 16px radius, `.comfortable()` 42px / 8px / 14px type.
- Toggle: 38x22 pill, 16px knob with Rest shadow; on = lime fill and on-primary knob at the end.
- Segmented track: inset pill with 3px padding; the active segment rises as a raised pill with Rest shadow. Android uses ticket fill, 38px segments in 48px targets.
- Dropdown menu: raised, 12px, 6px padding, 176-320px wide, Float; items 30px, 6px radius.
- Icon button: 28px, 8px radius, overlay on hover, 13px tertiary glyph.

### Workspace

- **Teams org canvas**: dotted grid, raised nodes with Lifted shadow and 14px corners scaled by zoom, connector lines in `border_strong`, a Ready / Harness missing legend.
- **Apps**: cards with the app's identity mark from bundled marks first, then a CompanyEnrich logo fetched once and cached on disk, then a generic glyph. Never fetched from a URL during a frame.
- **Workflows, routines**: pipelines and rows on the same raised/border vocabulary.

### Website

`.block-link` is the CTA: 52px, 2px corners, uppercase 13px / 650 / 0.06em, tones lime, grey, ink, night; press scales to 0.96. `.chip` is a 34px white pill with a ring shadow. The hero demo replays the composer and a Live work rail of tickets.

## Do's and Don'ts

### Do:

- **Do** give each context at most one lime fill, and make it the action that moves work forward.
- **Do** put new live work on the rail as a ticket with a stable mono number and a labelled state.
- **Do** design every surface in light and dark from the same tokens; check both.
- **Do** use `elevation()` levels (or Android's `elevation*` strings) instead of ad hoc shadows.
- **Do** keep desktop chrome type in `sp()` and Android targets at 48dp.
- **Do** use bundled or cached real brand marks for apps and providers.

### Don't:

- **Don't** use lime as light-theme text or as a large surface fill; use leaf `#3d5a12` for text.
- **Don't** scatter the glow as tints on cards or sections; it is one field.
- **Don't** set ordinary labels in mono or in uppercase tracking inside the apps.
- **Don't** add uppercase eyebrow labels above headings on any surface.
- **Don't** let a ticket be cut at the rail edge or reorder settled tickets by state.
- **Don't** carry the website's 2px blocks and uppercase display into the native apps; the forge register is the site's alone.
