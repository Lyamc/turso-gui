use std::ops::Range;

use gpui::{
    div, prelude::*, px, rgb, uniform_list, App, Context, CursorStyle, ElementId, FocusHandle,
    FontWeight, KeyDownEvent, ListHorizontalSizingBehavior, MouseButton, Pixels, Window,
};
use rfd::FileDialog;
use turso_gui_core::{AppModel, PAGE_SIZES, SortDirection, StatusKind, Tab};

use crate::input::{apply_keystroke, display_value, ActiveField, KeyEffect};

const BG_WINDOW: u32 = 0x0d0d0d;
const BG_HEADER: u32 = 0x333333;
const BG_CELL: u32 = 0x1a1a1a;
const BG_CELL_ALT: u32 = 0x121212;
const BG_SELECTED: u32 = 0x1a66cc;
const ACCENT: u32 = 0x0080ff;
const TEXT: u32 = 0xffffff;
const DIM: u32 = 0x999999;
const ERROR: u32 = 0xff6666;
const SUCCESS: u32 = 0x66ff66;
const BORDER: u32 = 0x444444;

pub struct TursoApp {
    pub model: AppModel,
    pub path_focus: FocusHandle,
    query_focus: FocusHandle,
    cell_focus: FocusHandle,
    filter_focus: FocusHandle,
    active: ActiveField,
}

