use egui::{Color32, FontFamily, FontId, Stroke, TextStyle, Visuals};

pub struct PgTokens;

impl PgTokens {
    pub fn background() -> Color32 {
        Color32::from_rgb(0x1f, 0x1f, 0x1f)
    }

    pub fn titlebar() -> Color32 {
        Color32::from_rgb(0x33, 0x33, 0x33)
    }

    pub fn sidebar() -> Color32 {
        Color32::from_rgb(0x20, 0x21, 0x24)
    }

    pub fn panel() -> Color32 {
        Color32::from_rgb(0x2b, 0x2b, 0x2d)
    }

    pub fn raised() -> Color32 {
        Color32::from_rgb(0x35, 0x35, 0x36)
    }

    pub fn table_header() -> Color32 {
        Color32::from_rgb(0x4b, 0x4b, 0x4c)
    }

    pub fn table_row() -> Color32 {
        Color32::from_rgb(0x25, 0x25, 0x26)
    }

    pub fn table_row_alt() -> Color32 {
        Color32::from_rgb(0x29, 0x29, 0x2a)
    }

    pub fn hover() -> Color32 {
        Color32::from_rgb(0x3a, 0x3a, 0x3c)
    }

    pub fn active() -> Color32 {
        Color32::from_rgb(0x24, 0x44, 0x69)
    }

    pub fn border() -> Color32 {
        Color32::from_rgb(0x44, 0x44, 0x46)
    }

    pub fn text() -> Color32 {
        Color32::from_rgb(0xec, 0xef, 0xf4)
    }

    pub fn muted() -> Color32 {
        Color32::from_rgb(0xae, 0xb6, 0xc2)
    }

    pub fn faint() -> Color32 {
        Color32::from_rgb(0x7e, 0x87, 0x94)
    }

    pub fn primary() -> Color32 {
        Color32::from_rgb(0x3b, 0x82, 0xf6)
    }

    pub fn success() -> Color32 {
        Color32::from_rgb(0x22, 0xc5, 0x5e)
    }

    pub fn warning() -> Color32 {
        Color32::from_rgb(0xd9, 0x77, 0x06)
    }

    pub fn danger() -> Color32 {
        Color32::from_rgb(0xef, 0x44, 0x44)
    }

    pub fn stroke() -> Stroke {
        Stroke::new(1.0, Self::border())
    }
}

pub fn setup_styling(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);

    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        style.visuals = Visuals::dark();
        style.visuals.override_text_color = Some(PgTokens::text());
        style.visuals.panel_fill = PgTokens::background();
        style.visuals.window_fill = PgTokens::panel();
        style.visuals.extreme_bg_color = PgTokens::background();
        style.visuals.faint_bg_color = PgTokens::panel();
        style.visuals.selection.bg_fill = PgTokens::active();
        style.visuals.selection.stroke = Stroke::new(1.0, PgTokens::primary());
        style.visuals.hyperlink_color = PgTokens::primary();
        style.visuals.widgets.noninteractive.bg_fill = PgTokens::panel();
        style.visuals.widgets.noninteractive.bg_stroke = PgTokens::stroke();
        style.visuals.widgets.inactive.bg_fill = PgTokens::raised();
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, PgTokens::border());
        style.visuals.widgets.hovered.bg_fill = PgTokens::hover();
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, PgTokens::primary());
        style.visuals.widgets.active.bg_fill = PgTokens::active();
        style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, PgTokens::primary());

        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.menu_margin = egui::Margin::symmetric(8, 6);
        style.spacing.indent = 14.0;

        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(18.0, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(12.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        );
    });
}

pub fn fill_rect(ui: &egui::Ui, rect: egui::Rect, fill: Color32) {
    ui.painter().rect_filled(rect, 0.0, fill);
}
