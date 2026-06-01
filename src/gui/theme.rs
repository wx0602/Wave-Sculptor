use eframe::egui::{
    self, style::Widgets, Color32, Context, Rounding, Stroke, Style, Vec2, Visuals,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug)]
pub struct ThemePalette {
    pub app_bg: Color32,
    pub panel_bg: Color32,
    pub card_bg: Color32,
    pub card_alt_bg: Color32,
    pub card_border: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_active: Color32,
    pub accent_soft: Color32,
    pub waveform_mixed: Color32,
    pub waveform_left: Color32,
    pub waveform_right: Color32,
    pub playhead: Color32,
    pub selection_fill: Color32,
    pub axis_line: Color32,
    pub axis_text: Color32,
    pub status_bg: Color32,
    pub status_border: Color32,
    pub disabled_bg: Color32,
}

impl ThemeMode {
    pub const ALL: [Self; 2] = [Self::Light, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "浅色模式",
            Self::Dark => "深色模式",
        }
    }

    pub fn palette(self) -> ThemePalette {
        match self {
            Self::Dark => ThemePalette {
                app_bg: color(0x0B1220),
                panel_bg: color(0x0F182A),
                card_bg: color(0x111C2F),
                card_alt_bg: color(0x172A45),
                card_border: color(0x2A456A),
                text_primary: color(0xEAF3FF),
                text_secondary: color(0x93A8C2),
                accent: color(0x38BDF8),
                accent_hover: color(0x2563EB),
                accent_active: color(0x1D4ED8),
                accent_soft: rgba(0x38, 0xBD, 0xF8, 28),
                waveform_mixed: color(0x63CFF9),
                waveform_left: color(0x73C8F7),
                waveform_right: color(0x4FC3D9),
                playhead: color(0xF97316),
                selection_fill: rgba(0x38, 0xBD, 0xF8, 44),
                axis_line: color(0x36557D),
                axis_text: color(0x9BB3D3),
                status_bg: color(0x14233A),
                status_border: color(0x2A456A),
                disabled_bg: color(0x223753),
            },
            Self::Light => ThemePalette {
                app_bg: color(0xEAF3FA),
                panel_bg: color(0xEDF6FB),
                card_bg: color(0xF8FCFF),
                card_alt_bg: color(0xE8F7FA),
                card_border: color(0xB8D7EE),
                text_primary: color(0x17324D),
                text_secondary: color(0x60758A),
                accent: color(0x2563EB),
                accent_hover: color(0x3B82F6),
                accent_active: color(0x1D4ED8),
                accent_soft: rgba(0x37, 0x99, 0xEB, 24),
                waveform_mixed: color(0x215D92),
                waveform_left: color(0x2A6FA8),
                waveform_right: color(0x238A9F),
                playhead: color(0xDC2626),
                selection_fill: rgba(0x37, 0x99, 0xEB, 42),
                axis_line: color(0xB8D7EE),
                axis_text: color(0x6B829A),
                status_bg: color(0xEAF6FC),
                status_border: color(0xB8D7EE),
                disabled_bg: color(0xDCEAF5),
            },
        }
    }
}

pub fn apply_theme(ctx: &Context, theme: ThemeMode) {
    let palette = theme.palette();
    let mut style: Style = (*ctx.style()).clone();
    style.spacing.button_padding = Vec2::new(11.0, 7.0);
    style.spacing.interact_size = Vec2::new(38.0, 35.0);
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.menu_margin = egui::Margin::same(8.0);
    style.visuals = build_visuals(theme, palette);
    ctx.set_style(style);
}

fn build_visuals(theme: ThemeMode, palette: ThemePalette) -> Visuals {
    let mut visuals = match theme {
        ThemeMode::Dark => Visuals::dark(),
        ThemeMode::Light => Visuals::light(),
    };
    visuals.override_text_color = Some(palette.text_primary);
    visuals.window_fill = palette.app_bg;
    visuals.panel_fill = palette.panel_bg;
    visuals.faint_bg_color = palette.card_bg;
    visuals.extreme_bg_color = palette.card_alt_bg;
    visuals.code_bg_color = palette.card_alt_bg;
    visuals.selection.bg_fill = palette.accent;
    visuals.selection.stroke = Stroke::new(1.0, palette.text_primary);
    visuals.widgets = Widgets {
        noninteractive: egui::style::WidgetVisuals {
            weak_bg_fill: palette.card_bg,
            bg_fill: palette.card_bg,
            bg_stroke: Stroke::new(1.0, palette.card_border),
            fg_stroke: Stroke::new(1.0, palette.text_primary),
            rounding: Rounding::same(12.0),
            expansion: 0.0,
        },
        inactive: egui::style::WidgetVisuals {
            weak_bg_fill: palette.card_alt_bg,
            bg_fill: palette.card_alt_bg,
            bg_stroke: Stroke::new(1.0, palette.card_border),
            fg_stroke: Stroke::new(1.0, palette.text_primary),
            rounding: Rounding::same(12.0),
            expansion: 0.0,
        },
        hovered: egui::style::WidgetVisuals {
            weak_bg_fill: palette.accent_soft,
            bg_fill: palette.accent_hover,
            bg_stroke: Stroke::new(1.0, palette.accent_hover),
            fg_stroke: Stroke::new(1.2, Color32::WHITE),
            rounding: Rounding::same(12.0),
            expansion: 0.0,
        },
        active: egui::style::WidgetVisuals {
            weak_bg_fill: palette.accent_active,
            bg_fill: palette.accent_active,
            bg_stroke: Stroke::new(1.0, palette.accent_active),
            fg_stroke: Stroke::new(1.2, Color32::WHITE),
            rounding: Rounding::same(12.0),
            expansion: 0.0,
        },
        open: egui::style::WidgetVisuals {
            weak_bg_fill: palette.accent_soft,
            bg_fill: palette.card_alt_bg,
            bg_stroke: Stroke::new(1.0, palette.accent),
            fg_stroke: Stroke::new(1.0, palette.text_primary),
            rounding: Rounding::same(12.0),
            expansion: 0.0,
        },
    };
    visuals.window_stroke = Stroke::new(1.0, palette.card_border);
    visuals.widgets.inactive.bg_fill = palette.card_alt_bg;
    visuals.widgets.noninteractive.bg_fill = palette.card_bg;
    visuals.widgets.inactive.weak_bg_fill = palette.card_alt_bg;
    visuals.widgets.noninteractive.weak_bg_fill = palette.card_bg;
    visuals.widgets.hovered.rounding = Rounding::same(12.0);
    visuals.widgets.active.rounding = Rounding::same(12.0);
    visuals.widgets.inactive.rounding = Rounding::same(12.0);
    visuals.widgets.noninteractive.rounding = Rounding::same(12.0);
    visuals.widgets.open.rounding = Rounding::same(12.0);
    visuals.hyperlink_color = palette.accent;
    visuals
}

fn color(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
