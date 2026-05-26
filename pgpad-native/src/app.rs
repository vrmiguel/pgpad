use eframe::egui;
use egui::{
    Align, Align2, CentralPanel, Color32, FontId, Layout, Panel, RichText, ScrollArea, TextEdit,
    Vec2,
};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular;
use egui_tiles::{Behavior, Linear, LinearDir, TileId, Tiles, Tree, UiResponse};

use crate::model::{result_columns, result_rows, AppModel, SidebarTab};
use crate::style::{fill_rect, setup_styling, PgTokens};
use crate::widgets;

#[derive(Debug, Clone, PartialEq, Eq)]
enum WorkspacePane {
    Editor,
    Results,
}

pub struct App {
    model: AppModel,
    workspace: Tree<WorkspacePane>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_styling(&cc.egui_ctx);
        #[cfg(target_arch = "wasm32")]
        cc.egui_ctx.set_pixels_per_point(2.0);

        Self {
            model: AppModel::default(),
            workspace: workspace_tree(),
        }
    }

    fn top_command_bar(&mut self, root_ui: &mut egui::Ui) {
        Panel::top("top_command_bar")
            .exact_size(42.0)
            .frame(
                egui::Frame::NONE
                    .fill(PgTokens::titlebar())
                    .stroke(egui::Stroke::new(1.0, PgTokens::border()))
                    .inner_margin(egui::Margin::symmetric(14, 6)),
            )
            .show_inside(root_ui, |ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add_space(360.0);
                    widgets::command_button(ui, regular::PLAY, "Run Query");
                    widgets::command_button(ui, regular::FLOPPY_DISK, "Save Script");
                    ui.add_space(8.0);

                    if let Some(connection) = self.model.selected_connection() {
                        widgets::connection_pill(ui, connection.name, connection.connected);
                    } else {
                        widgets::connection_pill(ui, "No connection", false);
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let _ = widgets::icon_button(ui, regular::SUN, false);
                    });
                });
            });
    }

    fn sidebar(&mut self, root_ui: &mut egui::Ui) {
        let width = if self.model.sidebar_collapsed {
            56.0
        } else {
            320.0
        };

        Panel::left("sidebar")
            .exact_size(width)
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(PgTokens::sidebar())
                    .stroke(egui::Stroke::new(1.0, PgTokens::border()))
                    .inner_margin(egui::Margin::ZERO),
            )
            .show_inside(root_ui, |ui| {
                if self.model.sidebar_collapsed {
                    self.collapsed_sidebar_ui(ui);
                } else {
                    self.expanded_sidebar_ui(ui);
                }
            });
    }

    fn collapsed_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            if widgets::icon_button(ui, regular::CARET_RIGHT, false)
                .on_hover_text("Expand sidebar")
                .clicked()
            {
                self.model.sidebar_collapsed = false;
            }
            ui.add_space(14.0);

            for tab in SidebarTab::ALL {
                let selected = self.model.active_sidebar_tab == tab;
                if widgets::icon_button(ui, widgets::sidebar_tab_icon(tab), selected)
                    .on_hover_text(tab.label())
                    .clicked()
                {
                    self.model.active_sidebar_tab = tab;
                    self.model.sidebar_collapsed = false;
                }
                ui.add_space(6.0);
            }
        });
    }

    fn expanded_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(18, 18))
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.label(
                            RichText::new(regular::DATABASE)
                                .size(22.0)
                                .color(PgTokens::primary()),
                        );
                        ui.label(
                            RichText::new("PgPad")
                                .size(20.0)
                                .strong()
                                .color(PgTokens::text()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::icon_button(ui, regular::CARET_LEFT, false)
                                .on_hover_text("Collapse sidebar")
                                .clicked()
                            {
                                self.model.sidebar_collapsed = true;
                            }
                        });
                    });
                });

            ui.separator();
            ui.add_space(8.0);

            egui::Frame::NONE
                .fill(PgTokens::background())
                .corner_radius(7)
                .inner_margin(egui::Margin::symmetric(4, 3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for tab in SidebarTab::ALL {
                            let selected = self.model.active_sidebar_tab == tab;
                            if widgets::icon_button(ui, widgets::sidebar_tab_icon(tab), selected)
                                .on_hover_text(tab.label())
                                .clicked()
                            {
                                self.model.active_sidebar_tab = tab;
                            }
                        }
                    });
                });

            ui.add_space(8.0);
            self.sidebar_content(ui);
        });
    }

    fn sidebar_content(&mut self, ui: &mut egui::Ui) {
        match self.model.active_sidebar_tab {
            SidebarTab::Connections => self.connections_sidebar(ui),
            SidebarTab::Items => {
                placeholder_sidebar(ui, "Database Items", "Schemas and tables will land here.")
            }
            SidebarTab::Scripts => {
                placeholder_sidebar(ui, "Scripts", "Saved scripts will land here.")
            }
            SidebarTab::History => {
                placeholder_sidebar(ui, "Query History", "Recent query runs will land here.")
            }
        }
    }

    fn connections_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            widgets::icon_button(ui, regular::PLUS, false).on_hover_text("Add connection");
            widgets::icon_button(ui, regular::SLIDERS_HORIZONTAL, false)
                .on_hover_text("Connection filters");
            widgets::icon_button(ui, regular::PLUGS_CONNECTED, false)
                .on_hover_text("Connection actions");
        });
        ui.add_space(8.0);

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (idx, connection) in self.model.connections.iter().enumerate() {
                    if widgets::connection_row(
                        ui,
                        connection,
                        self.model.selected_connection == idx,
                    )
                    .clicked()
                    {
                        self.model.selected_connection = idx;
                    }
                }
            });
    }

    fn main_workspace(&mut self, root_ui: &mut egui::Ui) {
        CentralPanel::default()
            .frame(egui::Frame::NONE.fill(PgTokens::background()))
            .show_inside(root_ui, |ui| {
                let mut behavior = WorkspaceBehavior {
                    model: &mut self.model,
                };
                self.workspace.ui(&mut behavior, ui);
            });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.top_command_bar(ui);
        self.sidebar(ui);
        self.main_workspace(ui);
    }
}

