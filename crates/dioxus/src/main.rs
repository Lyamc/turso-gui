#![windows_subsystem = "windows"]

mod style;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use clap::Parser;
use dioxus::desktop::tao::dpi::{LogicalPosition, LogicalSize};
use dioxus::desktop::tao::window::Theme;
use dioxus::desktop::{Config, WindowBuilder};
use dioxus::prelude::*;
use rfd::FileDialog;
use turso_gui_core::{init_gui_host, AppModel, PAGE_SIZES, StatusKind, Tab, WindowPlacement};

use style::CSS;

type Model = Rc<RefCell<AppModel>>;

static ARGS: OnceLock<Args> = OnceLock::new();

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Turso / SQLite DB Browser (Dioxus)", long_about = None)]
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

fn main() {
    init_gui_host();
    tracing_subscriber::fmt::init();
    let _ = ARGS.set(Args::parse());
    let place = WindowPlacement::suggested();

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::new()
                .with_window(
                    WindowBuilder::new()
                        .with_title("Turso DB Browser (Dioxus)")
                        .with_inner_size(LogicalSize::new(
                            place.logical_width as f64,
                            place.logical_height as f64,
                        ))
                        .with_min_inner_size(LogicalSize::new(
                            place.min_logical_width as f64,
                            place.min_logical_height as f64,
                        ))
                        .with_position(LogicalPosition::new(
                            place.logical_x as f64,
                            place.logical_y as f64,
                        ))
                        .with_theme(Some(Theme::Dark)),
                )
                .with_menu(None)
                .with_background_color((13, 13, 13, 255))
                .with_custom_head(format!("<style>{CSS}</style>")),
        )
        .launch(app);
}

fn app() -> Element {
    let args = ARGS.get().cloned().unwrap_or_else(|| Args {
        database: None,
        token: None,
        debug: false,
        console: false,
    });
    let model = use_context_provider(|| {
        Rc::new(RefCell::new(AppModel::from_args(
            args.database.clone(),
            args.token.clone(),
            args.debug,
        )))
    });
    let tick = use_signal(|| 0u64);
    use_context_provider(|| tick);
    let _ = tick();

    let connected = model.borrow().is_connected();
    let active_tab = model.borrow().active_tab;

    rsx! {
        div { class: "app",
            if connected {
                Toolbar {}
                TabBar {}
                match active_tab {
                    Tab::Structure => rsx! { StructureView {} },
                    Tab::BrowseData => rsx! { BrowseView {} },
                    Tab::ExecuteSql => rsx! { SqlView {} },
                }
                StatusBar {}
            } else {
                ConnectView {}
                StatusBar {}
            }
        }
    }
}

fn sqlite_dialog() -> FileDialog {
    FileDialog::new().add_filter("SQLite", &["db", "sqlite"])
}

fn new_database(model: &Model) {
    if let Some(path) = sqlite_dialog().set_file_name("new.db").save_file() {
        let _ = model.borrow_mut().open_path(path.to_string_lossy().into_owned());
    }
}

fn open_database(model: &Model) {
    if let Some(path) = sqlite_dialog().pick_file() {
        let _ = model.borrow_mut().open_path(path.to_string_lossy().into_owned());
    }
}

fn apply_filter(model: &Model, col: usize) {
    let mut m = model.borrow_mut();
    let value = m.active_grid().filters.get(col).cloned().unwrap_or_default();
    match m.active_tab {
        Tab::ExecuteSql => {
            let _ = m.set_sql_filter(col, value);
        }
        _ => {
            let _ = m.set_browse_filter(col, value);
        }
    }
}

fn set_filter_text(model: &Model, col: usize, value: String) {
    let mut m = model.borrow_mut();
    let filters = match m.active_tab {
        Tab::ExecuteSql => &mut m.sql_state.filters,
        _ => &mut m.browse_state.filters,
    };
    if col >= filters.len() {
        filters.resize(col + 1, String::new());
    }
    filters[col] = value;
}

