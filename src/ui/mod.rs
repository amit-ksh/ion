use gpui::{
    AnyElement, App, Context, Div, ElementId, Hsla, Img, InteractiveElement, Interactivity,
    KeyDownEvent, ParentElement, PathBuilder, Pixels, RenderOnce, ScrollHandle, SharedString,
    Stateful, StyleRefinement, Styled, Svg, Window, canvas, div, img, point, prelude::*, px, rgb,
    svg,
};

pub mod menu;
pub mod motion;
pub mod scrollbar;
pub mod text_field;
pub mod tooltip;

gpui::actions!(focus_navigation, [FocusNext, FocusPrevious]);

pub fn init_focus_navigation(cx: &mut App) {
    cx.bind_keys([
        gpui::KeyBinding::new("tab", FocusNext, None),
        gpui::KeyBinding::new("shift-tab", FocusPrevious, None),
    ]);
}

/// Fallback traversal for every native surface. More specific picker/editor
/// bindings keep their conventional Tab behavior.
pub fn focus_navigation(content: AnyElement) -> Div {
    div()
        .size_full()
        .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
        .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
        .child(content)
}

use crate::model::{ActivityKind, ProviderKind, SessionStatus};
use crate::theme::{Theme, sp};

/// A monochrome icon from the embedded set, tinted via text color. Sized in
/// `sp` so icons keep pace with the chrome text they sit beside when the UI
/// font size setting moves.
pub fn icon(path: &'static str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(path)
        .w(sp(size))
        .h(sp(size))
        .flex_none()
        .text_color(color)
}

/// How far a surface floats above the canvas.
#[derive(Clone, Copy)]
pub enum Elevation {
    /// Resting content objects: tickets, the composer, cards on a page.
    Rest,
    /// A resting object under the pointer.
    Lifted,
    /// Transient layers: menus, popovers, toasts.
    Float,
}

/// Cast depth for a surface: a tight contact shadow under a soft ambient one,
/// both offset downward so the light reads as coming from above. Borders keep
/// structure; this carries elevation.
pub fn elevation(theme: &Theme, level: Elevation) -> Vec<gpui::BoxShadow> {
    let tint = theme.shadow;
    let shade = |strength: f32| Hsla {
        a: (tint.a * strength).min(1.0),
        ..tint
    };
    let (contact, ambient) = match level {
        Elevation::Rest => ((1.0, 2.0, 0.9), (6.0, 18.0, 0.7)),
        Elevation::Lifted => ((2.0, 4.0, 1.0), (10.0, 28.0, 1.0)),
        Elevation::Float => ((4.0, 10.0, 1.0), (18.0, 44.0, 1.3)),
    };
    vec![
        gpui::BoxShadow::new(px(0.0), px(contact.0), shade(contact.2)).blur_radius(px(contact.1)),
        gpui::BoxShadow::new(px(0.0), px(ambient.0), shade(ambient.2))
            .blur_radius(px(ambient.1))
            .spread_radius(px(-4.0)),
    ]
}

/// A polychrome file icon rendered as an image so the SVG's authored colors
/// are preserved. GPUI's `svg()` element intentionally renders an alpha mask
/// tinted with one text color.
pub fn file_icon(path: &'static str, size: f32) -> Img {
    img(path).w(sp(size)).h(sp(size)).flex_none()
}