fn workspace_tree() -> Tree<WorkspacePane> {
    let mut tiles = Tiles::default();
    let editor = tiles.insert_pane(WorkspacePane::Editor);
    let results = tiles.insert_pane(WorkspacePane::Results);
    let root = tiles.insert_container(Linear::new_binary(
        LinearDir::Vertical,
        [editor, results],
        0.6,
    ));

    Tree::new("pgpad_workspace", root, tiles)
}

struct WorkspaceBehavior<'a> {
    model: &'a mut AppModel,
}

impl Behavior<WorkspacePane> for WorkspaceBehavior<'_> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: TileId,
        pane: &mut WorkspacePane,
    ) -> UiResponse {
        match pane {
            WorkspacePane::Editor => editor_pane(ui, self.model),
            WorkspacePane::Results => results_pane(ui, self.model),
        }

        UiResponse::default()
    }

    fn tab_title_for_pane(&mut self, pane: &WorkspacePane) -> egui::WidgetText {
        match pane {
            WorkspacePane::Editor => "Editor".into(),
            WorkspacePane::Results => "Results".into(),
        }
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        0.0
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        1.0
    }

    fn resize_stroke(
        &self,
        _style: &egui::Style,
        _resize_state: egui_tiles::ResizeState,
    ) -> egui::Stroke {
        egui::Stroke::new(1.0, PgTokens::border())
    }

    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> Color32 {
        PgTokens::background()
    }
}

fn editor_pane(ui: &mut egui::Ui, model: &mut AppModel) {
    fill_rect(ui, ui.max_rect(), PgTokens::panel());
    ui.vertical(|ui| {
        script_tabs(ui, model);
        horizontal_rule(ui);

        egui::Frame::NONE
            .fill(PgTokens::panel())
            .inner_margin(egui::Margin::ZERO)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("1")
                            .monospace()
                            .size(13.0)
                            .color(PgTokens::faint()),
                    );
                    ui.add_space(10.0);

                    if let Some(script) = model.active_script_mut() {
                        let response = ui.add_sized(
                            ui.available_size(),
                            TextEdit::multiline(&mut script.text)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .code_editor()
                                .frame(egui::Frame::NONE),
                        );
                        if response.changed() {
                            script.dirty = true;
                        }
                    }
                });
            });
    });
}

fn script_tabs(ui: &mut egui::Ui, model: &mut AppModel) {
    let mut close_idx = None;
    egui::Frame::NONE
        .fill(PgTokens::background())
        .inner_margin(egui::Margin::ZERO)
        .show(ui, |ui| {
            ScrollArea::horizontal()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.horizontal(|ui| {
                        for idx in 0..model.scripts.len() {
                            let active = model.active_script == idx;
                            let title = model.scripts[idx].title.clone();
                            let dirty = model.scripts[idx].dirty;

                            let tab = widgets::script_tab(ui, &title, active, dirty, 170.0);
                            if tab.close_clicked {
                                close_idx = Some(idx);
                            } else if tab.response.clicked() {
                                model.select_script(idx);
                            }
                        }

                        if widgets::tab_plus(ui).on_hover_text("New script").clicked() {
                            model.add_script();
                        }
                    });
                });
        });

    if let Some(idx) = close_idx {
        model.close_script(idx);
    }
}

fn results_pane(ui: &mut egui::Ui, model: &mut AppModel) {
    fill_rect(ui, ui.max_rect(), PgTokens::panel());
    ui.vertical(|ui| {
        result_tabs(ui, model);
        horizontal_rule(ui);
        let row_count = result_rows().len();
        result_table(ui, model);
        results_footer(ui, row_count);
    });
}

