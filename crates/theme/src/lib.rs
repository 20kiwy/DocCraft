//! DocCraft shared design tokens.
//!
//! Ported 1:1 from the upstream FigCraft `ui-egui` design system
//! (`clone/figcraft/crates/ui-egui/src/theme.rs`, UI3 light + dark), so the
//! native crafts (Draw / Markdown / Typeset / Notes) share the exact same
//! design language as the real storytold crafts. Every native craft:
//!
//! 1. calls [`apply`] with the theme it wants to show this frame,
//! 2. paints all of its chrome from [`Tokens::get`] — never from hardcoded hex.
//!
//! Default theme is dark (matches the DocCraft launcher shell); each craft has
//! a Sun/Moon toggle for the light theme, like FigCraft's tab-bar toggle.

use egui::{Color32, Context, FontFamily, FontId, Stroke, TextStyle, Visuals};

pub mod icons;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub canvas: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub text_tertiary: Color32,
    pub icon: Color32,
    pub icon_secondary: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub field: Color32,
    pub field_hover_border: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_text: Color32,
    pub selected_row: Color32,
    pub selected_row_child: Color32,
    pub component: Color32,
    pub component_row: Color32,
    pub danger: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub guide: Color32,
    pub tab_bar: Color32,
    pub tab_active: Color32,
    pub shadow: Color32,
    pub menu: Color32,
    pub menu_text: Color32,
    pub menu_hover: Color32,
    pub menu_dim: Color32,
    /// Focus ring color: at least 3:1 against panels and canvas.
    pub focus: Color32,
}

pub const ACCENT: Color32 = Color32::from_rgb(0x0D, 0x99, 0xFF);
pub const COMPONENT: Color32 = Color32::from_rgb(0x97, 0x47, 0xFF);

impl Tokens {
    pub fn light() -> Self {
        Self {
            dark: false,
            bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            panel: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            canvas: Color32::from_rgb(0xF5, 0xF5, 0xF5),
            border: Color32::from_rgb(0xE6, 0xE6, 0xE6),
            border_strong: Color32::from_rgb(0xD9, 0xD9, 0xD9),
            text: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            text_secondary: Color32::from_rgb(0x6E, 0x6E, 0x6E),
            text_tertiary: Color32::from_rgb(0xB3, 0xB3, 0xB3),
            icon: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            icon_secondary: Color32::from_rgb(0x75, 0x75, 0x75),
            hover: Color32::from_rgb(0xF5, 0xF5, 0xF5),
            pressed: Color32::from_rgb(0xE6, 0xE6, 0xE6),
            field: Color32::from_rgb(0xF5, 0xF5, 0xF5),
            field_hover_border: Color32::from_rgb(0xE6, 0xE6, 0xE6),
            accent: ACCENT,
            accent_hover: Color32::from_rgb(0x00, 0x7B, 0xE5),
            accent_text: Color32::from_rgb(0x00, 0x6B, 0xD1),
            selected_row: Color32::from_rgb(0xE5, 0xF4, 0xFF),
            selected_row_child: Color32::from_rgb(0xF2, 0xF9, 0xFF),
            component: COMPONENT,
            component_row: Color32::from_rgb(0xF1, 0xE9, 0xFF),
            danger: Color32::from_rgb(0xF2, 0x48, 0x22),
            success: Color32::from_rgb(0x14, 0xAE, 0x5C),
            warning: Color32::from_rgb(0xFF, 0xCD, 0x29),
            guide: Color32::from_rgb(0xF2, 0x48, 0x22),
            tab_bar: Color32::from_rgb(0xE8, 0xE8, 0xE8),
            tab_active: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            shadow: Color32::from_black_alpha(40),
            menu: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            menu_text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            menu_hover: ACCENT,
            menu_dim: Color32::from_rgb(0x9A, 0x9A, 0x9A),
            focus: Color32::from_rgb(0x00, 0x6B, 0xD1),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            bg: Color32::from_rgb(0x2C, 0x2C, 0x2C),
            panel: Color32::from_rgb(0x2C, 0x2C, 0x2C),
            canvas: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            border: Color32::from_rgb(0x44, 0x44, 0x44),
            border_strong: Color32::from_rgb(0x55, 0x55, 0x55),
            text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            text_secondary: Color32::from_rgb(0xB3, 0xB3, 0xB3),
            text_tertiary: Color32::from_rgb(0x75, 0x75, 0x75),
            icon: Color32::from_rgb(0xF5, 0xF5, 0xF5),
            icon_secondary: Color32::from_rgb(0xB3, 0xB3, 0xB3),
            hover: Color32::from_rgb(0x38, 0x38, 0x38),
            pressed: Color32::from_rgb(0x44, 0x44, 0x44),
            field: Color32::from_rgb(0x38, 0x38, 0x38),
            field_hover_border: Color32::from_rgb(0x4D, 0x4D, 0x4D),
            accent: ACCENT,
            accent_hover: Color32::from_rgb(0x3D, 0xAD, 0xFF),
            accent_text: Color32::from_rgb(0x7C, 0xC4, 0xF8),
            selected_row: Color32::from_rgb(0x2A, 0x45, 0x5C),
            selected_row_child: Color32::from_rgb(0x2D, 0x37, 0x41),
            component: Color32::from_rgb(0xD2, 0x7C, 0xFF),
            component_row: Color32::from_rgb(0x3E, 0x30, 0x52),
            danger: Color32::from_rgb(0xF2, 0x48, 0x22),
            success: Color32::from_rgb(0x14, 0xAE, 0x5C),
            warning: Color32::from_rgb(0xFF, 0xCD, 0x29),
            guide: Color32::from_rgb(0xF2, 0x48, 0x22),
            tab_bar: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            tab_active: Color32::from_rgb(0x2C, 0x2C, 0x2C),
            shadow: Color32::from_black_alpha(90),
            menu: Color32::from_rgb(0x1E, 0x1E, 0x1E),
            menu_text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            menu_hover: ACCENT,
            menu_dim: Color32::from_rgb(0x9A, 0x9A, 0x9A),
            focus: ACCENT,
        }
    }

