---
name: Ion
description: A quiet native workbench for coordinating coding agents and reviewing real work.
colors:
  primary: "#c2ef68"
  on-primary: "#1b2510"
  dark-canvas: "#181b19"
  dark-sidebar: "#141715"
  dark-surface: "#1d201e"
  dark-raised: "#272b28"
  dark-composer: "#222623"
  dark-inset: "#131614"
  dark-text: "#f2f4ef"
  dark-text-secondary: "#c2c8c0"
  dark-text-tertiary: "#abb3a8"
  dark-text-ghost: "#9aa395"
  dark-border: "hsla(220, 10%, 90%, 0.07)"
  dark-border-strong: "hsla(220, 10%, 90%, 0.14)"
  dark-overlay: "hsla(220, 10%, 90%, 0.05)"
  dark-overlay-strong: "hsla(220, 10%, 90%, 0.09)"
  desktop-dark-accent-soft: "hsla(80, 81%, 67%, 0.12)"
  mobile-dark-accent-soft: "rgba(194, 239, 104, 0.12)"
  mobile-dark-selected: "#303a25"
  dark-success: "#62c987"
  dark-warning: "#e0b36a"
  dark-danger: "#e2726a"
  light-canvas: "#f5f6f2"
  light-sidebar: "#ecefe8"
  light-surface: "#fafbf8"
  light-raised: "#ffffff"
  light-inset: "#ebeee7"
  light-text: "#20271f"
  light-text-secondary: "#505b4d"
  light-text-tertiary: "#606b5c"
  light-text-ghost: "#626d5e"
  light-border: "hsla(220, 10%, 12%, 0.08)"
  light-border-strong: "hsla(220, 10%, 12%, 0.15)"
  light-overlay: "hsla(220, 10%, 12%, 0.05)"
  light-overlay-strong: "hsla(220, 10%, 12%, 0.09)"
  light-accent: "#47651b"
  desktop-light-accent-soft: "hsla(80, 55%, 34%, 0.10)"
  mobile-light-accent-soft: "rgba(100, 135, 39, 0.10)"
  mobile-light-selected: "#e4ebd8"
  light-success: "#237640"
  light-warning: "#875014"
  light-danger: "#ac3933"
  dark-code: "#e0a882"
  light-code: "#9a5528"
  selection-blue: "#3b82f6"
typography:
  desktop-heading:
    fontFamily: ".SystemUIFont"
    fontSize: "20px"
  desktop-section:
    fontFamily: ".SystemUIFont"
    fontSize: "17px"
    fontWeight: 600
  desktop-body:
    fontFamily: ".SystemUIFont"
    fontSize: "14px"
  desktop-label:
    fontFamily: ".SystemUIFont"
    fontSize: "13px"
  desktop-detail:
    fontFamily: ".SystemUIFont"
    fontSize: "12.5px"
  desktop-field:
    fontFamily: ".SystemUIFont"
    fontSize: "14px"
    lineHeight: "22px"
  mobile-title:
    fontFamily: "normal"
    fontSize: "26px"
    fontWeight: 600
  mobile-heading:
    fontFamily: "normal"
    fontSize: "20px"
    fontWeight: 600
    lineHeight: "28px"
  mobile-section:
    fontFamily: "normal"
    fontSize: "17px"
    fontWeight: 600
  mobile-field:
    fontFamily: "normal"
    fontSize: "16px"
    lineHeight: "24px"
  mobile-row:
    fontFamily: "normal"
    fontSize: "16px"
    fontWeight: 500
    lineHeight: "22px"
  mobile-button:
    fontFamily: "normal"
    fontSize: "14px"
    fontWeight: 600
    lineHeight: "20px"
  mobile-detail:
    fontFamily: "normal"
    fontSize: "13px"
    lineHeight: "19px"
  mobile-navigation:
    fontFamily: "normal"
    fontSize: "12px"
    lineHeight: "18px"
  mobile-code:
    fontFamily: "monospace"
    fontSize: "11.5px"
    lineHeight: "17px"
rounded:
  tiny: "6px"
  small: "8px"
  medium: "12px"
  large: "16px"
  pill: "999px"
spacing:
  half: "2px"
  one: "4px"
  two: "8px"
  three: "16px"
  four: "24px"
  five: "32px"
  six: "64px"