/// Compact desktop toolbar target. Callers attach activation and a tooltip.
pub fn icon_button(id: impl Into<ElementId>, path: &'static str, theme: Theme) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(28.0))
        .rounded(px(8.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_default()
        .hover(|element| element.bg(theme.overlay))
        .active(|element| element.bg(theme.overlay_strong))
        .child(icon(path, 13.0, theme.text_tertiary))
}

/// Keeps a wheel gesture in a nested scrollable while it can consume the
/// delta, then lets it chain to the ancestor at either boundary. Call from an
/// `on_scroll_wheel` listener. GPUI's own scroll handler runs first during the
/// bubble phase, so an offset outside the clamped range means this event tried
/// to move past the top or bottom and must keep bubbling. A viewport whose
/// content fits also keeps chaining so short blocks do not dead-zone the page.
/// Stopping propagation skips wheel listeners pushed earlier on the same
/// element, so fold any sibling wheel logic into the listener that calls this.
pub fn contain_scroll(handle: &ScrollHandle, cx: &mut App) {
    if nested_scroll_consumed_delta(handle.offset().y, handle.max_offset().y) {
        cx.stop_propagation();
    }
}

fn nested_scroll_consumed_delta(offset: Pixels, max_offset: Pixels) -> bool {
    max_offset > px(0.5) && offset >= -max_offset && offset <= px(0.0)
}

/// Add conventional mouse and keyboard activation to a focusable element.
pub trait ActivationExt: Sized {
    fn on_activation<E>(
        self,
        cx: &mut Context<E>,
        activate: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self
    where
        E: 'static;
}

impl ActivationExt for Stateful<Div> {
    fn on_activation<E>(
        self,
        cx: &mut Context<E>,
        activate: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self
    where
        E: 'static,
    {
        let activate = std::rc::Rc::new(activate);
        let click_activate = activate.clone();
        let key_activate = activate;
        self.on_click(cx.listener(move |this, _, window, cx| {
            click_activate(this, window, cx);
            cx.stop_propagation();
        }))
        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
            // Bare Enter/Space only. A modified chord belongs to whatever
            // command owns it, so a focused control must not swallow it —
            // this is the guard the hand-rolled settings toggles carried
            // before they moved onto this helper.
            if !event.keystroke.modifiers.modified()
                && matches!(event.keystroke.key.as_str(), "enter" | "space")
            {
                key_activate(this, window, cx);
                cx.stop_propagation();
            }
        }))
    }
}

/// The shared pill switch used by settings and automation forms.
///
/// `activate` is ignored while `disabled` is true, but the control remains in
/// the tab order so a pending operation does not move focus unexpectedly.
pub fn toggle_switch<E>(
    id: impl Into<ElementId>,
    on: bool,
    disabled: bool,
    theme: Theme,
    cx: &mut Context<E>,
    activate: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
) -> Stateful<Div>
where
    E: 'static,
{
    // Lime is the selection color, so an enabled switch fills with it; the
    // knob position carries the state for anyone who cannot see the hue.
    let knob_color = if on {
        theme.on_primary
    } else if theme.is_dark {
        theme.text_tertiary
    } else {
        rgb(0xFFFFFF).into()
    };
    let base = div()
        .id(id)
        .tab_index(0)
        .focus_visible(|style| style.border_color(theme.accent))
        .w(px(38.0))
        .h(px(22.0))
        .p(px(2.0))
        .flex_none()
        .rounded_full()
        .cursor_default()
        .when(disabled, |element| element.opacity(0.55))
        .bg(if on { theme.primary } else { theme.inset })
        .border_1()
        .border_color(if on {
            theme.primary
        } else {
            theme.border_strong
        })
        .flex()
        .items_center()
        .when(on, |element| element.justify_end())
        .child(
            div()
                .size(px(16.0))
                .rounded_full()
                .bg(knob_color)
                .shadow(elevation(&theme, Elevation::Rest)),
        );

    if disabled {
        base
    } else {
        base.on_activation(cx, activate)
    }
}

/// A pill track holding mutually exclusive views. The active segment rises
/// out of the track as a small raised card; the rest stay quiet text.
pub fn segmented_track(theme: &Theme) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(2.0))
        .p(px(3.0))
        .rounded_full()
        .bg(theme.inset)
        .border_1()
        .border_color(theme.border)
}

