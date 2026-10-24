use gpui::{App, Global, Hsla, Rems, Window, WindowAppearance, hsla, rems, rgb, transparent_black};

pub use waku_client::theme::ThemePreference;

/// Scaled pixels: a dimension authored at the default 14px UI font size,
/// expressed in rems so the UI font size setting scales it. The window's rem
/// size *is* the UI font size, so at the default setting this resolves to
/// exactly the authored pixel value.
///
/// Chrome text sizes and their line heights go through here. Content surfaces
/// that already derive from a font-size setting — markdown metrics, the file
/// editor, diff rows, tool-output mono — stay in `px` so they never scale
/// twice.
pub fn sp(value: f32) -> Rems {
    rems(value / waku_client::persistence::DEFAULT_UI_FONT_SIZE)
}

fn resolves_to_dark(preference: ThemePreference, system_appearance: WindowAppearance) -> bool {
    match preference {
        ThemePreference::System => matches!(
            system_appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
        ThemePreference::Light => false,
        ThemePreference::Dark => true,
    }
}

fn native_override(preference: ThemePreference) -> Option<bool> {
    match preference {
        ThemePreference::System => None,
        ThemePreference::Light => Some(false),
        ThemePreference::Dark => Some(true),
    }
}

/// Ion's neutral workbench palette. Lime marks intentional primary actions;
/// status colors keep their own meaning. On macOS the sidebar's semantic
/// tint is installed as a native layer above Sidebar vibrancy; keeping this
/// GPUI surface clear avoids incorrectly accumulating the alpha of nested Metal
/// backgrounds. Selected, hovered, and pressed rows remain a 6% neutral layer.
#[derive(Clone, Copy)]
pub struct Theme {
    pub is_dark: bool,
    pub canvas: Hsla,
    pub sidebar: Hsla,
    pub sidebar_drag_background: Hsla,
    pub sidebar_item_background: Hsla,
    pub surface: Hsla,
    pub raised: Hsla,
    pub composer: Hsla,
    pub inset: Hsla,
    /// Terminal screen surface: paper-white in light mode, near-black in dark.
    pub terminal: Hsla,
    pub overlay: Hsla,
    pub overlay_strong: Hsla,

    pub border: Hsla,
    pub border_strong: Hsla,
    pub sidebar_border: Hsla,

    pub text: Hsla,
    pub text_secondary: Hsla,
    pub text_tertiary: Hsla,
    pub text_ghost: Hsla,

    /// Legible brand foreground; use `primary` for filled actions.
    pub accent: Hsla,
    pub accent_soft: Hsla,
    pub primary: Hsla,
    pub on_primary: Hsla,
    pub resize_handle: Hsla,
    /// Meter fills in the usage panel. Quota-meter blue by convention;
    /// warning/danger take over as a lane fills.
    pub gauge: Hsla,

    /// Text-selection wash. Painted *under* the glyphs, so it stays
    /// translucent and deliberately reads as the familiar browser blue rather
    /// than as brand color.
    pub selection: Hsla,
    /// Inline `code` foreground and its rounded wash.
    pub code_text: Hsla,
    pub code_wash: Hsla,

    /// Light fill for primary buttons (send, allow), dark glyph on top.
    pub inverse: Hsla,
    pub on_inverse: Hsla,

    pub warning: Hsla,
    pub success: Hsla,
    pub favorite: Hsla,
    pub danger: Hsla,
    pub danger_soft: Hsla,

    /// The home glow field: one mint-to-lime wash rising behind the prompt.
    /// Painted only as a large field, never as scattered tints.
    pub glow_mint: Hsla,
    pub glow_lime: Hsla,
    /// Ticket stock for live work: the card a request becomes on the rail.
    pub ticket: Hsla,
    pub ticket_border: Hsla,
    /// Tint for cast elevation. Light themes use a soft green-black; dark
    /// themes use deeper black so floating layers separate from the canvas.
    pub shadow: Hsla,
}

impl Theme {
    /// Workspace pages share the same layers as chat, sidebar, and settings.
    pub fn workspace(cx: &App) -> Self {
        Self::current(cx)
    }

    pub fn current(cx: &App) -> Self {
        if cx.has_global::<ActiveWakuTheme>() {
            cx.global::<ActiveWakuTheme>().0
        } else {
            Self::dark()
        }
    }

    pub fn dark() -> Self {
        Self {
            is_dark: true,
            canvas: rgb(0x0F1211).into(),
            sidebar: if cfg!(target_os = "macos") {
                transparent_black()
            } else {
                rgb(0x0C0F0E).into()
            },
            sidebar_drag_background: rgb(0x0C0F0E).into(),
            sidebar_item_background: hsla(0.0, 0.0, 0.941, 0.05),
            surface: rgb(0x121615).into(),
            raised: rgb(0x1A1F1D).into(),
            composer: rgb(0x171C1A).into(),
            inset: rgb(0x0D100F).into(),
            terminal: rgb(0x0D100F).into(),
            overlay: hsla(150.0 / 360.0, 0.10, 0.90, 0.05),
            overlay_strong: hsla(150.0 / 360.0, 0.10, 0.90, 0.09),

            border: hsla(150.0 / 360.0, 0.10, 0.90, 0.075),
            border_strong: hsla(150.0 / 360.0, 0.10, 0.90, 0.14),
            sidebar_border: rgb(0x1C2120).into(),

            text: rgb(0xEEF2EC).into(),
            text_secondary: rgb(0xBAC2B7).into(),
            text_tertiary: rgb(0x9DA69A).into(),
            text_ghost: rgb(0x8A9487).into(),

            accent: rgb(0xC2EF68).into(),
            accent_soft: hsla(80.0 / 360.0, 0.81, 0.67, 0.13),
            primary: rgb(0xC2EF68).into(),
            on_primary: rgb(0x1B2510).into(),
            resize_handle: rgb(0x3B82F6).into(),
            gauge: rgb(0x3B82F6).into(),

            selection: hsla(211.0 / 360.0, 1.0, 0.50, 0.55),
            code_text: rgb(0xE0A882).into(),
            code_wash: hsla(220.0 / 360.0, 0.10, 0.90, 0.08),

            inverse: rgb(0xE7E9EC).into(),
            on_inverse: rgb(0x17181C).into(),

            warning: rgb(0xE0B36A).into(),
            success: rgb(0x62C987).into(),
            favorite: rgb(0xEAB308).into(),
            danger: rgb(0xE2726A).into(),
            danger_soft: hsla(4.0 / 360.0, 0.55, 0.63, 0.10),

            glow_mint: hsla(162.0 / 360.0, 0.55, 0.42, 0.20),
            glow_lime: hsla(80.0 / 360.0, 0.78, 0.58, 0.12),
            ticket: rgb(0x181D1B).into(),
            ticket_border: hsla(150.0 / 360.0, 0.10, 0.90, 0.09),
            shadow: hsla(0.0, 0.0, 0.0, 0.42),
        }
    }

    pub fn light() -> Self {
        Self {
            is_dark: false,
            canvas: rgb(0xF6F7F5).into(),
            sidebar: if cfg!(target_os = "macos") {
                transparent_black()
            } else {
                rgb(0xF3F5F2).into()
            },
            sidebar_drag_background: rgb(0xEDF0EB).into(),
            sidebar_item_background: hsla(0.0, 0.0, 0.078, 0.05),
            surface: rgb(0xFBFCFA).into(),
            raised: rgb(0xFFFFFF).into(),
            composer: rgb(0xFFFFFF).into(),
            inset: rgb(0xF1F3EF).into(),
            terminal: rgb(0xFFFFFF).into(),
            overlay: hsla(150.0 / 360.0, 0.08, 0.12, 0.05),
            overlay_strong: hsla(150.0 / 360.0, 0.08, 0.12, 0.085),

            border: hsla(150.0 / 360.0, 0.08, 0.12, 0.08),
            border_strong: hsla(150.0 / 360.0, 0.08, 0.12, 0.14),
            sidebar_border: hsla(150.0 / 360.0, 0.08, 0.12, 0.08),

            text: rgb(0x121712).into(),
            text_secondary: rgb(0x465045).into(),
            text_tertiary: rgb(0x5A6457).into(),
            text_ghost: rgb(0x626C5F).into(),

            accent: rgb(0x3D5A12).into(),
            accent_soft: hsla(80.0 / 360.0, 0.72, 0.55, 0.18),
            primary: rgb(0xC2EF68).into(),
            on_primary: rgb(0x1B2510).into(),
            resize_handle: rgb(0x2563EB).into(),
            gauge: rgb(0x2563EB).into(),

            selection: hsla(211.0 / 360.0, 1.0, 0.50, 0.35),
            code_text: rgb(0x9A5528).into(),
            code_wash: hsla(220.0 / 360.0, 0.10, 0.12, 0.07),

            inverse: rgb(0x202227).into(),
            on_inverse: rgb(0xF8F8F9).into(),

            warning: rgb(0x875014).into(),
            success: rgb(0x237640).into(),
            favorite: rgb(0xCA8A04).into(),
            danger: rgb(0xAC3933).into(),
            danger_soft: hsla(4.0 / 360.0, 0.55, 0.52, 0.10),

            glow_mint: hsla(160.0 / 360.0, 0.62, 0.80, 0.85),
            glow_lime: hsla(80.0 / 360.0, 0.80, 0.78, 0.42),
            ticket: rgb(0xFFFFFF).into(),
            ticket_border: hsla(150.0 / 360.0, 0.08, 0.12, 0.09),
            shadow: hsla(150.0 / 360.0, 0.25, 0.10, 0.08),
        }
    }
}

#[derive(Clone, Copy)]
struct ActiveWakuTheme(Theme);

impl Global for ActiveWakuTheme {}

/// Publish the resolved palette. [`Theme::current`] reads it back from the
/// global, which is how every view gets its colors.
fn set_active_theme(theme: Theme, cx: &mut App) {
    cx.set_global(ActiveWakuTheme(theme));
}

/// Resolve and publish the startup palette, before any window exists.
pub fn init(cx: &mut App) {
    let system_appearance = cx.window_appearance();
    let theme = if resolves_to_dark(ThemePreference::System, system_appearance) {
        Theme::dark()
    } else {
        Theme::light()
    };
    set_active_theme(theme, cx);
}

pub fn apply_theme_preference(preference: ThemePreference, window: &mut Window, cx: &mut App) {
    crate::platform::set_window_appearance(window, native_override(preference));
    let is_dark = resolves_to_dark(preference, cx.window_appearance());
    set_active_theme(
        if is_dark {
            Theme::dark()
        } else {
            Theme::light()
        },
        cx,
    );
    crate::platform::configure_sidebar_material(window, is_dark);
    window.refresh();
}