components:
  desktop-button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    typography: "{typography.desktop-label}"
    rounded: "{rounded.small}"
    padding: "0 12px"
    height: "32px"
  desktop-button-secondary-dark:
    backgroundColor: "{colors.dark-raised}"
    textColor: "{colors.dark-text}"
    typography: "{typography.desktop-label}"
    rounded: "{rounded.small}"
    padding: "0 12px"
    height: "32px"
  desktop-field-dark:
    backgroundColor: "{colors.dark-inset}"
    textColor: "{colors.dark-text}"
    typography: "{typography.desktop-field}"
    rounded: "{rounded.small}"
    padding: "0 12px"
    height: "42px"
  mobile-button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    typography: "{typography.mobile-button}"
    rounded: "{rounded.medium}"
    padding: "10px 14px"
  mobile-button-secondary-dark:
    backgroundColor: "{colors.dark-raised}"
    textColor: "{colors.dark-text}"
    typography: "{typography.mobile-button}"
    rounded: "{rounded.medium}"
    padding: "10px 14px"
  mobile-field-dark:
    backgroundColor: "{colors.dark-raised}"
    textColor: "{colors.dark-text}"
    typography: "{typography.mobile-field}"
    rounded: "{rounded.small}"
    padding: "12px"
  mobile-card-dark:
    backgroundColor: "{colors.dark-surface}"
    textColor: "{colors.dark-text}"
    rounded: "{rounded.large}"
    padding: "16px"
  mobile-navigation-item:
    typography: "{typography.mobile-navigation}"
    rounded: "{rounded.medium}"
  mobile-filter-selected-dark:
    backgroundColor: "{colors.mobile-dark-accent-soft}"
    textColor: "{colors.primary}"
    rounded: "{rounded.medium}"
    padding: "12px 16px"
  desktop-navigation-selected-dark:
    backgroundColor: "{colors.desktop-dark-accent-soft}"
    textColor: "{colors.dark-text}"
    typography: "{typography.desktop-label}"
    rounded: "{rounded.small}"
    padding: "0 11px"
    height: "36px"
  mobile-composer-dark:
    backgroundColor: "{colors.dark-composer}"
    textColor: "{colors.dark-text}"
    rounded: "{rounded.large}"
    padding: "6px 10px 8px"
  desktop-composer-dark:
    backgroundColor: "{colors.dark-composer}"
    textColor: "{colors.dark-text}"
    rounded: "{rounded.large}"
    padding: "10px 0"
    width: "720px"
---

# Design System: Ion

## Overview

**Creative North Star: "Operate"**

Ion is a native workbench: quiet charcoal or warm pale surfaces, compact contextual controls, and a restrained lime signal for the next useful action. Alignment and readable hierarchy do most of the work. Rounded groups hold an object, editor, or composer without turning every region into a card.

Desktop uses GPUI; Android uses React Native and Expo. Their shared palette and vocabulary establish one product while their control sizes and navigation remain native. The independent dotted-stem-and-loop Ion mark is canonical in `assets/icons/ion.svg`; `scripts/assets/ion-icon.py` produces platform assets with lime on charcoal.

This is a source-extracted record of the implemented redesign, not a visual acceptance record. The session was code led; there is no approved comp. Fresh native evidence now covers the desktop agent model menu, fixed equal-width Save/Close footer, scrolling and outside dismissal in dark mode, plus Appearance and home in light mode (`docs/ion-redesign/evidence/refinement/`). The original System theme preference was restored. Earlier Teams evidence proves its crash correction. Android emulator evidence now covers signed-in phone, 320dp at 130% font scale, expanded rail, model selection, save/reopen and keyboard handling. Broader desktop provider/state coverage, hardware checks and full visual acceptance remain incomplete. Formal visual disposition is **recapture**; the seven source findings resolved in `docs/ion-redesign/reviewer-verdict.md` do not establish full parity or visual acceptance. See `docs/ion-redesign/ux-audit.md` for the complete evidence boundary.

**Key Characteristics:**

- Neutral surfaces with selective lime emphasis.
- Compact desktop controls and larger Android touch targets.
- Real state, short contextual labels, and nearby actions.
- Flat content groups with depth reserved for transient layers.
- Visible focus, readable themes, and motion that respects system settings.

Token authority is `src/theme.rs` and `apps/mobile/src/constants/theme.ts`. Component evidence is in `src/ui/{mod,text_field}.rs`, `src/app/{workspace_control,composer,settings,sidebar}.rs`, and Android's `workbench.tsx`, `workbench-navigation.tsx`, `mobile-composer.tsx`, `automation-form-controls.tsx`, `_layout.tsx`, `work.tsx`, `agent-editor.tsx`, and `team-editor.tsx`. Frontmatter records extracted values; source remains the implementation authority. The companion sidecar renders illustrative HTML representations of native primitives; its CSS does not drive either app.