/// One segment for `segmented_track`. Callers attach activation.
pub fn segment(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    theme: &Theme,
) -> Stateful<Div> {
    let theme = *theme;
    let label: SharedString = label.into();
    div()
        .id(id)
        .tab_index(0)
        .h(px(26.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .rounded_full()
        .cursor_default()
        .text_size(sp(12.5))
        .font_weight(gpui::FontWeight::MEDIUM)
        .border_1()
        .border_color(gpui::transparent_black())
        .focus_visible(|style| style.border_color(theme.accent))
        .map(|segment| {
            if active {
                segment
                    .bg(theme.raised)
                    .border_color(theme.border)
                    .text_color(theme.text)
                    .shadow(elevation(&theme, Elevation::Rest))
            } else {
                segment
                    .text_color(theme.text_secondary)
                    .hover(|style| style.text_color(theme.text).bg(theme.overlay))
            }
        })
        .when(!label.is_empty(), |segment| segment.child(label))
}

/// A small status pill: icon plus label, tinted by meaning. The label always
/// carries the meaning so the hue is never the only signal.
pub fn status_pill(
    icon_path: &'static str,
    label: impl Into<SharedString>,
    fg: Hsla,
    bg: Hsla,
) -> Div {
    div()
        .flex_none()
        .h(px(22.0))
        .px(px(8.0))
        .rounded_full()
        .bg(bg)
        .flex()
        .items_center()
        .gap(px(5.0))
        .text_size(sp(11.5))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(fg)
        .child(icon(icon_path, 11.0, fg))
        .child(label.into())
}

/// A square avatar tile that frames a mark or icon on list cards.
pub fn avatar_tile(size: f32, bg: Hsla, border: Hsla) -> Div {
    div()
        .size(px(size))
        .flex_none()
        .rounded(px((size * 0.3).round()))
        .bg(bg)
        .border_1()
        .border_color(border)
        .flex()
        .items_center()
        .justify_center()
}

/// Brand hue for each provider's official mark.
pub fn provider_color(theme: &Theme, provider: ProviderKind) -> Hsla {
    match provider {
        ProviderKind::Antigravity => rgb(0x7EABF4).into(),
        ProviderKind::Amp => rgb(0xF34E3F).into(),
        ProviderKind::Claude => rgb(0xD97757).into(),
        ProviderKind::DeepSeek => rgb(0x4D6BFE).into(),
        ProviderKind::Codex
        | ProviderKind::Cursor
        | ProviderKind::Nebius
        | ProviderKind::Fx
        | ProviderKind::OpenCode
        | ProviderKind::OpenCode2
        | ProviderKind::Grok
        | ProviderKind::Kimi
        | ProviderKind::OhMyPi
        | ProviderKind::Pi => {
            if theme.is_dark {
                rgb(0xF3F3F3).into()
            } else {
                rgb(0x34363B).into()
            }
        }
    }
}

/// Recognizable provider marks, matching the model picker vocabulary.
pub fn provider_icon(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::Antigravity => "icons/sparkle.svg",
        ProviderKind::Amp => "icons/provider-amp.svg",
        ProviderKind::Claude => "icons/provider-claude.svg",
        ProviderKind::Codex => "icons/provider-openai.svg",
        ProviderKind::Cursor => "icons/provider-cursor.svg",
        ProviderKind::DeepSeek => "icons/provider-deepseek.svg",
        ProviderKind::Fx => "icons/provider-fx.svg",
        ProviderKind::OpenCode => "icons/provider-opencode.svg",
        ProviderKind::OpenCode2 => "icons/provider-opencode2.svg",
        ProviderKind::Grok => "icons/provider-grok.svg",
        ProviderKind::Kimi => "icons/provider-kimi.svg",
        ProviderKind::Nebius => "icons/sparkle.svg",
        ProviderKind::OhMyPi => "icons/provider-ohmypi.svg",
        ProviderKind::Pi => "icons/provider-pi.svg",
    }
}

/// The separately coloured layer some marks carry — OpenCode 2's red "2".
///
/// `svg()` renders an alpha mask tinted by ONE color, so stacking a second
/// element is the only way to give part of a mark its own color.
pub fn provider_badge(provider: ProviderKind) -> Option<&'static str> {
    match provider {
        ProviderKind::OpenCode2 => Some("icons/provider-opencode2-badge.svg"),
        _ => None,
    }
}

