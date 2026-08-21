#![windows_subsystem = "windows"]

mod grid;

use clap::Parser;
use eframe::egui::{self, Color32, RichText, TextEdit, ViewportBuilder};
use grid::GridOutput;
use rfd::FileDialog;
use turso_gui_core::{init_gui_host, AppModel, StatusKind, Tab, WindowPlacement};

const COLOR_INFO: Color32 = Color32::from_rgb(64, 156, 255);
const COLOR_ERROR: Color32 = Color32::from_rgb(255, 96, 96);
const COLOR_SUCCESS: Color32 = Color32::from_rgb(80, 220, 80);
const COLOR_DIM: Color32 = Color32::from_rgb(153, 153, 153);
const COLOR_TAB_ACTIVE: Color32 = Color32::from_rgb(26, 102, 204);
const COLOR_TAB_INACTIVE: Color32 = Color32::from_rgb(51, 51, 51);

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Turso / SQLite DB Browser (egui)", long_about = None)]
struct Args {
    /// Path to the database file or Turso URL
    #[arg(short, long)]
    database: Option<String>,

    /// Authentication token for Turso (if using a remote URL)
    #[arg(short, long)]
    token: Option<String>,

    /// Enable debug output
    #[arg(short = 'D', long)]
    debug: bool,

    /// Open a console for logs. A terminal that already launched this process is reused.
    #[arg(long)]
    console: bool,
}

struct TursoGui {
    model: AppModel,
}

impl TursoGui {
    fn new(args: Args) -> Self {
        Self {
            model: AppModel::from_args(args.database, args.token, args.debug),
        }
    }

    fn sqlite_dialog() -> FileDialog {
        FileDialog::new().add_filter("SQLite", &["db", "sqlite"])
    }

    fn new_database(&mut self) {
        if let Some(path) = Self::sqlite_dialog()
            .set_file_name("new.db")
            .save_file()
        {
            let _ = self.model.open_path(path.to_string_lossy().into_owned());
        }
    }

    fn open_database(&mut self) {
        if let Some(path) = Self::sqlite_dialog().pick_file() {
            let _ = self.model.open_path(path.to_string_lossy().into_owned());
        }
    }

    fn apply_grid_output(&mut self, output: GridOutput) {
        if let Some(row) = output.select_row {
            self.model.select_row(row);
        }
        if let Some((row, col)) = output.select_cell {
            self.model.select_cell(row, col);
        }
        if let Some((i, value)) = output.filter {
            match self.model.active_tab {
                Tab::ExecuteSql => {
                    let _ = self.model.set_sql_filter(i, value);
                }
                _ => {
                    let _ = self.model.set_browse_filter(i, value);
                }
            }
        }
        if let Some(i) = output.sort {
            let _ = self.model.sort_column(i);
        }
        if let Some(page) = output.page {
            let _ = self.model.change_page(page);
        }
        if let Some(size) = output.page_size {
            let _ = self.model.change_page_size(size);
        }
        if let Some(all) = output.all_results {
            let _ = self.model.toggle_all_results(all);
        }
    }