## Colors

The palette pairs a bright lime action fill with slightly warm neutral layers. Colors are grouped by their actual roles; semantic status hues remain distinct from brand emphasis.

### Primary

- **Ion Lime** (`primary`): filled create, open, send, and save actions; dark-theme brand foreground.
- **Ink on Lime** (`on-primary`): text and icons inside the primary fill in both themes.
- **Leaf Ink** (`light-accent`): light-theme accent text, focus, and selected icons. Bright lime remains the fill rather than the light-theme text color.
- **Soft Lime** (`desktop-*-accent-soft`, `mobile-*-accent-soft`): selected navigation and contextual controls. Desktop and mobile currently use slightly different canonical HSLA/RGBA expressions; preserve those source values rather than claiming exact token parity.

### Neutral

- **Workbench Canvas** (`*-canvas`): broad page background.
- **Sidebar Neutral** (`*-sidebar`): non-macOS sidebar and mobile light muted layer. On macOS, GPUI's sidebar color is transparent over a native Sidebar material; the opaque swatch is not the complete macOS compositing result.
- **Object Surface** (`*-surface`): mobile workbench cards and shared content surfaces.
- **Raised Neutral** (`*-raised`): desktop controls and forms; dark mobile muted controls share this layer. Light mobile muted controls use the light sidebar neutral rather than white.
- **Composer Surface** (`dark-composer`; light uses `light-raised`): a bounded input surface distinct from the dark canvas.
- **Inset Well** (`*-inset`): desktop fields, code, and terminal wells. The light desktop terminal instead uses white.
- **Primary / Secondary / Tertiary / Ghost Ink** (`*-text*`): descending emphasis for content, supporting copy, metadata, and subdued affordances.
- **Hairline / Strong Stroke** (`*-border*`): grouping and controls. Neutral overlays supply hover/pressed fills without introducing an accent wash everywhere.

### Functional colors

- **Success, Warning, Danger** (`*-success`, `*-warning`, `*-danger`): outcome, caution, and destructive/error state with text or an icon.
- **Native Blue** (`selection-blue`): Android text selection and desktop dark resize/usage accents. Desktop text selection is a separate translucent blue HSLA token; light desktop resize/usage uses a deeper blue. Do not replace these native/functional roles with lime.
- **Code Warmth** (`*-code`): inline code foreground paired with the source theme's neutral translucent code wash.

**The Intentional Lime Rule.** Use filled lime for the contextual primary action and selected brand affordances; keep broad work surfaces neutral.

**The Semantic Foreground Rule.** Resolve status and accent text from the current theme. Pair meaning with a label or icon, not color alone.

## Typography

Desktop chrome uses the native system face (`.SystemUIFont`). Code uses the bundled **JetBrains Mono**. Android uses the platform's default sans face (`normal`) and **monospace** for code. There is no separate marketing display face.

Frontmatter desktop pixel values are authored baseline sizes: `sp()` converts chrome type to rems relative to the default UI font setting (14px). Content/editor/code typography follows its own settings and must not be scaled twice. Android values represent React Native logical/font units; CSS pixels in the sidecar are previews, not physical device measurements.

### Hierarchy

- **Desktop heading:** compact page heading; workspace titles use the desktop-heading role. Weight is set at the call site.
- **Desktop section:** semibold form title; body, label, and detail roles keep controls compact.
- **Desktop field:** comfortable single-line field text with an explicit line height.
- **Mobile title:** agent/team editor heading.
- **Mobile heading:** empty, loading, and recovery state heading; section headings are smaller.
- **Mobile row / field:** readable object labels and form text; row details use the smaller detail role.
- **Mobile button / navigation:** semibold action copy and persistent destination labels. Selected Android destination labels change from medium to bold.
- **Mobile code:** compact monospace request details. The composer input independently uses 16-unit text with a 21-unit line height.

**The Native Type Rule.** Preserve platform text scaling and existing content font settings; do not replace chrome with a branded display font.

## Layout

Desktop workspace pages center their content within a maximum width (1280px). The main conversation/composer has a narrower maximum (720px). Workspace forms use the available width below a remaining-content threshold (900px), then a side form (420px, minimum 280px). Do not apply the transcript width to every workbench page.

Desktop layout uses compact row alignment, local padding, and gaps before nested containers. The mobile spacing scale is recorded in frontmatter; desktop also uses local gaps and padding such as 6, 10, 12, 14, 18, 20, and 28px. These values are observed component geometry, not a universal desktop spacing token scale.