#[component]
fn ConnectView() -> Element {
    let model = use_context::<Model>();
    let mut tick = use_context::<Signal<u64>>();
    let _ = tick();
    let db_path = model.borrow().db_path.clone();

    rsx! {
        div { class: "connect content",
            h1 { "Connect to Turso / SQLite" }
            form {
                class: "connect-row",
                onsubmit: {
                    let model = model.clone();
                    move |e: FormEvent| {
                        e.prevent_default();
                        let _ = model.borrow_mut().connect();
                        tick += 1;
                    }
                },
                input {
                    value: "{db_path}",
                    placeholder: "Database Path",
                    oninput: {
                        let model = model.clone();
                        move |e: FormEvent| {
                            model.borrow_mut().db_path = e.value();
                        }
                    },
                }
                button { class: "btn primary", r#type: "submit", "Connect" }
            }
            div { class: "connect-row",
                button {
                    class: "btn",
                    r#type: "button",
                    onclick: {
                        let model = model.clone();
                        move |_| {
                            new_database(&model);
                            tick += 1;
                        }
                    },
                    "New Database"
                }
                button {
                    class: "btn",
                    r#type: "button",
                    onclick: {
                        let model = model.clone();
                        move |_| {
                            open_database(&model);
                            tick += 1;
                        }
                    },
                    "Open Database"
                }
            }
        }
    }
}

#[component]
fn Toolbar() -> Element {
    let model = use_context::<Model>();
    let mut tick = use_context::<Signal<u64>>();
    let _ = tick();
    let db_path = model.borrow().db_path.clone();
    let has_changes = model.borrow().has_changes;

    rsx! {
        div { class: "toolbar",
            div { class: "toolbar-left",
                button {
                    class: "btn",
                    onclick: {
                        let model = model.clone();
                        move |_| {
                            new_database(&model);
                            tick += 1;
                        }
                    },
                    "New"
                }
                button {
                    class: "btn",
                    onclick: {
                        let model = model.clone();
                        move |_| {
                            open_database(&model);
                            tick += 1;
                        }
                    },
                    "Open"
                }
                button {
                    class: "btn",
                    onclick: {
                        let model = model.clone();
                        move |_| {
                            model.borrow_mut().close();
                            tick += 1;
                        }
                    },
                    "Close"
                }
                button {
                    class: "btn",
                    disabled: !has_changes,
                    onclick: {
                        let model = model.clone();
                        move |_| {
                            let _ = model.borrow_mut().write_changes();
                            tick += 1;
                        }
                    },
                    "Write"
                }
                button {
                    class: "btn",
                    disabled: !has_changes,
                    onclick: {
                        let model = model.clone();
                        move |_| {
                            let _ = model.borrow_mut().revert_changes();
                            tick += 1;
                        }
                    },
                    "Revert"
                }
            }
            span { class: "path", "{db_path}" }
        }
    }
}

#[component]
fn TabBar() -> Element {
    let model = use_context::<Model>();
    let mut tick = use_context::<Signal<u64>>();
    let _ = tick();
    let active = model.borrow().active_tab;

    rsx! {
        div { class: "tabs",
            for tab in Tab::all() {
                button {
                    key: "{tab.label()}",
                    class: if active == tab { "tab active" } else { "tab" },
                    onclick: {
                        let model = model.clone();
                        move |_| {
                            model.borrow_mut().switch_tab(tab);
                            tick += 1;
                        }
                    },
                    "{tab.label()}"
                }
            }
        }
    }
}

#[component]
fn StatusBar() -> Element {
    let model = use_context::<Model>();
    let tick = use_context::<Signal<u64>>();
    let _ = tick();
    let status = model
        .borrow()
        .status_text()
        .map(|(text, kind)| (text.to_string(), kind));

    let class = match status.as_ref().map(|(_, k)| *k) {
        Some(StatusKind::Error) => "status err",
        Some(StatusKind::Success) => "status ok",
        Some(StatusKind::Info) => "status info",
        None => "status",
    };
    let text = status
        .as_ref()
        .map(|(t, _)| t.as_str())
        .unwrap_or("");

    rsx! {
        div { class: "{class}", "{text}" }
    }
}

#[component]
fn StructureView() -> Element {
    let model = use_context::<Model>();
    let mut tick = use_context::<Signal<u64>>();
    let _ = tick();

    let table_names = model.borrow().table_names.clone();
    let selected = model.borrow().selected_structure_table.clone();
    let columns = model.borrow().selected_table_columns.clone();
    let sql = model.borrow().structure_sql().to_string();

    rsx! {
        div { class: "content",
            div { class: "sidebar",
                h2 { "Tables" }
                div { class: "list",
                    if table_names.is_empty() {
                        div { class: "muted", "No tables" }
                    }
                    for name in table_names.iter().cloned() {
                        {
                            let active = selected.as_deref() == Some(name.as_str());
                            let label = name.clone();
                            let model = model.clone();
                            rsx! {
                                button {
                                    key: "{label}",
                                    class: if active { "list-item active" } else { "list-item" },
                                    onclick: move |_| {
                                        let _ = model.borrow_mut().select_structure_table(&name);
                                        tick += 1;
                                    },
                                    "{label}"
                                }
                            }
                        }
                    }
                }
            }
            div { class: "panel",
                h2 { "Columns" }
                table { class: "cols",
                    thead {
                        tr {
                            th { "PK" }
                            th { "Name" }
                            th { "Type" }
                            th { "NOT NULL" }
                        }
                    }
                    tbody {
                        for col in columns.iter() {
                            tr {
                                key: "{col.name}",
                                td { if col.pk { "🔑" } }
                                td { "{col.name}" }
                                td { class: "muted", "{col.data_type}" }
                                td { if col.not_null { "NOT NULL" } }
                            }
                        }
                    }
                }
                h2 { "Schema" }
                pre { class: "schema", "{sql}" }
            }
        }
    }
}

#[component]
fn CellEditor() -> Element {
    let model = use_context::<Model>();
    let tick = use_context::<Signal<u64>>();
    let _ = tick();
    let value = model.borrow().cell_editor.clone();

    rsx! {
        div { class: "editor-pane",
            h2 { "Cell Editor" }
            textarea {
                class: "cell-editor",
                value: "{value}",
                oninput: {
                    let model = model.clone();
                    move |e: FormEvent| {
                        model.borrow_mut().cell_editor = e.value();
                    }
                },
            }
        }
    }
}

#[component]
fn BrowseView() -> Element {
    let model = use_context::<Model>();
    let mut tick = use_context::<Signal<u64>>();
    let _ = tick();

    let table_names = model.borrow().table_names.clone();
    let browse_table = model.borrow().browse_table.clone();

    rsx! {
        div { class: "content", style: "flex-direction: column;",
            div { class: "browse-bar",
                span { "Table:" }
                select {
                    onchange: {
                        let model = model.clone();
                        move |e: FormEvent| {
                            let name = e.value();
                            if !name.is_empty() {
                                let _ = model.borrow_mut().browse_table_named(&name);
                                tick += 1;
                            }
                        }
                    },
                    option {
                        value: "",
                        disabled: true,
                        selected: browse_table.is_none(),
                        "Select table"
                    }
                    for name in table_names.iter() {
                        option {
                            key: "{name}",
                            value: "{name}",
                            selected: browse_table.as_deref() == Some(name.as_str()),
                            "{name}"
                        }
                    }
                }
                button {
                    class: "btn",
                    onclick: {
                        let model = model.clone();
                        move |_| {
                            let _ = model.borrow_mut().load_browse_data();
                            tick += 1;
                        }
                    },
                    "Refresh"
                }
            }
            div { class: "split",
                DataGrid { sortable: true }
                CellEditor {}
            }
        }
    }
}

#[component]
fn SqlView() -> Element {
    let model = use_context::<Model>();
    let mut tick = use_context::<Signal<u64>>();
    let _ = tick();
    let query = model.borrow().query.clone();

    rsx! {
        div { class: "content", style: "flex-direction: column;",
            div { class: "sql-bar",
                textarea {
                    class: "sql-input",
                    value: "{query}",
                    placeholder: "Enter SQL query here",
                    oninput: {
                        let model = model.clone();
                        move |e: FormEvent| {
                            model.borrow_mut().query = e.value();
                        }
                    },
                    onkeydown: {
                        let model = model.clone();
                        move |e: KeyboardEvent| {
                            if e.key() == Key::Enter && (e.modifiers().ctrl() || e.modifiers().meta()) {
                                e.prevent_default();
                                let _ = model.borrow_mut().execute_query();
                                tick += 1;
                            }
                        }
                    },
                }
                button {
                    class: "btn primary sql-exec",
                    title: "Ctrl+Enter",
                    onclick: {
                        let model = model.clone();
                        move |_| {
                            let _ = model.borrow_mut().execute_query();
                            tick += 1;
                        }
                    },
                    "Execute"
                }
            }
            div { class: "split",
                DataGrid { sortable: true }
                CellEditor {}
            }
        }
    }
}

#[component]
fn DataGrid(sortable: bool) -> Element {
    let model = use_context::<Model>();
    let mut tick = use_context::<Signal<u64>>();
    let _ = tick();

    let grid = model.borrow().active_grid().clone();
    let headers = grid.headers.clone();
    let rows = grid.rows.clone();
    let filters = grid.filters.clone();
    let widths = grid.widths.clone();
    let selected_cell = grid.selected_cell;
    let sort = grid.sort;
    let empty = headers.is_empty();

    rsx! {
        div { class: "main-col",
            div { class: "grid-wrap",
                if empty {
                    div { class: "empty", "No data to display" }
                } else {
                    table { class: "grid",
                        thead {
                            tr {
                                th { class: "row-num", "#" }
                                for (i, name) in headers.iter().enumerate() {
                                    {
                                        let mut title = name.clone();
                                        if let Some((col, dir)) = sort {
                                            if sortable && col == i {
                                                title.push(' ');
                                                title.push_str(dir.icon());
                                            }
                                        }
                                        let filter = filters.get(i).cloned().unwrap_or_default();
                                        let width = widths.get(i).copied().unwrap_or(150.0);
                                        let model_sort = model.clone();
                                        let model_input = model.clone();
                                        let model_change = model.clone();
                                        let model_key = model.clone();
                                        rsx! {
                                            th {
                                                key: "{name}",
                                                style: "min-width: {width}px",
                                                button {
                                                    class: "sort",
                                                    onclick: move |_| {
                                                        if sortable {
                                                            let _ = model_sort.borrow_mut().sort_column(i);
                                                            tick += 1;
                                                        }
                                                    },
                                                    "{title}"
                                                }
                                                input {
                                                    class: "filter",
                                                    value: "{filter}",
                                                    placeholder: "Filter (=, >, NULL)…",
                                                    oninput: move |e: FormEvent| {
                                                        set_filter_text(&model_input, i, e.value());
                                                    },
                                                    onchange: move |_| {
                                                        apply_filter(&model_change, i);
                                                        tick += 1;
                                                    },
                                                    onkeydown: move |e: KeyboardEvent| {
                                                        if e.key() == Key::Enter {
                                                            apply_filter(&model_key, i);
                                                            tick += 1;
                                                        }
                                                    },
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        tbody {
                            for (ri, row) in rows.iter().enumerate() {
                                {
                                    let row_num = if grid.all_results {
                                        ri as u32 + 1
                                    } else {
                                        grid.page
                                            .saturating_mul(grid.page_size)
                                            .saturating_add(ri as u32)
                                            .saturating_add(1)
                                    };
                                    let alt = ri % 2 == 1;
                                    let model_row = model.clone();
                                    rsx! {
                                        tr {
                                            key: "{ri}",
                                            class: if alt { "alt" } else { "" },
                                            td {
                                                class: "row-num",
                                                onclick: {
                                                    let model = model_row.clone();
                                                    move |_| {
                                                        model.borrow_mut().select_row(ri);
                                                        tick += 1;
                                                    }
                                                },
                                                "{row_num}"
                                            }
                                            for (ci, cell) in row.iter().enumerate() {
                                                td {
                                                    key: "{ci}",
                                                    class: if selected_cell == Some((ri, ci)) { "sel" } else { "" },
                                                    title: "{cell}",
                                                    onclick: {
                                                        let model = model_row.clone();
                                                        move |_| {
                                                            model.borrow_mut().select_cell(ri, ci);
                                                            tick += 1;
                                                        }
                                                    },
                                                    "{cell}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Pager {}
        }
    }
}

#[component]
fn Pager() -> Element {
    let model = use_context::<Model>();
    let mut tick = use_context::<Signal<u64>>();
    let _ = tick();

    let grid = model.borrow().active_grid().clone();
    let page = grid.page;
    let total_pages = grid.total_pages();
    let page_size = grid.page_size;
    let total_records = grid.total_records;
    let all_results = grid.all_results;
    let display_page = page.saturating_add(1);

    rsx! {
        div { class: "pager",
            span { "{total_records} records" }
            button {
                class: "btn",
                disabled: page == 0 || all_results,
                onclick: {
                    let model = model.clone();
                    move |_| {
                        let _ = model.borrow_mut().change_page(page.saturating_sub(1));
                        tick += 1;
                    }
                },
                "Prev"
            }
            span { "Page {display_page} of {total_pages}" }
            button {
                class: "btn",
                disabled: display_page >= total_pages || all_results,
                onclick: {
                    let model = model.clone();
                    move |_| {
                        let _ = model.borrow_mut().change_page(page.saturating_add(1));
                        tick += 1;
                    }
                },
                "Next"
            }
            span { "Page size:" }
            select {
                disabled: all_results,
                onchange: {
                    let model = model.clone();
                    move |e: FormEvent| {
                        if let Ok(size) = e.value().parse::<u32>() {
                            let _ = model.borrow_mut().change_page_size(size);
                            tick += 1;
                        }
                    }
                },
                for size in PAGE_SIZES {
                    option {
                        key: "{size}",
                        value: "{size}",
                        selected: page_size == size,
                        "{size}"
                    }
                }
            }
            label {
                input {
                    r#type: "checkbox",
                    checked: all_results,
                    onchange: {
                        let model = model.clone();
                        move |_| {
                            let _ = model.borrow_mut().toggle_all_results(!all_results);
                            tick += 1;
                        }
                    },
                }
                "Return All"
            }
        }
    }
}