/// A provider mark, including any separately coloured badge layer.
///
/// Prefer this over `icon(provider_icon(..), ..)`: a bare `icon` call silently
/// drops the badge, which is the only thing distinguishing the two OpenCode
/// marks at a glance.
pub fn provider_mark(theme: &Theme, provider: ProviderKind, size: f32, color: Hsla) -> Div {
    let base = div()
        .relative()
        .w(sp(size))
        .h(sp(size))
        .flex_none()
        .child(icon(provider_icon(provider), size, color));
    match provider_badge(provider) {
        // The badge inherits the base's alpha so a dimmed row dims both layers.
        Some(badge) => base.child(div().absolute().top_0().left_0().child(icon(
            badge,
            size,
            theme.danger.opacity(color.a),
        ))),
        None => base,
    }
}

pub fn status_color(theme: &Theme, status: SessionStatus) -> Hsla {
    match status {
        SessionStatus::Idle => theme.text_ghost,
        SessionStatus::Connecting | SessionStatus::Working => theme.accent,
        SessionStatus::Background => theme.text_secondary,
        SessionStatus::Waiting => theme.warning,
        SessionStatus::Failed => theme.danger,
    }
}

pub fn activity_icon(kind: ActivityKind) -> &'static str {
    match kind {
        ActivityKind::Reasoning => "icons/sparkle.svg",
        ActivityKind::Command => "icons/terminal.svg",
        ActivityKind::FileChange => "icons/pencil.svg",
        ActivityKind::FileRead => "icons/file.svg",
        ActivityKind::FileSearch => "icons/search.svg",
        ActivityKind::FileList => "icons/folder.svg",
        ActivityKind::Search => "icons/search.svg",
        ActivityKind::Plan => "icons/list.svg",
        ActivityKind::Tool => "icons/wrench.svg",
    }
}

pub fn activity_noun(kind: ActivityKind) -> (String, String) {
    match kind {
        ActivityKind::Reasoning => (tr!("activity.thought"), tr!("activity.thoughts")),
        ActivityKind::Command => (tr!("activity.command"), tr!("activity.commands")),
        ActivityKind::FileChange => (tr!("activity.file_edit"), tr!("activity.file_edits")),
        ActivityKind::FileRead => (tr!("activity.file_read"), tr!("activity.file_reads")),
        ActivityKind::FileSearch => (tr!("activity.file_search"), tr!("activity.file_searches")),
        ActivityKind::FileList => (tr!("activity.file_list"), tr!("activity.file_lists")),
        ActivityKind::Search => (tr!("activity.search"), tr!("activity.searches")),
        ActivityKind::Plan => (tr!("activity.plan_step"), tr!("activity.plan_steps")),
        ActivityKind::Tool => (tr!("activity.tool_call"), tr!("activity.tool_calls")),
    }
}

/// A compact chip used as a dropdown-menu trigger. `selected` is driven by the
/// menu's open state and renders as a soft fill.
#[derive(IntoElement)]
pub struct MenuChip {
    base: Stateful<Div>,
    icon: Option<(&'static str, Hsla)>,
    /// A second, separately coloured icon layer — see [`provider_mark`].
    badge: Option<(&'static str, Hsla)>,
    label: SharedString,
    caret: bool,
    outlined: bool,
    filled: bool,
    selected: bool,
    disabled: bool,
    height: Option<Pixels>,
    background: Option<Hsla>,
}

impl MenuChip {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            base: div().id(id),
            icon: None,
            badge: None,
            label: SharedString::default(),
            caret: true,
            outlined: false,
            filled: false,
            selected: false,
            disabled: false,
            height: None,
            background: None,
        }
    }