Android primary navigation is a labelled bottom bar in compact widths and a left rail at window widths of at least 600dp. The rail is 96dp wide, with 72dp minimum destination items; bottom items have a 56dp minimum. Navigation respects safe-area insets, hides on detail routes, and hides the compact bar while the keyboard is visible. It requires a saved desktop profile. Chat, Work, Artifacts, and Settings remain the four primary destinations.

Mobile agent/team editors center content at a maximum width of 800dp with 16dp outer padding. Work allows a 1000dp maximum. Actions wrap; Work filters scroll horizontally. Card padding is 16dp with a 12dp internal gap. Android buttons and fields start at 48dp; object rows start at 64dp. The mobile composer uses 48dp circular controls, 4dp hit slop, and an 8dp cluster gap. Narrow and large-text recording states still require device validation.

Keep long desktop collections virtualized and mobile object collections in `FlatList`. Layout and decoration must preserve existing stream cadence, cached pane boundaries, and background work; see `docs/performance.md`.

## Elevation & Depth

Content rests on opaque tonal layers with subtle strokes. Workbench cards and Android controls do not add decorative shadows. Desktop floating menus and composer popovers use GPUI's `shadow_lg()`; it is a transient-layer convention, not a card default. Its two native shadow layers are recorded in the sidecar.

On macOS the sidebar uses native material. Android's `GlassSurface` resolves to an ordinary opaque `View`, including the composer fallback color. Existing iOS glass code is outside this Android redesign and is not evidence of Android transparency.

**The Flat Content Rule.** Establish everyday grouping with surface tone, spacing, and hairlines; reserve cast shadow for floating desktop layers.

Existing motion is functional. Desktop panels slide over 200ms with an ease-out-quint curve; repeating loaders use the shared self-parking pulse clock and stop under reduce motion. Spinners are capped at 60Hz; other pulses retain approximately 30Hz or slower cadences. Do not introduce decorative perpetual animation.

## Shapes

Use economical rounded rectangles: small controls and fields, medium form groups, and larger composer/card corners. The shared mobile radius vocabulary is in frontmatter. Desktop additionally retains local compact radii (4, 6, 7, 8, 10, 12, 13, and 16px); this is not a claim that every desktop control uses the mobile radius scale.

Desktop toolbar icon targets are 28px squares with 8px corners and 13-unit icons. Desktop send/stop controls are 32px circles. Android composer controls are 48dp circles. Preserve these platform-specific sizes.

GPUI's `overflow_hidden` clips to a rectangle. Descendant fills, images, and hover overlays that touch a rounded edge need their own matching radius, adjusted for border inset.

## Components

### Buttons

Compact and explicit. Desktop `control_button` uses a 32px height, 12px horizontal inset, 8px corners, and a strong 1px stroke. Primary create/open/save controls use lime, dark ink, and semibold text; hover and press reduce opacity. Secondary controls use the raised layer and neutral hover/pressed fills.

Android `WorkbenchButton` uses a minimum height of 48dp, medium corners, 14dp horizontal and 10dp vertical padding, and a 2dp focus border. Primary uses lime/dark ink; secondary uses muted surface; destructive secondary labels use danger. Pressed opacity is 0.75 and disabled opacity 0.45. The focused border uses the current accent.

Desktop focusable controls use visible accent treatment and conventional Enter/Space activation. The shared activation helper preserves modified key chords. New controls must retain keyboard operation, but this document does not certify every existing control's keyboard path.

### Chips / contextual filters

Mobile Work section filters are tab controls with a 48dp minimum height, 12dp corners, 16dp horizontal/12dp vertical padding, and a selected soft-accent fill. Selected text uses the theme accent; inactive text uses secondary ink. These are contextual navigation, not decorative badges.

### Cards / containers

Android `WorkbenchCard` has large corners, a surface fill, a hairline stroke, 16dp padding, and a 12dp gap. Desktop workspace/editor groups typically use 12px corners with raised fills and local 14–20px padding. Settings groups also use 12px corners and 18px padding. A card should contain a real object or coherent form group.

### Inputs / fields

Desktop `TextField` is inset-filled, 1px bordered, and accent-bordered when its input is visually focused. Comfortable form fields are 42px high with 8px corners, 12px horizontal padding, and the desktop-field typography role. Compact fields remain 28px high with 6px corners and smaller text. Labels sit above fields.