    /// The tokens the context currently shows (defaults to light when none
    /// applied yet — same contract as FigCraft).
    pub fn get(ctx: &Context) -> Self {
        ctx.data(|d| d.get_temp::<Tokens>(tokens_id())).unwrap_or_else(Tokens::light)
    }
}

fn tokens_id() -> egui::Id {
    egui::Id::new("doccraft_tokens")
}

/// egui's default for how long hover fades and openings animate.
const ANIMATION_TIME: f32 = 0.2;

/// Install tokens and egui visuals for this frame.
pub fn apply(ctx: &Context, t: Tokens) {
    ctx.data_mut(|d| d.insert_temp(tokens_id(), t));
    ctx.set_visuals(visuals(t));
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(8.0, 4.0);
        s.spacing.interact_size = egui::vec2(24.0, 24.0);
        s.spacing.menu_margin = egui::Margin::same(8);
        s.spacing.window_margin = egui::Margin::same(12);
        s.text_styles = [
            (TextStyle::Small, FontId::new(10.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(11.0, FontFamily::Monospace)),
        ]
        .into();
        s.interaction.tooltip_delay = 0.4;
        s.animation_time = ANIMATION_TIME;
    });
}

/// egui visuals for tokens `t` — FigCraft's exact widget treatment
/// (radius 5 widgets, radius 13 windows/menus, accent selection, soft shadows).
pub fn visuals(t: Tokens) -> Visuals {
    let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = t.panel;
    v.window_fill = t.panel;
    v.extreme_bg_color = t.field;
    v.faint_bg_color = t.hover;
    v.override_text_color = Some(t.text);
    v.window_stroke = Stroke::new(1.0, t.border);
    v.window_corner_radius = egui::CornerRadius::same(13);
    v.menu_corner_radius = egui::CornerRadius::same(13);
    v.selection.bg_fill = t.accent.gamma_multiply(0.3);
    v.selection.stroke = Stroke::new(1.0, t.accent);
    v.hyperlink_color = t.accent_text;
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open, &mut v.widgets.noninteractive] {
        w.corner_radius = egui::CornerRadius::same(5);
        w.fg_stroke = Stroke::new(1.0, t.text);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
    v.widgets.noninteractive.bg_fill = t.panel;
    v.widgets.inactive.bg_fill = t.field;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.bg_fill = t.hover;
    v.widgets.hovered.weak_bg_fill = t.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.field_hover_border);
    v.widgets.active.bg_fill = t.pressed;
    v.widgets.active.weak_bg_fill = t.pressed;
    v.widgets.active.bg_stroke = Stroke::new(1.0, t.accent);
    v.popup_shadow = egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: t.shadow };
    v.window_shadow = egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: t.shadow };
    v.text_cursor.stroke = Stroke::new(1.0, t.accent);
    v
}

/// Run `f` with the dark tokens and visuals (dark popovers on either theme).
pub fn on_dark<R>(ui: &mut egui::Ui, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let ctx = ui.ctx().clone();
    let saved = Tokens::get(&ctx);
    let dark = Tokens::dark();
    ctx.data_mut(|d| d.insert_temp(tokens_id(), dark));
    let r = ui
        .scope(|ui| {
            *ui.visuals_mut() = visuals(dark);
            f(ui)
        })
        .inner;
    ctx.data_mut(|d| d.insert_temp(tokens_id(), saved));
    r
}

/// Semibold-style font id at `size` (painted labels, captions, page text).
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Regular font id at `size`.
pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// 1px hairline stroke in the theme's border color.
pub fn hairline(t: &Tokens) -> Stroke {
    Stroke::new(1.0, t.border)
}

/// Soft drop shadow used for page sheets, cards and floating pills
/// (FigCraft's frame shadow).
pub fn card_shadow(t: &Tokens) -> egui::Shadow {
    egui::Shadow { offset: [0, 2], blur: 8, spread: 0, color: t.shadow }
}