    fn show_status(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some((text, kind)) = self.model.status_text() {
                let color = match kind {
                    StatusKind::Info => COLOR_INFO,
                    StatusKind::Error => COLOR_ERROR,
                    StatusKind::Success => COLOR_SUCCESS,
                };
                ui.colored_label(color, text);
            }
        });
    }

    fn show_connect(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(64.0);
            ui.label(
                RichText::new("Connect to Turso / SQLite")
                    .size(30.0)
                    .strong(),
            );
            ui.add_space(16.0);
            ui.add(
                TextEdit::singleline(&mut self.model.db_path)
                    .hint_text("Database Path")
                    .desired_width(420.0),
            );
            ui.add_space(8.0);
            if ui.add_sized([120.0, 32.0], egui::Button::new("Connect")).clicked() {
                let _ = self.model.connect();
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("New Database").clicked() {
                    self.new_database();
                }
                if ui.button("Open Database").clicked() {
                    self.open_database();
                }
            });
        });
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("New").clicked() {
                self.new_database();
            }
            if ui.button("Open").clicked() {
                self.open_database();
            }
            if ui.button("Close").clicked() {
                self.model.close();
            }
            if ui
                .add_enabled(self.model.has_changes, egui::Button::new("Write"))
                .clicked()
            {
                let _ = self.model.write_changes();
            }
            if ui
                .add_enabled(self.model.has_changes, egui::Button::new("Revert"))
                .clicked()
            {
                let _ = self.model.revert_changes();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(&self.model.db_path).color(COLOR_DIM));
            });
        });
    }

    fn show_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for tab in Tab::all() {
                let selected = self.model.active_tab == tab;
                let fill = if selected {
                    COLOR_TAB_ACTIVE
                } else {
                    COLOR_TAB_INACTIVE
                };
                let text = if selected {
                    RichText::new(tab.label()).strong()
                } else {
                    RichText::new(tab.label()).color(COLOR_DIM)
                };
                if ui
                    .add(egui::Button::new(text).fill(fill).min_size(egui::vec2(96.0, 28.0)))
                    .clicked()
                {
                    self.model.switch_tab(tab);
                }
            }
        });
    }

    fn show_structure(&mut self, ui: &mut egui::Ui) {
        let mut clicked: Option<String> = None;
        egui::SidePanel::left("structure_tables")
            .resizable(true)
            .default_width(220.0)
            .show_inside(ui, |ui| {
                ui.heading("Tables");
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if self.model.table_names.is_empty() {
                        ui.label(RichText::new("No tables").color(COLOR_DIM));
                    }
                    for name in &self.model.table_names {
                        let selected =
                            self.model.selected_structure_table.as_deref() == Some(name.as_str());
                        if ui.selectable_label(selected, name).clicked() {
                            clicked = Some(name.clone());
                        }
                    }
                });
            });

        if let Some(name) = clicked {
            let _ = self.model.select_structure_table(&name);
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.heading("Columns");
                ui.add_space(6.0);
                egui::Grid::new("structure_columns")
                    .striped(true)
                    .num_columns(4)
                    .spacing([16.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("PK");
                        ui.strong("Name");
                        ui.strong("Type");
                        ui.strong("NOT NULL");
                        ui.end_row();
                        for col in &self.model.selected_table_columns {
                            ui.label(if col.pk { "🔑" } else { "" });
                            ui.label(&col.name);
                            ui.label(RichText::new(&col.data_type).color(COLOR_DIM));
                            ui.label(if col.not_null { "NOT NULL" } else { "" });
                            ui.end_row();
                        }
                    });
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);
                ui.heading("Schema");
                ui.add_space(6.0);
                ui.label(RichText::new(self.model.structure_sql()).monospace());
            });
        });
    }

    fn show_cell_editor(&mut self, ui: &mut egui::Ui) {
        ui.heading("Cell Editor");
        ui.add_space(6.0);
        ui.add_sized(
            ui.available_size(),
            TextEdit::multiline(&mut self.model.cell_editor)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Monospace),
        );
    }

    fn show_browse(&mut self, ui: &mut egui::Ui) {
        let table_names = self.model.table_names.clone();
        let mut selected = self.model.browse_table.clone();
        let mut refresh = false;

        egui::TopBottomPanel::top("browse_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Table:");
                egui::ComboBox::from_id_salt("browse_table")
                    .selected_text(selected.as_deref().unwrap_or("Select table"))
                    .show_ui(ui, |ui| {
                        for name in &table_names {
                            ui.selectable_value(&mut selected, Some(name.clone()), name);
                        }
                    });
                refresh = ui.button("Refresh").clicked();
            });
        });

        if selected != self.model.browse_table {
            if let Some(name) = selected {
                let _ = self.model.browse_table_named(&name);
            }
        } else if refresh {
            let _ = self.model.load_browse_data();
        }

        egui::SidePanel::right("browse_cell_editor")
            .resizable(true)
            .default_width(280.0)
            .show_inside(ui, |ui| {
                self.show_cell_editor(ui);
            });

        let output = grid::show_grid(ui, "browse_grid", &self.model.browse_state, true);
        self.apply_grid_output(output);
    }

    fn show_sql(&mut self, ui: &mut egui::Ui) {
        let execute_shortcut = ui.ctx().input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                || i.consume_key(egui::Modifiers::CTRL, egui::Key::Enter)
        });

        let mut execute = execute_shortcut;
        egui::TopBottomPanel::top("sql_editor")
            .resizable(true)
            .default_height(120.0)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let editor_width = (ui.available_width() - 96.0).max(80.0);
                    let editor_height = (ui.available_height() - 8.0).max(48.0);
                    ui.add_sized(
                        [editor_width, editor_height],
                        TextEdit::multiline(&mut self.model.query)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace)
                            .hint_text("Enter SQL query here"),
                    );
                    if ui
                        .add_sized([88.0, 28.0], egui::Button::new("Execute"))
                        .on_hover_text("Ctrl+Enter")
                        .clicked()
                    {
                        execute = true;
                    }
                });
            });

        if execute {
            let _ = self.model.execute_query();
        }

        egui::SidePanel::right("sql_cell_editor")
            .resizable(true)
            .default_width(280.0)
            .show_inside(ui, |ui| {
                self.show_cell_editor(ui);
            });

        let output = grid::show_grid(ui, "sql_grid", &self.model.sql_state, true);
        self.apply_grid_output(output);
    }
}

impl eframe::App for TursoGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.model.is_connected() {
            egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
                self.show_toolbar(ui);
            });
        }

        if self.model.is_connected() {
            egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
                self.show_tabs(ui);
            });
            egui::TopBottomPanel::bottom("status")
                .exact_height(24.0)
                .show(ctx, |ui| {
                    self.show_status(ui);
                });
            egui::CentralPanel::default().show(ctx, |ui| match self.model.active_tab {
                Tab::Structure => self.show_structure(ui),
                Tab::BrowseData => self.show_browse(ui),
                Tab::ExecuteSql => self.show_sql(ui),
            });
        } else {
            egui::TopBottomPanel::bottom("status")
                .exact_height(24.0)
                .show(ctx, |ui| {
                    self.show_status(ui);
                });
            egui::CentralPanel::default().show(ctx, |ui| {
                self.show_connect(ui);
            });
        }
    }
}

fn main() -> anyhow::Result<()> {
    init_gui_host();
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let place = WindowPlacement::suggested();

    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Turso DB Browser (egui)")
            .with_inner_size([place.logical_width, place.logical_height])
            .with_min_inner_size([place.min_logical_width, place.min_logical_height])
            .with_position([place.logical_x, place.logical_y]),
        ..Default::default()
    };

    eframe::run_native(
        "Turso DB Browser (egui)",
        native_options,
        Box::new(move |cc| {
            cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(TursoGui::new(args)))
        }),
    )
    .map_err(|err| anyhow::anyhow!("eframe error: {err}"))
}