Android `AutomationField` uses muted surface, a 1dp border, 8dp corners, 12dp padding, and a 48dp minimum. Multiline fields start at 120dp. Focus changes the border to accent; labels and accessibility names remain explicit. Agent instructions are labelled required; save availability follows actual validity, connection freshness, and conflict state.

### Navigation

Desktop sidebar actions are compact labelled rows (32px high, 7px corners); New Task uses a soft accent and accent icon. Settings navigation uses 36px rows with 8px corners and a soft selected fill. Neutral hover layers remain separate from persistent selected state.

Android destination items pair icons and labels. The selected item has a soft-accent icon pill, accent icon, primary-ink bold label, and selected accessibility state. Rail/bar composition follows Layout; detail routes retain system Back. The Android hamburger has a transparent background and a 48dp target. Android navigation headers use an opaque theme background so scrolling fields stay below readable chrome.

### Composer

A distinct action surface. Desktop has 16px corners, a strong stroke, theme composer fill, and 10px vertical padding within the conversation width. Android uses the same corner role, an opaque composer fill and hairline stroke, with a multiline input above a wrapping toolbar. Controls expose accessible names; send displays busy/disabled state. Provider errors use a bounded three-line banner; HTML failures show a short recovery message while the runtime retains the diagnostic. The repeated mobile context-syntax caption is removed; context and attachment controls remain available.

Keep model, harness, and environment context explicit. Dictation has visible state and local actions. Preserve attachment, queue, approval, question, stop, and recovery behavior; do not substitute fictional progress or new animation.

### Definition editors and device management

Desktop model selection is a dropdown over cached provider models, including Harness default. Form fields scroll independently of a fixed bottom footer: primary Save and secondary Close divide its width equally, separated by an 8px gap. The footer uses 20px insets and a top hairline. Outside click and Escape dismiss the editor; an open dropdown handles its own dismissal first. Do not put these actions back inside the scrolling field column.

Android agent/team editors use the same action hierarchy with a fixed surface footer, 16dp insets, safe-area bottom padding and equal flexible columns. The keyboard-aware root keeps the footer above the Android IME, including edge-to-edge layouts. Models use the native picker rather than arbitrary model-ID entry; Harness default is a single distinct selection. Cloud definitions remain inspectable but must be managed on desktop; mobile never silently changes a saved cloud agent's environment.

Android home is a conversation surface: a model selector, open space for the current conversation, a compact project/desktop setup entry and the composer. Configuration is disclosed in a sheet; repeated syntax tutorials do not occupy the chat canvas. Paired desktop rows expose one options button; connection editing, reconnect, disconnect and removal live in its action sheet. Account identity remains in Settings. Shared service switches and cloud compute/billing/gateway destinations are absent from mobile.

### Empty, recovery, and conflict states

Android `WorkbenchState` pairs a 20-unit semibold heading with secondary detail and an optional real action. Work distinguishes disconnected, loading, empty, and failed refresh states. Editors keep a stale draft until an explicit Reload from desktop; conflicts must remain visible and must not be styled as successful saves.

### Identity

Use the canonical Ion vector mark: a separate dot, rounded stem, and rounded loop. It is authored as a monochrome mask and tinted by the host; platform tile generation supplies the lime/charcoal identity. Reproduce assets through the generator rather than redrawing the mark or substituting another company's logo.

## Do's and Don'ts

### Do:

- **Do** use neutral layers for broad surfaces and lime for the contextual primary action.
- **Do** resolve text, status, and focus colors from the current theme.
- **Do** preserve native system typography, platform scaling, and the bundled desktop code face.
- **Do** pair status with text or an icon and keep actions keyboard/touch operable.
- **Do** retain Android's labelled compact navigation and expanded rail.
- **Do** keep real connection, draft, busy, error, approval, and conflict states visible.
- **Do** use the canonical Ion vector and reproducible platform assets.
- **Do** recapture missing theme, viewport, and interaction evidence before claiming visual acceptance.

### Don't:

- **Don't** spread the primary lime across large canvas regions or use it as light-theme body text.
- **Don't** add fictional metrics, decorative charts, stacked glass, or perpetual decorative motion.
- **Don't** replace Android's opaque workbench surfaces with a simulated glass treatment.
- **Don't** force desktop control geometry onto Android touch controls.
- **Don't** treat parent rounding as clipping for GPUI descendant paint.
- **Don't** add I/O, whole-session scans, or a new frame clock to a rendering path.
- **Don't** claim full desktop/Android parity or hardware validation from source checks or one screenshot.
