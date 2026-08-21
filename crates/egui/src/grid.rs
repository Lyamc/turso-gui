use egui::{Color32, ComboBox, RichText, Sense, TextEdit, Ui};
use egui_extras::{Column, TableBuilder};
use turso_gui_core::{GridState, PAGE_SIZES};

const COLOR_DIM: Color32 = Color32::from_rgb(153, 153, 153);

#[derive(Default)]
pub struct GridOutput {
    pub sort: Option<usize>,
    pub filter: Option<(usize, String)>,
    pub select_cell: Option<(usize, usize)>,
    pub select_row: Option<usize>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub all_results: Option<bool>,
}

pub fn show_grid(ui: &mut Ui, id_salt: &str, grid: &GridState, sortable: bool) -> GridOutput {
    let mut output = GridOutput::default();

    egui::TopBottomPanel::bottom(egui::Id::new((id_salt, "pager")))
        .exact_height(28.0)
        .show_inside(ui, |ui| {
            show_footer(ui, id_salt, grid, &mut output);
        });

    if grid.headers.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No data to display").color(COLOR_DIM));
        });
        return output;
    }

    let n_data_cols = grid.headers.len();
    let n_rows = grid.rows.len();
    let min_height = ui.available_height();
    let mut table = TableBuilder::new(ui)
        .id_salt(id_salt)
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .min_scrolled_height(min_height)
        .max_scroll_height(f32::INFINITY)
        .auto_shrink([false, false])
        .column(Column::auto().at_least(44.0).clip(false).resizable(false));

    for i in 0..n_data_cols {
        let width = grid.widths.get(i).copied().filter(|w| *w > 0.0).unwrap_or(150.0);
        table = table.column(
            Column::initial(width)
                .resizable(true)
                .clip(true)
                .at_least(40.0),
        );
    }

    table
        .header(52.0, |mut header| {
            header.col(|ui| {
                ui.label(RichText::new("#").strong().color(COLOR_DIM));
            });
            for (i, name) in grid.headers.iter().enumerate() {
                header.col(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let mut title = name.clone();
                        if let Some((col, dir)) = grid.sort {
                            if sortable && col == i {
                                title.push(' ');
                                title.push_str(dir.icon());
                            }
                        }
                        let title_btn = ui.add(
                            egui::Button::new(RichText::new(title).strong())
                                .frame(false)
                                .truncate(),
                        );
                        if sortable && title_btn.clicked() {
                            output.sort = Some(i);
                        }

                        let mut filter = grid.filters.get(i).cloned().unwrap_or_default();
                        let resp = ui.add(
                            TextEdit::singleline(&mut filter)
                                .id_salt(("filter", i))
                                .hint_text("Filter (=, >, NULL)…")
                                .desired_width(ui.available_width()),
                        );
                        if resp.changed() {
                            output.filter = Some((i, filter));
                        }
                    });
                });
            }
        })
        .body(|body| {
            body.rows(22.0, n_rows, |mut row| {
                let row_index = row.index();
                let row_num = grid
                    .page
                    .saturating_mul(grid.page_size)
                    .saturating_add(row_index as u32)
                    .saturating_add(1);

                let (_, num_resp) = row.col(|ui| {
                    ui.label(
                        RichText::new(row_num.to_string())
                            .small()
                            .color(COLOR_DIM),
                    );
                });
                if num_resp.clicked() {
                    output.select_row = Some(row_index);
                }

                for col in 0..n_data_cols {
                    let selected = grid.selected_cell == Some((row_index, col));
                    row.set_selected(selected);
                    let value = grid
                        .rows
                        .get(row_index)
                        .and_then(|r| r.get(col))
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    let (_, resp) = row.col(|ui| {
                        ui.add(egui::Label::new(value).truncate());
                    });
                    if resp.clicked() {
                        output.select_cell = Some((row_index, col));
                    }
                }
            });
        });

    output
}

fn show_footer(ui: &mut Ui, id_salt: &str, grid: &GridState, output: &mut GridOutput) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(
            RichText::new(format!("Total Records: {}", grid.total_records))
                .small()
                .color(COLOR_DIM),
        );

        ui.add_space(12.0);
        ui.label(RichText::new("Page:").small().color(COLOR_DIM));

        let total_pages = grid.total_pages().max(1);
        let mut page = grid.page.saturating_add(1).min(total_pages).max(1);
        if total_pages <= 1000 {
            ComboBox::from_id_salt((id_salt, "page"))
                .selected_text(page.to_string())
                .width(64.0)
                .show_ui(ui, |ui| {
                    for p in 1..=total_pages {
                        ui.selectable_value(&mut page, p, p.to_string());
                    }
                });
        } else if ui
            .add(egui::DragValue::new(&mut page).speed(1).range(1.0..=total_pages as f32))
            .changed()
        {
            page = page.clamp(1, total_pages);
        }
        if page.saturating_sub(1) != grid.page {
            output.page = Some(page.saturating_sub(1));
        }
        ui.label(
            RichText::new(format!("of {total_pages}"))
                .small()
                .color(COLOR_DIM),
        );

        ui.add_space(12.0);
        ui.label(RichText::new("Page Size:").small().color(COLOR_DIM));
        let mut page_size = grid.page_size;
        ComboBox::from_id_salt((id_salt, "page_size"))
            .selected_text(page_size.to_string())
            .width(72.0)
            .show_ui(ui, |ui| {
                for &size in &PAGE_SIZES {
                    ui.selectable_value(&mut page_size, size, size.to_string());
                }
            });
        if page_size != grid.page_size {
            output.page_size = Some(page_size);
        }

        ui.add_space(12.0);
        let mut all = grid.all_results;
        if ui.checkbox(&mut all, "Return All").changed() {
            output.all_results = Some(all);
        }
    });
}