fn result_tabs(ui: &mut egui::Ui, model: &mut AppModel) {
    egui::Frame::NONE
        .fill(PgTokens::background())
        .inner_margin(egui::Margin::ZERO)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.horizontal(|ui| {
                for (idx, result) in model.results.iter().enumerate() {
                    if widgets::result_tab(ui, result.title, model.active_result == idx, 220.0)
                        .response
                        .clicked()
                    {
                        model.active_result = idx;
                    }
                }
            });
        });
}

fn result_table(ui: &mut egui::Ui, model: &mut AppModel) {
    let columns = result_columns();
    let rows = result_rows();
    let selected_row = &mut model.selected_result_row;
    let max_scroll_height = (ui.available_height() - 34.0).max(120.0);

    let saved_spacing = ui.spacing().item_spacing;
    let saved_visuals = ui.visuals().clone();
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    ui.visuals_mut().faint_bg_color = PgTokens::table_row_alt();
    ui.visuals_mut().selection.bg_fill = PgTokens::active();
    ui.visuals_mut().widgets.hovered.bg_fill = PgTokens::hover();

    egui::Frame::NONE
        .fill(PgTokens::table_row())
        .inner_margin(egui::Margin::ZERO)
        .show(ui, |ui| {
            ScrollArea::horizontal()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    TableBuilder::new(ui)
                        .id_salt("pgpad_result_table")
                        .striped(true)
                        .resizable(true)
                        .sense(egui::Sense::click())
                        .cell_layout(Layout::left_to_right(Align::Center))
                        .min_scrolled_height(0.0)
                        .max_scroll_height(max_scroll_height)
                        .auto_shrink([false, false])
                        .column(
                            Column::initial(260.0)
                                .at_least(120.0)
                                .clip(true)
                                .resizable(true),
                        )
                        .column(
                            Column::initial(140.0)
                                .at_least(90.0)
                                .clip(true)
                                .resizable(true),
                        )
                        .column(
                            Column::initial(180.0)
                                .at_least(110.0)
                                .clip(true)
                                .resizable(true),
                        )
                        .column(
                            Column::initial(230.0)
                                .at_least(150.0)
                                .clip(true)
                                .resizable(true),
                        )
                        .column(
                            Column::remainder()
                                .at_least(150.0)
                                .clip(true)
                                .resizable(true),
                        )
                        .header(30.0, |mut header| {
                            for column in columns {
                                header.col(|ui| table_header_cell(ui, column));
                            }
                        })
                        .body(|body| {
                            body.rows(30.0, rows.len(), |mut row| {
                                let row_idx = row.index();
                                row.set_selected(*selected_row == Some(row_idx));

                                for value in rows[row_idx].iter() {
                                    row.col(|ui| table_body_cell(ui, value));
                                }

                                if row.response().clicked() {
                                    *selected_row = match *selected_row {
                                        Some(selected) if selected == row_idx => None,
                                        _ => Some(row_idx),
                                    };
                                }
                            });
                        });
                });
        });

    ui.spacing_mut().item_spacing = saved_spacing;
    ui.style_mut().visuals = saved_visuals;
}

fn table_header_cell(ui: &mut egui::Ui, text: &str) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    let stroke = egui::Stroke::new(1.0, PgTokens::border());

    painter.rect_filled(rect, 0.0, PgTokens::table_header());
    painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
    painter.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
    painter
        .with_clip_rect(rect.shrink2(Vec2::new(9.0, 0.0)))
        .text(
            egui::pos2(rect.left() + 10.0, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(12.0),
            PgTokens::muted(),
        );
}

fn table_body_cell(ui: &mut egui::Ui, text: &str) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    let stroke = egui::Stroke::new(1.0, PgTokens::border());

    painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
    painter.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
    painter
        .with_clip_rect(rect.shrink2(Vec2::new(9.0, 0.0)))
        .text(
            egui::pos2(rect.left() + 10.0, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            FontId::monospace(12.0),
            PgTokens::text(),
        );
}

fn results_footer(ui: &mut egui::Ui, row_count: usize) {
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.add_space(12.0);
        ui.label(
            RichText::new(format!("{row_count} rows"))
                .size(12.0)
                .color(PgTokens::muted()),
        );
        ui.label(RichText::new(".").size(12.0).color(PgTokens::faint()));
        widgets::command_button(ui, regular::COPY, "Copy");
    });
}

fn placeholder_sidebar(ui: &mut egui::Ui, title: &str, body: &str) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            widgets::section_label(ui, title);
            ui.add_space(8.0);
            ui.label(RichText::new(body).size(12.0).color(PgTokens::muted()));
        });
}

fn horizontal_rule(ui: &mut egui::Ui) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        egui::Stroke::new(1.0, PgTokens::border()),
    );
}