impl TursoApp {
    pub fn new(
        database: Option<String>,
        token: Option<String>,
        debug: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            model: AppModel::from_args(database, token, debug),
            path_focus: cx.focus_handle(),
            query_focus: cx.focus_handle(),
            cell_focus: cx.focus_handle(),
            filter_focus: cx.focus_handle(),
            active: ActiveField::Path,
        }
    }

    fn new_db(&mut self) {
        if let Some(path) = FileDialog::new()
            .add_filter("SQLite", &["db", "sqlite"])
            .save_file()
        {
            let _ = self.model.open_path(path.to_string_lossy().to_string());
        }
    }

    fn open_db(&mut self) {
        if let Some(path) = FileDialog::new()
            .add_filter("SQLite", &["db", "sqlite"])
            .pick_file()
        {
            let _ = self.model.open_path(path.to_string_lossy().to_string());
        }
    }

    fn focus_handle_for(&self, field: ActiveField) -> FocusHandle {
        match field {
            ActiveField::Path => self.path_focus.clone(),
            ActiveField::Query => self.query_focus.clone(),
            ActiveField::Cell => self.cell_focus.clone(),
            ActiveField::BrowseFilter(_) | ActiveField::SqlFilter(_) => self.filter_focus.clone(),
            ActiveField::None => self.path_focus.clone(),
        }
    }

    fn buffer_mut(&mut self, field: ActiveField) -> &mut String {
        match field {
            ActiveField::Path => &mut self.model.db_path,
            ActiveField::Query => &mut self.model.query,
            ActiveField::Cell => &mut self.model.cell_editor,
            ActiveField::BrowseFilter(index) => {
                if index >= self.model.browse_state.filters.len() {
                    self.model
                        .browse_state
                        .filters
                        .resize(index + 1, String::new());
                }
                &mut self.model.browse_state.filters[index]
            }
            ActiveField::SqlFilter(index) => {
                if index >= self.model.sql_state.filters.len() {
                    self.model
                        .sql_state
                        .filters
                        .resize(index + 1, String::new());
                }
                &mut self.model.sql_state.filters[index]
            }
            ActiveField::None => &mut self.model.db_path,
        }
    }

    fn submit_field(&mut self, field: ActiveField) {
        match field {
            ActiveField::Path => {
                let _ = self.model.connect();
            }
            ActiveField::Query => {
                let _ = self.model.execute_query();
            }
            ActiveField::BrowseFilter(index) => {
                let value = self
                    .model
                    .browse_state
                    .filters
                    .get(index)
                    .cloned()
                    .unwrap_or_default();
                let _ = self.model.set_browse_filter(index, value);
            }
            ActiveField::SqlFilter(index) => {
                let value = self
                    .model
                    .sql_state
                    .filters
                    .get(index)
                    .cloned()
                    .unwrap_or_default();
                let _ = self.model.set_sql_filter(index, value);
            }
            ActiveField::Cell => {
                self.model.cell_editor.push('\n');
            }
            ActiveField::None => {}
        }
    }

    fn handle_key(&mut self, field: ActiveField, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let effect = {
            let buffer = self.buffer_mut(field);
            apply_keystroke(buffer, ev, cx)
        };
        if effect == KeyEffect::Submit {
            self.submit_field(field);
        }
        cx.notify();
    }

    fn tool_btn(
        &self,
        id: impl Into<ElementId>,
        label: &'static str,
        enabled: bool,
        cx: &mut Context<Self>,
        handler: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .px_3()
            .py_2()
            .rounded_sm()
            .bg(rgb(BG_HEADER))
            .text_color(rgb(if enabled { TEXT } else { DIM }))
            .when(enabled, |this| {
                this.hover(|style| style.bg(rgb(BG_SELECTED)).cursor_pointer())
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |app, _, _, cx| {
                            handler(app, cx);
                            cx.notify();
                        }),
                    )
            })
            .child(label)
    }

    fn tab_btn(&self, tab: Tab, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.model.active_tab == tab;
        div()
            .id(tab.label())
            .px_5()
            .py_2()
            .bg(rgb(if active { BG_SELECTED } else { BG_HEADER }))
            .text_color(rgb(if active { TEXT } else { DIM }))
            .hover(|style| style.bg(rgb(BG_SELECTED)).cursor_pointer())
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.model.switch_tab(tab);
                    this.active = ActiveField::None;
                    cx.notify();
                }),
            )
            .child(tab.label())
    }

    fn render_field(
        &self,
        id: impl Into<ElementId>,
        placeholder: &'static str,
        value: &str,
        field: ActiveField,
        height: Option<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let focus = self.focus_handle_for(field);
        let focused = focus.is_focused(window) && self.active == field;
        let (shown, is_placeholder) = display_value(value, placeholder, focused);
        let lines: Vec<String> = shown.lines().map(|line| line.to_string()).collect();
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };

        let track = !matches!(
            field,
            ActiveField::BrowseFilter(_) | ActiveField::SqlFilter(_)
        ) || self.active == field;

        div()
            .id(id)
            .when(track, |this| this.track_focus(&focus))
            .key_context("Field")
            .w_full()
            .px_2()
            .py_1()
            .rounded_sm()
            .border_1()
            .border_color(rgb(if focused { ACCENT } else { BORDER }))
            .bg(rgb(BG_WINDOW))
            .text_color(rgb(if is_placeholder { DIM } else { TEXT }))
            .cursor(CursorStyle::IBeam)
            .overflow_hidden()
            .when_some(height, |this, h| this.h(h))
            .when(height.is_none(), |this| {
                this.flex_1().min_h(px(80.)).overflow_y_scroll()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.active = field;
                    let handle = this.focus_handle_for(field);
                    window.focus(&handle);
                    window.prevent_default();
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
                this.handle_key(field, ev, cx);
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .children(lines.into_iter().map(|line| {
                        div()
                            .w_full()
                            .whitespace_nowrap()
                            .child(if line.is_empty() {
                                " ".to_string()
                            } else {
                                line
                            })
                    })),
            )
    }

    fn view_status(&self) -> impl IntoElement {
        let (text, color) = match self.model.status_text() {
            Some((msg, StatusKind::Info)) => (msg.to_string(), ACCENT),
            Some((msg, StatusKind::Error)) => (msg.to_string(), ERROR),
            Some((msg, StatusKind::Success)) => (msg.to_string(), SUCCESS),
            None => (String::new(), DIM),
        };

        div()
            .h(px(24.))
            .px_3()
            .py_1()
            .w_full()
            .text_sm()
            .text_color(rgb(color))
            .child(text)
    }

    fn view_connect(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(480.))
                    .gap_4()
                    .p_8()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(TEXT))
                            .child("Connect to Turso / SQLite"),
                    )
                    .child(self.render_field(
                        "field-path",
                        "Database path",
                        &self.model.db_path,
                        ActiveField::Path,
                        Some(px(34.)),
                        window,
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(self.tool_btn("connect", "Connect", true, cx, |this, _| {
                                let _ = this.model.connect();
                            }))
                            .child(self.tool_btn("new-db", "New", true, cx, |this, _| {
                                this.new_db();
                            }))
                            .child(self.tool_btn("open-db", "Open", true, cx, |this, _| {
                                this.open_db();
                            })),
                    )
                    .child(self.view_status()),
            )
    }

    fn view_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_changes = self.model.has_changes;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .w_full()
            .bg(rgb(BG_WINDOW))
            .child(self.tool_btn("new", "New", true, cx, |this, _| this.new_db()))
            .child(self.tool_btn("open", "Open", true, cx, |this, _| this.open_db()))
            .child(self.tool_btn("close", "Close", true, cx, |this, _| {
                this.model.close();
                this.active = ActiveField::Path;
            }))
            .child(
                self.tool_btn("write", "Write", has_changes, cx, |this, _| {
                    let _ = this.model.write_changes();
                }),
            )
            .child(
                self.tool_btn("revert", "Revert", has_changes, cx, |this, _| {
                    let _ = this.model.revert_changes();
                }),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(DIM))
                    .truncate()
                    .child(self.model.db_path.clone()),
            )
    }

    fn view_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .w_full()
            .children(Tab::all().map(|tab| self.tab_btn(tab, cx)))
    }

    fn view_structure(&self, _window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let table_count = self.model.tables.len();
        let selected = self.model.selected_structure_table.clone();
        let columns = self.model.selected_table_columns.clone();
        let schema = self.model.structure_sql().to_string();

        div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(
                div()
                    .w(px(200.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(rgb(BG_CELL))
                    .border_r_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .text_sm()
                            .text_color(rgb(DIM))
                            .child("Tables"),
                    )
                    .child(
                        uniform_list(
                            "structure-tables",
                            table_count,
                            cx.processor(move |this, range: Range<usize>, _, cx| {
                                range
                                    .map(|ix| this.render_structure_table(ix, cx).into_any_element())
                                    .collect()
                            }),
                        )
                        .flex_1()
                        .h_full(),
                    ),
            )
            .child(
                div()
                    .id("structure-details")
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .overflow_scroll()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child(selected.unwrap_or_else(|| "Select a table".into())),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Columns"),
                    )
                    .children(columns.into_iter().map(|col| {
                        div()
                            .flex()
                            .flex_row()
                            .gap_3()
                            .text_sm()
                            .child(
                                div()
                                    .w(px(28.))
                                    .text_color(rgb(ACCENT))
                                    .child(if col.pk { "PK" } else { "" }),
                            )
                            .child(div().w(px(180.)).child(col.name.clone()))
                            .child(
                                div()
                                    .w(px(100.))
                                    .text_color(rgb(DIM))
                                    .child(col.data_type.clone()),
                            )
                            .child(div().text_color(rgb(DIM)).child(if col.not_null {
                                "NOT NULL"
                            } else {
                                ""
                            }))
                    }))
                    .child(div().h(px(1.)).w_full().bg(rgb(BORDER)))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Schema"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(TEXT))
                            .child(if schema.is_empty() {
                                "No schema available".to_string()
                            } else {
                                schema
                            }),
                    ),
            )
    }

    fn render_structure_table(&self, ix: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let name = self
            .model
            .tables
            .get(ix)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        let selected = self.model.selected_structure_table.as_deref() == Some(name.as_str());
        let label = name.clone();
        div()
            .id(("stbl", ix))
            .px_3()
            .py_1()
            .w_full()
            .bg(rgb(if selected { BG_SELECTED } else { BG_CELL }))
            .hover(|style| style.bg(rgb(BG_SELECTED)).cursor_pointer())
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let _ = this.model.select_structure_table(&name);
                    cx.notify();
                }),
            )
            .child(label)
    }

    fn view_browse(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chips: Vec<(usize, String, bool)> = self
            .model
            .table_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                (
                    i,
                    name.clone(),
                    self.model.browse_table.as_deref() == Some(name.as_str()),
                )
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .child(div().text_sm().child("Table:"))
                    .child(
                        div()
                            .id("table-chips")
                            .flex()
                            .flex_row()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .overflow_x_scroll()
                            .children(chips.into_iter().map(|(i, name, selected)| {
                                let click_name = name.clone();
                                div()
                                    .id(("chip", i))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .text_sm()
                                    .bg(rgb(if selected { BG_SELECTED } else { BG_HEADER }))
                                    .hover(|style| style.bg(rgb(BG_SELECTED)).cursor_pointer())
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            let _ = this.model.browse_table_named(&click_name);
                                            cx.notify();
                                        }),
                                    )
                                    .child(name)
                            })),
                    )
                    .child(self.tool_btn("refresh", "Refresh", true, cx, |this, _| {
                        if let Some(name) = this.model.browse_table.clone() {
                            let _ = this.model.browse_table_named(&name);
                        }
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.view_grid(true, window, cx))
                    .child(div().w(px(1.)).h_full().bg(rgb(BORDER)))
                    .child(self.view_editor(window, cx)),
            )
    }

    fn view_sql(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap_2()
                    .p_2()
                    .child(self.render_field(
                        "field-query",
                        "Enter SQL query here",
                        &self.model.query,
                        ActiveField::Query,
                        Some(px(38.)),
                        window,
                        cx,
                    ))
                    .child(self.tool_btn("execute", "Execute", true, cx, |this, _| {
                        let _ = this.model.execute_query();
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.view_grid(false, window, cx))
                    .child(div().w(px(1.)).h_full().bg(rgb(BORDER)))
                    .child(self.view_editor(window, cx)),
            )
    }

    fn view_editor(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w(px(280.))
            .h_full()
            .p_3()
            .gap_2()
            .bg(rgb(BG_CELL))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Cell Editor"),
            )
            .child(self.render_field(
                "field-cell",
                "Select a cell",
                &self.model.cell_editor,
                ActiveField::Cell,
                None,
                window,
                cx,
            ))
    }

    fn view_grid(&self, browse: bool, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = if browse {
            &self.model.browse_state
        } else {
            &self.model.sql_state
        };
        if state.headers.is_empty() {
            return div()
                .id(if browse { "b-empty" } else { "s-empty" })
                .flex()
                .flex_1()
                .min_h_0()
                .items_center()
                .justify_center()
                .text_color(rgb(DIM))
                .child("No data to display")
                .into_any_element();
        }

        let row_count = state.rows.len();
        let list_id = if browse { "browse-rows" } else { "sql-rows" };

        div()
            .id(if browse { "b-grid" } else { "s-grid" })
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(self.view_grid_header(browse, window, cx))
            .child(
                uniform_list(
                    list_id,
                    row_count,
                    cx.processor(move |this, range: Range<usize>, _, cx| {
                        range
                            .map(|ix| this.render_data_row(ix, browse, cx).into_any_element())
                            .collect()
                    }),
                )
                .flex_1()
                .h_full()
                .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained),
            )
            .child(self.view_grid_footer(browse, cx))
            .into_any_element()
    }

    fn view_grid_header(
        &self,
        browse: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = if browse {
            &self.model.browse_state
        } else {
            &self.model.sql_state
        };
        let headers = state.headers.clone();
        let widths = state.widths.clone();
        let filters = state.filters.clone();
        let sort = state.sort;

        div()
            .id(if browse { "b-header" } else { "s-header" })
            .flex()
            .flex_row()
            .flex_shrink_0()
            .overflow_x_scroll()
            .bg(rgb(BG_HEADER))
            .child(
                div()
                    .w(px(44.))
                    .h(px(52.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(rgb(DIM))
                    .flex_shrink_0()
                    .child("#"),
            )
            .children(headers.iter().enumerate().map(|(i, name)| {
                let width = widths.get(i).copied().unwrap_or(150.0);
                let sort_icon = match sort {
                    Some((col, SortDirection::Asc)) if col == i => " ▲",
                    Some((col, SortDirection::Desc)) if col == i => " ▼",
                    _ => "",
                };
                let title = format!("{name}{sort_icon}");
                let filter_val = filters.get(i).cloned().unwrap_or_default();
                let field = if browse {
                    ActiveField::BrowseFilter(i)
                } else {
                    ActiveField::SqlFilter(i)
                };
                let header_id: ElementId = if browse {
                    ("bh", i).into()
                } else {
                    ("sh", i).into()
                };
                let filter_id: ElementId = if browse {
                    ("bf", i).into()
                } else {
                    ("sf", i).into()
                };

                div()
                    .flex()
                    .flex_col()
                    .w(px(width))
                    .flex_shrink_0()
                    .child(
                        div()
                            .id(header_id)
                            .h(px(26.))
                            .px_1()
                            .flex()
                            .items_center()
                            .font_weight(FontWeight::BOLD)
                            .text_xs()
                            .truncate()
                            .hover(|style| style.bg(rgb(BG_SELECTED)).cursor_pointer())
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |app, _, _, cx| {
                                    let _ = app.model.sort_column(i);
                                    cx.notify();
                                }),
                            )
                            .child(title),
                    )
                    .child(self.render_field(
                        filter_id,
                        "Filter (=, >, NULL)…",
                        &filter_val,
                        field,
                        Some(px(22.)),
                        window,
                        cx,
                    ))
            }))
    }

    fn render_data_row(&self, ix: usize, browse: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let state = if browse {
            &self.model.browse_state
        } else {
            &self.model.sql_state
        };
        let row = state.rows.get(ix).cloned().unwrap_or_default();
        let widths = state.widths.clone();
        let selected = state.selected_cell;
        let page = state.page;
        let page_size = state.page_size;
        let alt = ix % 2 == 1;
        let row_num = page * page_size + ix as u32 + 1;
        let prefix = if browse { "br" } else { "sr" };
        let cell_prefix = if browse { "bc" } else { "sc" };

        div()
            .id((prefix, ix))
            .flex()
            .flex_row()
            .h(px(26.))
            .flex_shrink_0()
            .child(
                div()
                    .id((if browse { "bn" } else { "sn" }, ix))
                    .w(px(44.))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(rgb(DIM))
                    .bg(rgb(BG_HEADER))
                    .flex_shrink_0()
                    .hover(|style| style.cursor_pointer())
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.model.select_row(ix);
                            this.active = ActiveField::Cell;
                            window.focus(&this.cell_focus);
                            cx.notify();
                        }),
                    )
                    .child(format!("{row_num}")),
            )
            .children(row.iter().enumerate().map(|(col, value)| {
                let width = widths.get(col).copied().unwrap_or(150.0);
                let is_selected = selected == Some((ix, col));
                let label = value.clone();
                div()
                    .id((cell_prefix, ix.saturating_mul(4096).saturating_add(col)))
                    .w(px(width))
                    .h_full()
                    .px_1()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .text_xs()
                    .truncate()
                    .bg(rgb(if is_selected {
                        BG_SELECTED
                    } else if alt {
                        BG_CELL_ALT
                    } else {
                        BG_CELL
                    }))
                    .hover(|style| style.cursor_pointer())
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.model.select_cell(ix, col);
                            this.active = ActiveField::Cell;
                            window.focus(&this.cell_focus);
                            cx.notify();
                        }),
                    )
                    .child(label)
            }))
    }

    fn view_grid_footer(&self, browse: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let state = if browse {
            &self.model.browse_state
        } else {
            &self.model.sql_state
        };
        let page = state.page;
        let total_pages = state.total_pages();
        let total_records = state.total_records;
        let page_size = state.page_size;
        let all = state.all_results;
        let can_prev = page > 0 && !all;
        let can_next = page + 1 < total_pages && !all;

        div()
            .id(if browse { "b-footer" } else { "s-footer" })
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .bg(rgb(BG_HEADER))
            .text_xs()
            .text_color(rgb(DIM))
            .child(format!("Total Records: {total_records}"))
            .child(
                div()
                    .id(if browse { "b-prev" } else { "s-prev" })
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(BG_CELL))
                    .text_color(rgb(if can_prev { TEXT } else { DIM }))
                    .when(can_prev, |this| {
                        this.hover(|style| style.bg(rgb(BG_SELECTED)).cursor_pointer())
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |app, _, _, cx| {
                                    let _ = app.model.change_page(page.saturating_sub(1));
                                    cx.notify();
                                }),
                            )
                    })
                    .child("Prev"),
            )
            .child(format!("{} / {}", page + 1, total_pages))
            .child(
                div()
                    .id(if browse { "b-next" } else { "s-next" })
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(BG_CELL))
                    .text_color(rgb(if can_next { TEXT } else { DIM }))
                    .when(can_next, |this| {
                        this.hover(|style| style.bg(rgb(BG_SELECTED)).cursor_pointer())
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |app, _, _, cx| {
                                    let _ = app.model.change_page(page + 1);
                                    cx.notify();
                                }),
                            )
                    })
                    .child("Next"),
            )
            .child("Page Size:")
            .children(PAGE_SIZES.iter().copied().map(|size| {
                let active = size == page_size && !all;
                div()
                    .id((if browse { "bps" } else { "sps" }, size as usize))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(if active { BG_SELECTED } else { BG_CELL }))
                    .text_color(rgb(TEXT))
                    .hover(|style| style.bg(rgb(BG_SELECTED)).cursor_pointer())
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            let _ = this.model.change_page_size(size);
                            cx.notify();
                        }),
                    )
                    .child(size.to_string())
            }))
            .child(
                div()
                    .id(if browse { "b-all" } else { "s-all" })
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(if all { BG_SELECTED } else { BG_CELL }))
                    .text_color(rgb(TEXT))
                    .hover(|style| style.bg(rgb(BG_SELECTED)).cursor_pointer())
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            let _ = this.model.toggle_all_results(!all);
                            cx.notify();
                        }),
                    )
                    .child("Return All"),
            )
    }

    fn view_connected(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .child(self.view_toolbar(cx))
            .child(self.view_tabs(cx))
            .child(self.view_status())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(match self.model.active_tab {
                        Tab::Structure => self.view_structure(window, cx).into_any_element(),
                        Tab::BrowseData => self.view_browse(window, cx).into_any_element(),
                        Tab::ExecuteSql => self.view_sql(window, cx).into_any_element(),
                    }),
            )
    }
}

impl Render for TursoApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(BG_WINDOW))
            .text_color(rgb(TEXT))
            .text_sm()
            .when(!self.model.is_connected(), |this| {
                this.child(self.view_connect(window, cx))
            })
            .when(self.model.is_connected(), |this| {
                this.child(self.view_connected(window, cx))
            })
    }
}

impl gpui::Focusable for TursoApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        match self.active {
            ActiveField::Query => self.query_focus.clone(),
            ActiveField::Cell => self.cell_focus.clone(),
            ActiveField::BrowseFilter(_) | ActiveField::SqlFilter(_) => self.filter_focus.clone(),
            ActiveField::Path | ActiveField::None => self.path_focus.clone(),
        }
    }
}
