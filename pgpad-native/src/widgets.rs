use egui::{
    Align, Align2, Color32, FontId, Frame, Layout, Rect, RichText, Sense, Stroke, Ui, Vec2,
};
use egui_phosphor::regular;

use crate::model::{ConnectionKind, DummyConnection, SidebarTab};
use crate::style::PgTokens;

pub fn icon_label(icon: &str, label: &str) -> String {
    format!("{icon}  {label}")
}

pub fn command_button(ui: &mut Ui, icon: &str, label: &str) -> egui::Response {
    let button = egui::Button::new(
        RichText::new(icon_label(icon, label))
            .size(12.0)
            .color(PgTokens::text()),
    )
    .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 15))
    .stroke(Stroke::NONE)
    .corner_radius(7)
    .min_size(Vec2::new(0.0, 28.0));

    ui.add(button)
}

pub fn icon_button(ui: &mut Ui, icon: &str, selected: bool) -> egui::Response {
    let fill = if selected {
        PgTokens::raised()
    } else {
        Color32::TRANSPARENT
    };

    let button = egui::Button::new(RichText::new(icon).size(16.0).color(PgTokens::muted()))
        .fill(fill)
        .stroke(Stroke::NONE)
        .corner_radius(7)
        .min_size(Vec2::splat(34.0));

    ui.add(button)
}

pub fn connection_pill(ui: &mut Ui, name: &str, connected: bool) {
    Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 15))
        .corner_radius(7)
        .inner_margin(egui::Margin::symmetric(9, 5))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                status_dot(
                    ui,
                    if connected {
                        PgTokens::success()
                    } else {
                        PgTokens::danger()
                    },
                );
                ui.label(
                    RichText::new(name)
                        .size(12.0)
                        .strong()
                        .color(PgTokens::text()),
                );
            });
        });
}

pub fn status_dot(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.0, color);
}

pub fn sidebar_tab_icon(tab: SidebarTab) -> &'static str {
    match tab {
        SidebarTab::Connections => regular::PLUG,
        SidebarTab::Items => regular::TABLE,
        SidebarTab::Scripts => regular::FILE_CODE,
        SidebarTab::History => regular::CLOCK_COUNTER_CLOCKWISE,
    }
}

pub fn database_icon(kind: ConnectionKind) -> &'static str {
    match kind {
        ConnectionKind::Postgres => regular::DATABASE,
        ConnectionKind::Sqlite => regular::FILE_SQL,
    }
}

pub fn connection_row(ui: &mut Ui, connection: &DummyConnection, selected: bool) -> egui::Response {
    let row_height = 52.0;
    let available = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(available, row_height), Sense::click());
    let fill = if selected {
        PgTokens::active()
    } else if response.hovered() {
        PgTokens::hover()
    } else {
        Color32::TRANSPARENT
    };

    ui.painter().rect_filled(rect.shrink(2.0), 8.0, fill);

    let mut row_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(8.0, 5.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );

    status_dot(
        &mut row_ui,
        if connection.connected {
            PgTokens::success()
        } else {
            PgTokens::faint()
        },
    );
    row_ui.add_space(4.0);
    row_ui.label(
        RichText::new(database_icon(connection.kind))
            .size(16.0)
            .color(PgTokens::text()),
    );
    row_ui.add_space(4.0);
    row_ui.vertical(|ui| {
        ui.label(
            RichText::new(connection.name)
                .size(13.0)
                .strong()
                .color(PgTokens::text()),
        );
        ui.label(
            RichText::new(connection.detail)
                .size(12.0)
                .monospace()
                .color(PgTokens::muted()),
        );
    });

    response
}

pub struct TabResponse {
    pub response: egui::Response,
    pub close_clicked: bool,
}

pub fn script_tab(ui: &mut Ui, title: &str, active: bool, dirty: bool, width: f32) -> TabResponse {
    painted_tab(ui, title, active, dirty, true, width)
}