    /// Override the chip's fixed height, for rows whose controls share a
    /// different one.
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }

    /// Fill behind an outlined chip. The default matches raised cards; a
    /// chip sitting directly on another surface passes that surface here so
    /// it doesn't read as a filled pill.
    pub fn background(mut self, background: Hsla) -> Self {
        self.background = Some(background);
        self
    }

    pub fn icon(mut self, path: &'static str, color: Hsla) -> Self {
        self.icon = Some((path, color));
        self
    }

    /// A provider mark, carrying its badge layer if it has one. Prefer this
    /// over `icon(provider_icon(..), ..)`, which silently drops the badge.
    pub fn provider(mut self, theme: &Theme, provider: ProviderKind, color: Hsla) -> Self {
        self.icon = Some((provider_icon(provider), color));
        self.badge = provider_badge(provider).map(|badge| (badge, theme.danger.opacity(color.a)));
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    pub fn outlined(mut self) -> Self {
        self.outlined = true;
        self
    }

    /// Soft fill with a hairline at rest: the composer toolbar's pill chips.
    pub fn filled(mut self) -> Self {
        self.filled = true;
        self
    }

    pub fn caret(mut self, caret: bool) -> Self {
        self.caret = caret;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Soft fill marking the chip as the open menu's trigger.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl Styled for MenuChip {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for MenuChip {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl ParentElement for MenuChip {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl RenderOnce for MenuChip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let badge = self.badge;
        // Forge-style pill: icon, label, chevron. Plain chips keep a
        // transparent hairline so the open state can draw an edge without
        // shifting the label.
        self.base
            .h(self
                .height
                .unwrap_or(if self.outlined { px(30.0) } else { px(28.0) }))
            .px(if self.outlined { px(12.0) } else { px(10.0) })
            .rounded_full()
            .flex()
            .items_center()
            .gap(px(6.0))
            .text_size(sp(12.5))
            .line_height(sp(16.0))
            .font_weight(gpui::FontWeight::MEDIUM)
            .cursor_default()
            .border_1()
            .border_color(gpui::transparent_black())
            .focus_visible(|style| style.border_color(theme.accent))
            .when(self.outlined, |element| {
                element
                    .border_color(theme.border_strong)
                    .bg(self.background.unwrap_or(theme.raised))
            })
            .when(self.filled, |element| {
                element.bg(theme.overlay).border_color(theme.border)
            })
            .when(self.selected, |element| {
                element
                    .bg(if self.filled {
                        theme.overlay_strong
                    } else {
                        theme.overlay
                    })
                    .border_color(theme.border_strong)
            })
            .when(!self.disabled, |element| {
                let hover = if self.filled {
                    theme.overlay_strong
                } else {
                    theme.overlay
                };
                element.hover(move |element| element.bg(hover))
            })
            .when(self.disabled, |element| element.opacity(0.6))
            .when_some(self.icon, |element, (path, color)| {
                let mark = icon(path, 13.0, color);
                match badge {
                    Some((badge, badge_color)) => element.child(
                        div()
                            .relative()
                            .w(sp(13.0))
                            .h(sp(13.0))
                            .flex_none()
                            .child(mark)
                            .child(div().absolute().top_0().left_0().child(icon(
                                badge,
                                13.0,
                                badge_color,
                            ))),
                    ),
                    None => element.child(mark),
                }
            })
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(theme.text_secondary)
                    .child(self.label),
            )
            .when(self.caret, |element| {
                element.child(icon("icons/chevron-down.svg", 10.0, theme.text_tertiary))
            })
    }
}

/// An inline, link-like dropdown trigger used for the project name in the
/// empty-state headline.
#[derive(IntoElement)]
pub struct ProjectNameSelector {
    base: Stateful<Div>,
    label: SharedString,
    selected: bool,
}

impl ProjectNameSelector {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            base: div().id(id),
            label: label.into(),
            selected: false,
        }
    }

    /// Emphasised underline while its menu is open.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl Styled for ProjectNameSelector {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for ProjectNameSelector {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl ParentElement for ProjectNameSelector {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl RenderOnce for ProjectNameSelector {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let underline_color = if self.selected {
            theme.text_secondary
        } else {
            theme.text_tertiary
        };

        self.base
            .relative()
            .flex_none()
            .cursor_default()
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .child(self.label)
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let y = bounds.origin.y + bounds.size.height - px(0.5);
                        let mut builder =
                            PathBuilder::stroke(px(1.0)).dash_array(&[px(1.0), px(2.0)]);
                        builder.move_to(point(bounds.origin.x, y));
                        builder.line_to(point(bounds.origin.x + bounds.size.width, y));
                        if let Ok(line) = builder.build() {
                            window.paint_path(line, underline_color);
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_scroll_chains_only_after_reaching_a_boundary() {
        let max_offset = px(100.0);

        assert!(nested_scroll_consumed_delta(px(0.0), max_offset));
        assert!(nested_scroll_consumed_delta(px(-50.0), max_offset));
        assert!(nested_scroll_consumed_delta(px(-100.0), max_offset));
        assert!(!nested_scroll_consumed_delta(px(1.0), max_offset));
        assert!(!nested_scroll_consumed_delta(px(-101.0), max_offset));
        assert!(!nested_scroll_consumed_delta(px(0.0), px(0.0)));
    }

    /// A literal icon path anywhere in the source must resolve: an unembedded
    /// path renders an invisible but still clickable control.
    #[test]
    fn every_icon_path_in_source_is_embedded() {
        use crate::assets::Assets;
        use gpui::AssetSource;

        fn visit(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, out);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    out.push(path);
                }
            }
        }
        let mut files = Vec::new();
        visit(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let mut missing = Vec::new();
        for file in files {
            let source = std::fs::read_to_string(&file).unwrap();
            for (start, _) in source.match_indices("\"icons/") {
                let rest = &source[start + 1..];
                let Some(end) = rest.find('"') else { continue };
                let path = &rest[..end];
                let literal = path
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '.' | '_'));
                if literal && path.ends_with(".svg") && Assets.load(path).unwrap().is_none() {
                    missing.push(format!("{path} ({})", file.display()));
                }
            }
        }
        assert!(missing.is_empty(), "unembedded icons: {missing:#?}");
    }

    #[test]
    fn every_referenced_icon_is_embedded() {
        use crate::assets::Assets;
        use crate::model::{ActivityKind, ProviderKind};
        use gpui::AssetSource;

        let mut paths = vec![
            "icons/panel-left.svg",
            "icons/plus.svg",
            "icons/arrow-left.svg",
            "icons/arrow-right.svg",
            "icons/arrow-up.svg",
            "icons/stop.svg",
            "icons/check.svg",
            "icons/copy.svg",
            "icons/rewind.svg",
            "icons/fork.svg",
            "icons/git-branch.svg",
            "icons/chart-column.svg",
            "icons/chevron-down.svg",
            "icons/chevron-right.svg",
            "icons/chevron-up.svg",
            "icons/chevrons-up-down.svg",
            "icons/folder.svg",
            "icons/folder-new.svg",
            "icons/laptop.svg",
            "icons/file-diff.svg",
            "icons/globe.svg",
            "icons/hourglass.svg",
            "icons/alert.svg",
            "icons/lock.svg",
            "icons/lock-open.svg",
            "icons/star.svg",
            "icons/star-filled.svg",
            "icons/sparkle.svg",
            "icons/zap.svg",
            "icons/panel-right.svg",
            "icons/x.svg",
            "icons/bot.svg",
            "icons/rotate-cw.svg",
            "icons/package.svg",
            "icons/trash.svg",
        ];
        for provider in ProviderKind::ALL {
            paths.push(provider_icon(provider));
            paths.extend(provider_badge(provider));
        }
        for kind in [
            ActivityKind::Reasoning,
            ActivityKind::Command,
            ActivityKind::FileChange,
            ActivityKind::FileRead,
            ActivityKind::FileSearch,
            ActivityKind::FileList,
            ActivityKind::Search,
            ActivityKind::Plan,
            ActivityKind::Tool,
        ] {
            paths.push(activity_icon(kind));
        }
        for path in paths {
            assert!(
                Assets.load(path).unwrap().is_some(),
                "missing embedded icon: {path}"
            );
        }
    }
}