pub fn result_tab(ui: &mut Ui, title: &str, active: bool, width: f32) -> TabResponse {
    painted_tab(ui, title, active, false, false, width)
}

pub fn tab_plus(ui: &mut Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(42.0, 34.0), Sense::click());
    let fill = if response.hovered() {
        PgTokens::hover()
    } else {
        PgTokens::background()
    };

    ui.painter().rect_filled(rect, 0.0, fill);
    paint_tab_border(ui, rect, false);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        regular::PLUS,
        FontId::proportional(17.0),
        PgTokens::muted(),
    );

    response
}

fn painted_tab(
    ui: &mut Ui,
    title: &str,
    active: bool,
    dirty: bool,
    closable: bool,
    width: f32,
) -> TabResponse {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click());
    let fill = if active {
        PgTokens::panel()
    } else if response.hovered() {
        PgTokens::hover()
    } else {
        PgTokens::background()
    };
    let text_color = if active {
        PgTokens::text()
    } else {
        PgTokens::muted()
    };
    let close_rect = Rect::from_min_size(
        egui::pos2(rect.right() - 34.0, rect.top()),
        Vec2::new(34.0, rect.height()),
    );
    let close_clicked = closable
        && active
        && response.clicked()
        && response
            .interact_pointer_pos()
            .is_some_and(|pos| close_rect.contains(pos));

    ui.painter().rect_filled(rect, 0.0, fill);
    paint_tab_border(ui, rect, active);

    let has_close_area = closable && active;
    let title_right = if has_close_area {
        close_rect.left() - 6.0
    } else if dirty {
        rect.right() - 24.0
    } else {
        rect.right() - 12.0
    };
    let text_pos = egui::pos2(rect.left() + 14.0, rect.center().y);
    let clip_rect = Rect::from_min_max(
        egui::pos2(rect.left() + 10.0, rect.top()),
        egui::pos2(title_right, rect.bottom()),
    );
    ui.painter().with_clip_rect(clip_rect).text(
        text_pos,
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(13.0),
        text_color,
    );

    if dirty {
        let dot_x = if has_close_area {
            title_right + 7.0
        } else {
            rect.right() - 12.0
        };
        ui.painter()
            .circle_filled(egui::pos2(dot_x, rect.center().y), 2.0, PgTokens::warning());
    }

    if has_close_area {
        let close_fill = if response.hovered()
            && response
                .hover_pos()
                .is_some_and(|pos| close_rect.contains(pos))
        {
            PgTokens::raised()
        } else {
            Color32::TRANSPARENT
        };
        ui.painter()
            .rect_filled(close_rect.shrink2(Vec2::new(6.0, 6.0)), 5.0, close_fill);
        ui.painter().text(
            close_rect.center(),
            Align2::CENTER_CENTER,
            regular::X,
            FontId::proportional(14.0),
            PgTokens::muted(),
        );
    }

    TabResponse {
        response,
        close_clicked,
    }
}

fn paint_tab_border(ui: &Ui, rect: Rect, active: bool) {
    let stroke = Stroke::new(1.0, PgTokens::border());
    ui.painter()
        .line_segment([rect.left_top(), rect.right_top()], stroke);
    ui.painter()
        .line_segment([rect.left_top(), rect.left_bottom()], stroke);
    ui.painter()
        .line_segment([rect.right_top(), rect.right_bottom()], stroke);
    ui.painter()
        .line_segment([rect.left_bottom(), rect.right_bottom()], stroke);

    if active {
        ui.painter().line_segment(
            [
                egui::pos2(rect.left(), rect.bottom() - 1.0),
                egui::pos2(rect.right(), rect.bottom() - 1.0),
            ],
            Stroke::new(2.0, PgTokens::primary()),
        );
    }
}

pub fn section_label(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text.to_ascii_uppercase())
            .size(10.0)
            .strong()
            .color(PgTokens::faint()),
    );
}
