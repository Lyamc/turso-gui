#![windows_subsystem = "windows"]

use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use clap::Parser;
use rstk::{
    self, PackFill, PackSide, Selection, State, Sticky, TkButton, TkCheckButton, TkCombobox,
    TkEntry, TkFrame, TkGridLayout, TkLabel, TkLabelOptions, TkListbox, TkNotebook, TkPackLayout,
    TkText,
    TkTopLevel, TkTreeview, TkWidget,
};
use turso_gui_core::{
    init_gui_host, report_error, AppModel, GridState, SortDirection, StatusKind, Tab, PAGE_SIZES,
};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Turso / SQLite DB Browser (Tcl/Tk)", long_about = None)]
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

struct Widgets {
    path: TkEntry,
    status: TkLabel,
    write_btn: TkButton,
    revert_btn: TkButton,
    tables: TkListbox,
    columns: TkTreeview,
    schema: TkText,
    table_combo: TkCombobox,
    browse_tree: TkTreeview,
    browse_filter_frame: TkFrame,
    browse_filter_host: TkLabel,
    sql_query: TkText,
    sql_tree: TkTreeview,
    sql_filter_frame: TkFrame,
    sql_filter_host: TkLabel,
    cell: TkText,
    sql_cell: TkText,
    records: TkLabel,
    page: TkLabel,
    page_size: TkCombobox,
    all: TkCheckButton,
    notebook: TkNotebook,
}

type SharedModel = Arc<Mutex<AppModel>>;
type SharedUi = Arc<Widgets>;

fn tcl_brace(s: &str) -> String {
    format!(
        "{{{}}}",
        s.replace('\\', "\\\\").replace('{', "\\{").replace('}', "\\}")
    )
}

fn tell(cmd: &str) {
    rstk::tell_wish(cmd);
}

/// Evaluate a Tcl command and return its result.
///
/// rstk's `ask_wish` reads wish stdout, so the command must `puts` a line.
/// Callers must not use combobox `value()` (that crate method `puts` via
/// `tell_wish`, which desynchronizes the event stream).
fn tcl_eval(cmd: &str) -> String {
    let wrapped = format!(
        "if {{[catch {{{}}} __r]}} {{puts {{}}}} else {{puts $__r}} ; flush stdout",
        cmd
    );
    let mut s = rstk::ask_wish(&wrapped);
    while s.ends_with('\n') || s.ends_with('\r') || s.ends_with('\u{0}') {
        s.pop();
    }
    s.trim().to_string()
}

fn combo_set(cb: &TkCombobox, value: &str) {
    tell(&format!("{} set {}", cb.id(), tcl_brace(value)));
}

fn combo_get(cb: &TkCombobox) -> String {
    tcl_eval(&format!("{} get", cb.id()))
}

fn entry_set(entry: &TkEntry, value: &str) {
    tell(&format!("{} delete 0 end", entry.id()));
    tell(&format!("{} insert 0 {}", entry.id(), tcl_brace(value)));
}

fn text_set(text: &TkText, value: &str) {
    tell(&format!("{} delete 1.0 end", text.id()));
    if !value.is_empty() {
        tell(&format!("{} insert 1.0 {}", text.id(), tcl_brace(value)));
    }
}

fn text_get(text: &TkText) -> String {
    tcl_eval(&format!("{} get 1.0 end-1c", text.id()))
}

fn attach_scrollbars(tree: &TkTreeview, holder: &impl TkWidget) {
    let tv = tree.id();
    let f = holder.id();
    tell(&format!(
        r#"
        catch {{pack forget {tv}}}
        catch {{grid forget {tv}}}
        if {{[winfo exists {tv}_sy]}} {{destroy {tv}_sy}}
        if {{[winfo exists {tv}_sx]}} {{destroy {tv}_sx}}
        ttk::scrollbar {tv}_sy -orient vertical -command [list {tv} yview]
        ttk::scrollbar {tv}_sx -orient horizontal -command [list {tv} xview]
        {tv} configure -yscrollcommand [list {tv}_sy set] -xscrollcommand [list {tv}_sx set]
        grid {tv} -in {f} -row 0 -column 0 -sticky nsew
        grid {tv}_sy -in {f} -row 0 -column 1 -sticky ns
        grid {tv}_sx -in {f} -row 1 -column 0 -sticky ew
        grid rowconfigure {f} 0 -weight 1
        grid columnconfigure {f} 0 -weight 1
        "#
    ));
}

fn attach_listbox_scroll(lb: &TkListbox, holder: &impl TkWidget) {
    let id = lb.id();
    let f = holder.id();
    tell(&format!(
        r#"
        catch {{pack forget {id}}}
        ttk::scrollbar {id}_sy -orient vertical -command [list {id} yview]
        {id} configure -yscrollcommand [list {id}_sy set]
        pack {id} -in {f} -side left -fill both -expand 1
        pack {id}_sy -in {f} -side right -fill y
        "#
    ));
}

fn listbox_clear(lb: &TkListbox) {
    tell(&format!("{} delete 0 end", lb.id()));
}

fn combo_values(cb: &TkCombobox, items: &[String]) {
    let list = items.iter().map(|s| tcl_brace(s)).collect::<Vec<_>>().join(" ");
    tell(&format!("{} configure -values [list {}]", cb.id(), list));
}

fn tree_clear(tree: &TkTreeview) {
    tell(&format!("{} delete [{} children {{}}]", tree.id(), tree.id()));
}

fn heading_title(name: &str, index: usize, sort: Option<(usize, SortDirection)>) -> String {
    match sort {
        Some((col, dir)) if col == index => format!("{} {}", name, dir.icon()),
        _ => name.to_string(),
    }
}

fn tree_setup(tree: &TkTreeview, headers: &[String], sort: Option<(usize, SortDirection)>) {
    tree_clear(tree);
    if headers.is_empty() {
        tree.columns(&["empty"]);
        tree.heading_text("empty", "(no columns)");
        tree.show_headings();
        return;
    }
    let names: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, _)| format!("c{i}"))
        .collect();
    let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    tree.columns(&name_refs);
    tree.show_headings();
    tell(&format!(
        "{} column #0 -width 0 -stretch 0 -minwidth 0",
        tree.id()
    ));
    let last = headers.len().saturating_sub(1);
    for (i, header) in headers.iter().enumerate() {
        let col = format!("c{i}");
        tree.heading_text(&col, &heading_title(header, i, sort));
        tree.column_stretch(&col, i == last);
        tree.column_min_width(&col, 48);
        let width = (header.len() as u64 * 9 + 48).clamp(72, 360);
        tree.column_width(&col, width);
    }
}

fn tree_fill(tree: &TkTreeview, grid: &GridState) {
    tree_setup(tree, &grid.headers, grid.sort);
    let id = tree.id();
    let mut script = String::new();
    for row in &grid.rows {
        script.push_str(&format!("{id} insert {{}} end -values [list"));
        for cell in row {
            script.push(' ');
            script.push_str(&tcl_brace(cell));
        }
        script.push_str("]\n");
    }
    if !script.is_empty() {
        tell(&script);
    }
}

fn rebuild_filter_row(
    frame: &TkFrame,
    host: &TkLabel,
    tree: &TkTreeview,
    grid: &GridState,
    prefix: &str,
) {
    let fid = frame.id();
    let tv = tree.id();
    let host_pat = format!("{}<Return>", host.id());
    tell(&format!(
        "if {{[winfo exists {fid}]}} {{ foreach w [winfo children {fid}] {{ destroy $w }} }}"
    ));
    if grid.headers.is_empty() {
        return;
    }
    let mut script = String::new();
    for (i, _) in grid.headers.iter().enumerate() {
        let val = tcl_brace(grid.filters.get(i).map(|s| s.as_str()).unwrap_or(""));
        script.push_str(&format!(
            r#"
            ttk::entry {fid}.e{i} -textvariable ::tf_{prefix}_{i}
            set ::tf_{prefix}_{i} {val}
            set cw 72
            catch {{ set cw [{tv} column c{i} -width] }}
            if {{$cw < 56}} {{ set cw 56 }}
            grid {fid}.e{i} -in {fid} -row 0 -column {i} -sticky nsew -padx 1 -pady 1
            grid columnconfigure {fid} {i} -minsize $cw -weight 0
            bind {fid}.e{i} <Return> {{ after cancel $::turso_ff ; puts cb1e:{host_pat}:0:0:0:0:0:0:0:Return:0 ; flush stdout }}
            bind {fid}.e{i} <KeyRelease> {{
                after cancel $::turso_ff
                set ::turso_ff [after 400 {{ puts cb1e:{host_pat}:0:0:0:0:0:0:0:Return:0 ; flush stdout }}]
            }}
            "#
        ));
    }
    tell("if {![info exists ::turso_ff]} { set ::turso_ff {} }");
    tell(&script);
}

fn sync_filter_widths(frame: &TkFrame, tree: &TkTreeview, n: usize) {
    if n == 0 {
        return;
    }
    let fid = frame.id();
    let tv = tree.id();
    let mut script = String::new();
    for i in 0..n {
        script.push_str(&format!(
            "if {{[winfo exists {fid}.e{i}]}} {{ set cw 72; catch {{ set cw [{tv} column c{i} -width] }}; if {{$cw < 56}} {{ set cw 56 }}; grid columnconfigure {fid} {i} -minsize $cw }}\n"
        ));
    }
    tell(&script);
}

fn read_filter_vars(prefix: &str, n: usize) -> Vec<String> {
    (0..n)
        .map(|i| tcl_eval(&format!("set ::tf_{prefix}_{i}")))
        .collect()
}

fn apply_column_filters(ui: &Widgets, model: &SharedModel, sql: bool) {
    let n = {
        let m = model.lock().expect("model lock");
        if sql {
            m.sql_state.headers.len()
        } else {
            m.browse_state.headers.len()
        }
    };
    let prefix = if sql { "sql" } else { "br" };
    let vals = read_filter_vars(prefix, n);
    with_model(model, |m| {
        if sql {
            m.sql_state.filters = vals;
            m.sql_state.ensure_filters();
            m.sql_state.page = 0;
            let _ = m.execute_query();
        } else {
            m.browse_state.filters = vals;
            m.browse_state.ensure_filters();
            m.browse_state.page = 0;
            let _ = m.load_browse_data();
        }
    });
    let m = model.lock().expect("model lock");
    if sql {
        refresh_sql(ui, &m, false);
    } else {
        refresh_browse(ui, &m, false);
    }
    set_status(ui, &m);
}

fn clear_column_filters(ui: &Widgets, model: &SharedModel, sql: bool) {
    with_model(model, |m| {
        if sql {
            m.sql_state.filters.clear();
            m.sql_state.page = 0;
            let _ = m.execute_query();
        } else {
            m.browse_state.filters.clear();
            m.browse_state.page = 0;
            let _ = m.load_browse_data();
        }
    });
    let m = model.lock().expect("model lock");
    if sql {
        refresh_sql(ui, &m, true);
    } else {
        refresh_browse(ui, &m, true);
    }
    set_status(ui, &m);
}

fn wish_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(local) = env::var("LOCALAPPDATA") {
        let bin = Path::new(&local).join("Apps").join("Tcl86").join("bin");
        out.push(bin.join("wish.exe"));
        out.push(bin.join("wish86t.exe"));
    }
    if let Ok(path) = env::var("PATH") {
        for dir in env::split_paths(&path) {
            out.push(dir.join("wish.exe"));
            out.push(dir.join("wish86t.exe"));
            out.push(dir.join("wish86.exe"));
        }
    }
    out
}

fn start_wish() -> Result<TkTopLevel> {
    // rstk can only be started once (internal OnceLock), so pick a real
    // executable before calling into it.
    let path = wish_candidates().into_iter().find(|p| p.is_file());
    let Some(path) = path else {
        let msg = "Failed to start wish. Install Tcl/Tk and ensure wish.exe is on PATH \
(for example %LOCALAPPDATA%\\Apps\\Tcl86\\bin).";
        report_error("Turso DB Browser (Tcl/Tk)", msg);
        return Err(anyhow!(msg));
    };
    rstk::start_with(&path.to_string_lossy()).map_err(|e| {
        let msg = format!("Failed to start {}: {e:?}", path.display());
        report_error("Turso DB Browser (Tcl/Tk)", &msg);
        anyhow!(msg)
    })
}

fn place_root_window() {
    // Tk has its own coordinate system (and DPI scaling). Center with Tcl
    // screen metrics instead of Windows physical pixels, then bring the
    // window to the front.
    tell(
        r#"
        update idletasks
        set sw [winfo screenwidth .]
        set sh [winfo screenheight .]
        set w 1280
        set h 800
        if {$w > $sw - 48} { set w [expr {$sw - 48}] }
        if {$h > $sh - 48} { set h [expr {$sh - 48}] }
        if {$w < 400} { set w $sw }
        if {$h < 300} { set h $sh }
        set x [expr {int(($sw - $w) / 2)}]
        set y [expr {int(($sh - $h) / 2)}]
        if {$x < 0} { set x 0 }
        if {$y < 0} { set y 0 }
        wm geometry . ${w}x${h}+${x}+${y}
        set mw $w
        set mh $h
        if {$mw > 800} { set mw 800 }
        if {$mh > 500} { set mh 500 }
        wm minsize . $mw $mh
        wm deiconify .
        raise .
        focus -force .
        wm attributes . -topmost 1
        update
        wm attributes . -topmost 0
        "#,
    );
}

fn with_model<R>(model: &SharedModel, f: impl FnOnce(&mut AppModel) -> R) -> R {
    let mut guard = model.lock().expect("model lock");
    f(&mut guard)
}

fn set_write_state(ui: &Widgets, model: &AppModel) {
    let state = if model.has_changes {
        State::Normal
    } else {
        State::Disabled
    };
    ui.write_btn.state(state.clone());
    ui.revert_btn.state(state);
}

fn set_status(ui: &Widgets, model: &AppModel) {
    match model.status_text() {
        Some((msg, StatusKind::Error)) => {
            ui.status.foreground("#cc3333");
            ui.status.text(msg);
        }
        Some((msg, StatusKind::Success)) => {
            ui.status.foreground("#2a7a2a");
            ui.status.text(msg);
        }
        Some((msg, StatusKind::Info)) => {
            ui.status.foreground("#1a5fb4");
            ui.status.text(msg);
        }
        None => {
            ui.status.foreground("#444444");
            let path = if model.is_connected() {
                format!("Connected: {}", model.db_path)
            } else {
                "Not connected".into()
            };
            ui.status.text(&path);
        }
    }
}

fn refresh_structure(ui: &Widgets, model: &AppModel) {
    listbox_clear(&ui.tables);
    for name in &model.table_names {
        ui.tables.append(name);
    }
    tree_setup(
        &ui.columns,
        &["PK".into(), "Name".into(), "Type".into(), "NOT NULL".into()],
        None,
    );
    let mut script = String::new();
    for col in &model.selected_table_columns {
        script.push_str(&format!(
            "{} insert {{}} end -values [list {} {} {} {}]\n",
            ui.columns.id(),
            tcl_brace(if col.pk { "PK" } else { "" }),
            tcl_brace(&col.name),
            tcl_brace(&col.data_type),
            tcl_brace(if col.not_null { "NOT NULL" } else { "" }),
        ));
    }
    if !script.is_empty() {
        tell(&script);
    }
    text_set(&ui.schema, model.structure_sql());
}

fn refresh_browse(ui: &Widgets, model: &AppModel, rebuild_filters: bool) {
    combo_values(&ui.table_combo, &model.table_names);
    if let Some(name) = &model.browse_table {
        combo_set(&ui.table_combo, name);
    } else {
        combo_set(&ui.table_combo, "");
    }
    tree_fill(&ui.browse_tree, &model.browse_state);
    if rebuild_filters {
        rebuild_filter_row(
            &ui.browse_filter_frame,
            &ui.browse_filter_host,
            &ui.browse_tree,
            &model.browse_state,
            "br",
        );
    } else {
        sync_filter_widths(
            &ui.browse_filter_frame,
            &ui.browse_tree,
            model.browse_state.headers.len(),
        );
    }
    refresh_pager(ui, model.active_grid());
}

fn refresh_sql(ui: &Widgets, model: &AppModel, rebuild_filters: bool) {
    tree_fill(&ui.sql_tree, &model.sql_state);
    if rebuild_filters {
        rebuild_filter_row(
            &ui.sql_filter_frame,
            &ui.sql_filter_host,
            &ui.sql_tree,
            &model.sql_state,
            "sql",
        );
    } else {
        sync_filter_widths(
            &ui.sql_filter_frame,
            &ui.sql_tree,
            model.sql_state.headers.len(),
        );
    }
    refresh_pager(ui, model.active_grid());
}

fn refresh_grid(ui: &Widgets, model: &AppModel, rebuild_filters: bool) {
    match model.active_tab {
        Tab::ExecuteSql => refresh_sql(ui, model, rebuild_filters),
        Tab::BrowseData => refresh_browse(ui, model, rebuild_filters),
        Tab::Structure => refresh_structure(ui, model),
    }
}

fn refresh_pager(ui: &Widgets, grid: &GridState) {
    let total_pages = grid.total_pages();
    ui.records.text(&format!(
        "Records: {}   Page {} / {}",
        grid.total_records,
        grid.page + 1,
        total_pages
    ));
    ui.page.text(&format!("{}", grid.page + 1));
    combo_set(&ui.page_size, &grid.page_size.to_string());
    ui.all.selected(grid.all_results);
}

fn refresh_all(ui: &Widgets, model: &AppModel) {
    entry_set(&ui.path, &model.db_path);
    set_write_state(ui, model);
    set_status(ui, model);
    if model.is_connected() {
        refresh_structure(ui, model);
        refresh_browse(ui, model, true);
        refresh_sql(ui, model, true);
        text_set(&ui.cell, &model.cell_editor);
        text_set(&ui.sql_cell, &model.cell_editor);
    }
}

fn active_grid<'a>(model: &'a AppModel) -> &'a GridState {
    match model.active_tab {
        Tab::ExecuteSql => &model.sql_state,
        _ => &model.browse_state,
    }
}

fn select_from_tree(ui: &Widgets, model: &SharedModel, tree: &TkTreeview, ev_x: i64, ev_y: i64) {
    let region = tcl_eval(&format!("{} identify region {} {}", tree.id(), ev_x, ev_y));
    if region == "heading" {
        let col = tcl_eval(&format!("{} identify column {} {}", tree.id(), ev_x, ev_y));
        if let Some(idx) = parse_col_index(&col) {
            with_model(model, |m| {
                let _ = m.sort_column(idx);
            });
            let m = model.lock().expect("model lock");
            refresh_grid(ui, &m, false);
            set_status(ui, &m);
        }
        return;
    }
    let item = tcl_eval(&format!("{} identify item {} {}", tree.id(), ev_x, ev_y));
    if item.is_empty() {
        return;
    }
    let children = tcl_eval(&format!("{} children {{}}", tree.id()));
    let row = children
        .split_whitespace()
        .position(|id| id == item)
        .unwrap_or(0);
    let col = parse_col_index(
        &tcl_eval(&format!("{} identify column {} {}", tree.id(), ev_x, ev_y)),
    )
    .unwrap_or(0);
    with_model(model, |m| m.select_cell(row, col));
    let m = model.lock().expect("model lock");
    text_set(&ui.cell, &m.cell_editor);
    text_set(&ui.sql_cell, &m.cell_editor);
}

fn open_table_in_browse(ui: &Widgets, model: &SharedModel, name: &str) {
    if name.is_empty() {
        return;
    }
    with_model(model, |m| {
        let _ = m.browse_table_named(name);
        m.switch_tab(Tab::BrowseData);
    });
    tell(&format!("{} select 1", ui.notebook.id()));
    let m = model.lock().expect("model lock");
    refresh_browse(ui, &m, true);
    set_status(ui, &m);
}

fn parse_col_index(col: &str) -> Option<usize> {
    let s = col.trim();
    if let Some(rest) = s.strip_prefix('#') {
        return rest.parse::<usize>().ok().map(|n| n.saturating_sub(1));
    }
    if let Some(rest) = s.strip_prefix('c') {
        return rest.parse().ok();
    }
    None
}

fn do_connect(ui: &Widgets, model: &SharedModel) {
    let path = ui.path.value_get();
    with_model(model, |m| {
        m.db_path = path;
        let _ = m.connect();
    });
    let m = model.lock().expect("model lock");
    refresh_all(ui, &m);
}

fn do_open(ui: &Widgets, model: &SharedModel) {
    if let Some(path) = rstk::open_file_chooser()
        .title("Open SQLite database")
        .file_types(&[("SQLite", ".db .sqlite .sqlite3"), ("All files", "*")])
        .show()
    {
        with_model(model, |m| {
            let _ = m.open_path(path);
        });
        let m = model.lock().expect("model lock");
        refresh_all(ui, &m);
    }
}

fn do_new(ui: &Widgets, model: &SharedModel) {
    if let Some(path) = rstk::save_file_chooser()
        .title("New SQLite database")
        .file_types(&[("SQLite", ".db .sqlite"), ("All files", "*")])
        .show()
    {
        with_model(model, |m| {
            let _ = m.open_path(path);
        });
        let m = model.lock().expect("model lock");
        refresh_all(ui, &m);
    }
}

fn build_ui(root: &TkTopLevel, model: SharedModel) -> SharedUi {
    tell("wm title . {Turso DB Browser (Tcl/Tk)}");
    tell("set ::turso_ff {}");
    place_root_window();

    let themes = rstk::theme_names();
    if themes.iter().any(|t| t == "vista") {
        rstk::use_theme("vista");
    } else if themes.iter().any(|t| t == "clam") {
        rstk::use_theme("clam");
    }

    let toolbar = rstk::make_frame(root);
    toolbar
        .grid()
        .row(0)
        .column(0)
        .sticky(Sticky::EW)
        .padx(4)
        .pady(4)
        .layout();

    let notebook = rstk::make_notebook(root);
    notebook
        .grid()
        .row(1)
        .column(0)
        .sticky(Sticky::NESW)
        .padx(4)
        .pady(2)
        .layout();

    let status = rstk::make_label(root);
    status
        .grid()
        .row(2)
        .column(0)
        .sticky(Sticky::EW)
        .padx(8)
        .pady(4)
        .layout();
    root.grid_configure_column(0, "weight", "1");
    root.grid_configure_row(1, "weight", "1");

    let btn_new = rstk::make_button(&toolbar);
    btn_new.text("New");
    btn_new.pack().side(PackSide::Left).padx(2).layout();
    let btn_open = rstk::make_button(&toolbar);
    btn_open.text("Open");
    btn_open.pack().side(PackSide::Left).padx(2).layout();
    let btn_close = rstk::make_button(&toolbar);
    btn_close.text("Close");
    btn_close.pack().side(PackSide::Left).padx(2).layout();
    let btn_write = rstk::make_button(&toolbar);
    btn_write.text("Write");
    btn_write.pack().side(PackSide::Left).padx(2).layout();
    let btn_revert = rstk::make_button(&toolbar);
    btn_revert.text("Revert");
    btn_revert.pack().side(PackSide::Left).padx(2).layout();

    let path = rstk::make_entry(&toolbar);
    path.width(40);
    path.pack()
        .side(PackSide::Left)
        .fill(PackFill::X)
        .expand(true)
        .padx(8)
        .layout();
    let btn_connect = rstk::make_button(&toolbar);
    btn_connect.text("Connect");
    btn_connect.pack().side(PackSide::Left).padx(2).layout();

    let tab_structure = rstk::make_frame(&notebook);
    let tab_browse = rstk::make_frame(&notebook);
    let tab_sql = rstk::make_frame(&notebook);
    notebook.add(&tab_structure, "Structure");
    notebook.add(&tab_browse, "Browse");
    notebook.add(&tab_sql, "SQL");

    let tables_wrap = rstk::make_frame(&tab_structure);
    tables_wrap
        .pack()
        .side(PackSide::Left)
        .fill(PackFill::Y)
        .padx(4)
        .pady(4)
        .layout();
    let tables_hdr = rstk::make_label(&tables_wrap);
    tables_hdr.text("Tables");
    tables_hdr.pack().anchor(rstk::Anchor::W).layout();
    let tables_body = rstk::make_frame(&tables_wrap);
    tables_body
        .pack()
        .fill(PackFill::Both)
        .expand(true)
        .layout();
    let tables = rstk::make_listbox(&tables_body, &[]);
    tables.width(28);
    tables.height(22);
    tables.selection_mode(Selection::Single);
    attach_listbox_scroll(&tables, &tables_body);
    let btn_browse_table = rstk::make_button(&tables_wrap);
    btn_browse_table.text("Browse selected");
    btn_browse_table.pack().fill(PackFill::X).pady(4).layout();

    let struct_right = rstk::make_frame(&tab_structure);
    struct_right
        .pack()
        .side(PackSide::Left)
        .fill(PackFill::Both)
        .expand(true)
        .layout();
    let col_label = rstk::make_label(&struct_right);
    col_label.text("Columns");
    col_label.pack().anchor(rstk::Anchor::W).padx(4).layout();
    let columns = rstk::make_treeview(&struct_right);
    columns.height(10);
    columns.select_mode(Selection::Single);
    columns
        .pack()
        .fill(PackFill::Both)
        .expand(true)
        .padx(4)
        .pady(2)
        .layout();
    let schema_label = rstk::make_label(&struct_right);
    schema_label.text("Schema");
    schema_label.pack().anchor(rstk::Anchor::W).padx(4).layout();
    let schema = rstk::make_text(&struct_right);
    schema.height(8);
    schema
        .pack()
        .fill(PackFill::Both)
        .expand(true)
        .padx(4)
        .pady(2)
        .layout();

    let browse_bar = rstk::make_frame(&tab_browse);
    browse_bar.pack().fill(PackFill::X).padx(4).pady(4).layout();
    let table_lbl = rstk::make_label(&browse_bar);
    table_lbl.text("Table:");
    table_lbl.pack().side(PackSide::Left).layout();
    let table_combo = rstk::make_combobox(&browse_bar, &[]);
    table_combo.state(State::Readonly);
    table_combo.width(28);
    table_combo.pack().side(PackSide::Left).padx(4).layout();
    let btn_refresh = rstk::make_button(&browse_bar);
    btn_refresh.text("Refresh");
    btn_refresh.pack().side(PackSide::Left).padx(4).layout();
    let filter_hint = rstk::make_label(&browse_bar);
    filter_hint.text("Click heading to sort  ·  Filter boxes: LIKE, =, >, NULL");
    filter_hint.pack().side(PackSide::Left).padx(8).layout();
    let btn_clear_filter = rstk::make_button(&browse_bar);
    btn_clear_filter.text("Clear filters");
    btn_clear_filter.pack().side(PackSide::Left).padx(4).layout();

    let browse_filter_host = rstk::make_label(&tab_browse);
    let browse_filter_frame = rstk::make_frame(&tab_browse);
    browse_filter_frame.pack().fill(PackFill::X).padx(8).pady(2).layout();

    let browse_body = rstk::make_frame(&tab_browse);
    browse_body
        .pack()
        .fill(PackFill::Both)
        .expand(true)
        .layout();
    let browse_tree_hold = rstk::make_frame(&browse_body);
    browse_tree_hold
        .pack()
        .side(PackSide::Left)
        .fill(PackFill::Both)
        .expand(true)
        .padx(4)
        .pady(2)
        .layout();
    let browse_tree = rstk::make_treeview(&browse_tree_hold);
    browse_tree.height(18);
    browse_tree.select_mode(Selection::Single);
    attach_scrollbars(&browse_tree, &browse_tree_hold);
    let cell_frame = rstk::make_frame(&browse_body);
    cell_frame
        .pack()
        .side(PackSide::Right)
        .fill(PackFill::Y)
        .padx(4)
        .layout();
    let cell_lbl = rstk::make_label(&cell_frame);
    cell_lbl.text("Cell Editor");
    cell_lbl.pack().anchor(rstk::Anchor::W).layout();
    let cell = rstk::make_text(&cell_frame);
    cell.width(36);
    cell.height(16);
    cell.pack().fill(PackFill::Both).expand(true).layout();

    let pager = rstk::make_frame(&tab_browse);
    pager.pack().fill(PackFill::X).padx(4).pady(4).layout();
    let records = rstk::make_label(&pager);
    records.text("Records: 0");
    records.pack().side(PackSide::Left).padx(4).layout();
    let btn_prev = rstk::make_button(&pager);
    btn_prev.text("Prev");
    btn_prev.pack().side(PackSide::Left).padx(4).layout();
    let page = rstk::make_label(&pager);
    page.text("1");
    page.pack().side(PackSide::Left).layout();
    let btn_next = rstk::make_button(&pager);
    btn_next.text("Next");
    btn_next.pack().side(PackSide::Left).padx(4).layout();
    let page_size_lbl = rstk::make_label(&pager);
    page_size_lbl.text("Page size:");
    page_size_lbl.pack().side(PackSide::Left).padx(8).layout();
    let page_sizes: Vec<String> = PAGE_SIZES.iter().map(|n| n.to_string()).collect();
    let page_size_refs: Vec<&str> = page_sizes.iter().map(|s| s.as_str()).collect();
    let page_size = rstk::make_combobox(&pager, &page_size_refs);
    page_size.state(State::Readonly);
    page_size.width(6);
    page_size.pack().side(PackSide::Left).layout();
    combo_set(&page_size, "100");
    let all = rstk::make_check_button(&pager);
    all.text("Return All");
    all.pack().side(PackSide::Left).padx(8).layout();

    let sql_bar = rstk::make_frame(&tab_sql);
    sql_bar.pack().fill(PackFill::X).padx(4).pady(4).layout();
    let sql_query = rstk::make_text(&sql_bar);
    sql_query.height(5);
    sql_query
        .pack()
        .side(PackSide::Left)
        .fill(PackFill::Both)
        .expand(true)
        .layout();
    let btn_exec = rstk::make_button(&sql_bar);
    btn_exec.text("Execute");
    btn_exec.pack().side(PackSide::Right).padx(6).pady(8).layout();

    let sql_filter_bar = rstk::make_frame(&tab_sql);
    sql_filter_bar.pack().fill(PackFill::X).padx(4).layout();
    let sql_fl = rstk::make_label(&sql_filter_bar);
    sql_fl.text("Click heading to sort  ·  Filter boxes: LIKE, =, >, NULL");
    sql_fl.pack().side(PackSide::Left).layout();
    let btn_sql_clear = rstk::make_button(&sql_filter_bar);
    btn_sql_clear.text("Clear filters");
    btn_sql_clear.pack().side(PackSide::Left).padx(8).layout();
    let sql_filter_host = rstk::make_label(&tab_sql);
    let sql_filter_frame = rstk::make_frame(&tab_sql);
    sql_filter_frame.pack().fill(PackFill::X).padx(8).pady(2).layout();

    let sql_body = rstk::make_frame(&tab_sql);
    sql_body
        .pack()
        .fill(PackFill::Both)
        .expand(true)
        .layout();
    let sql_tree_hold = rstk::make_frame(&sql_body);
    sql_tree_hold
        .pack()
        .side(PackSide::Left)
        .fill(PackFill::Both)
        .expand(true)
        .padx(4)
        .pady(4)
        .layout();
    let sql_tree = rstk::make_treeview(&sql_tree_hold);
    sql_tree.height(14);
    sql_tree.select_mode(Selection::Single);
    attach_scrollbars(&sql_tree, &sql_tree_hold);
    let sql_cell_frame = rstk::make_frame(&sql_body);
    sql_cell_frame
        .pack()
        .side(PackSide::Right)
        .fill(PackFill::Y)
        .padx(4)
        .layout();
    let sql_cell_lbl = rstk::make_label(&sql_cell_frame);
    sql_cell_lbl.text("Cell Editor");
    sql_cell_lbl.pack().anchor(rstk::Anchor::W).layout();
    let sql_cell = rstk::make_text(&sql_cell_frame);
    sql_cell.width(36);
    sql_cell.height(14);
    sql_cell.pack().fill(PackFill::Both).expand(true).layout();

    let ui = Arc::new(Widgets {
        path,
        status,
        write_btn: btn_write.clone(),
        revert_btn: btn_revert.clone(),
        tables,
        columns,
        schema,
        table_combo,
        browse_tree,
        browse_filter_frame,
        browse_filter_host,
        sql_query,
        sql_tree,
        sql_filter_frame,
        sql_filter_host,
        cell,
        sql_cell,
        records,
        page,
        page_size,
        all,
        notebook,
    });

    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_new.command(move || do_new(&ui_c, &model_c));
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_open.command(move || do_open(&ui_c, &model_c));
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_close.command(move || {
            with_model(&model_c, |m| m.close());
            let m = model_c.lock().expect("model lock");
            refresh_all(&ui_c, &m);
            tree_clear(&ui_c.browse_tree);
            tree_clear(&ui_c.sql_tree);
            tree_clear(&ui_c.columns);
            listbox_clear(&ui_c.tables);
            text_set(&ui_c.schema, "");
            text_set(&ui_c.cell, "");
            text_set(&ui_c.sql_cell, "");
            text_set(&ui_c.sql_query, "SELECT * FROM sqlite_master");
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_write.command(move || {
            with_model(&model_c, |m| {
                let _ = m.write_changes();
            });
            let m = model_c.lock().expect("model lock");
            refresh_all(&ui_c, &m);
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_revert.command(move || {
            with_model(&model_c, |m| {
                let _ = m.revert_changes();
            });
            let m = model_c.lock().expect("model lock");
            refresh_all(&ui_c, &m);
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_connect.command(move || do_connect(&ui_c, &model_c));
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        ui.path.bind("<Return>", move |_| do_connect(&ui_c, &model_c));
    }

    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        ui.tables.bind("<<ListboxSelect>>", move |_| {
            let idxs = ui_c.tables.selected_items();
            if let Some(i) = idxs.first().copied() {
                let name = tcl_eval(&format!("{} get {}", ui_c.tables.id(), i));
                if !name.is_empty() {
                    with_model(&model_c, |m| {
                        let _ = m.select_structure_table(&name);
                    });
                    let m = model_c.lock().expect("model lock");
                    refresh_structure(&ui_c, &m);
                    set_status(&ui_c, &m);
                }
            }
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        ui.tables.bind("<Double-1>", move |ev| {
            let idx = tcl_eval(&format!("{} nearest {}", ui_c.tables.id(), ev.y));
            let name = tcl_eval(&format!("{} get {}", ui_c.tables.id(), idx));
            open_table_in_browse(&ui_c, &model_c, &name);
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_browse_table.command(move || {
            let idxs = ui_c.tables.selected_items();
            if let Some(i) = idxs.first().copied() {
                let name = tcl_eval(&format!("{} get {}", ui_c.tables.id(), i));
                open_table_in_browse(&ui_c, &model_c, &name);
            }
        });
    }

    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        ui.table_combo.bind("<<ComboboxSelected>>", move |_| {
            let name = combo_get(&ui_c.table_combo);
            open_table_in_browse(&ui_c, &model_c, &name);
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_refresh.command(move || {
            with_model(&model_c, |m| {
                let _ = m.load_browse_data();
            });
            let m = model_c.lock().expect("model lock");
            refresh_browse(&ui_c, &m, false);
            set_status(&ui_c, &m);
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_clear_filter.command(move || clear_column_filters(&ui_c, &model_c, false));
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        ui.browse_filter_host.bind("<Return>", move |_| {
            apply_column_filters(&ui_c, &model_c, false);
        });
    }

    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        let tree = ui.browse_tree.clone();
        ui.browse_tree.bind("<Button-1>", move |ev| {
            with_model(&model_c, |m| m.switch_tab(Tab::BrowseData));
            select_from_tree(&ui_c, &model_c, &tree, ev.x, ev.y);
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        let tree = ui.sql_tree.clone();
        ui.sql_tree.bind("<Button-1>", move |ev| {
            with_model(&model_c, |m| m.switch_tab(Tab::ExecuteSql));
            select_from_tree(&ui_c, &model_c, &tree, ev.x, ev.y);
        });
    }

    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_exec.command(move || {
            let sql = text_get(&ui_c.sql_query);
            with_model(&model_c, |m| {
                m.query = sql;
                m.switch_tab(Tab::ExecuteSql);
                let _ = m.execute_query();
            });
            let m = model_c.lock().expect("model lock");
            refresh_sql(&ui_c, &m, true);
            set_status(&ui_c, &m);
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        ui.sql_query.bind("<Control-Return>", move |_| {
            let sql = text_get(&ui_c.sql_query);
            with_model(&model_c, |m| {
                m.query = sql;
                m.switch_tab(Tab::ExecuteSql);
                let _ = m.execute_query();
            });
            let m = model_c.lock().expect("model lock");
            refresh_sql(&ui_c, &m, true);
            set_status(&ui_c, &m);
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_sql_clear.command(move || clear_column_filters(&ui_c, &model_c, true));
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        ui.sql_filter_host.bind("<Return>", move |_| {
            apply_column_filters(&ui_c, &model_c, true);
        });
    }

    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_prev.command(move || {
            with_model(&model_c, |m| {
                let page = active_grid(m).page;
                if page > 0 {
                    let _ = m.change_page(page - 1);
                }
            });
            let m = model_c.lock().expect("model lock");
            refresh_grid(&ui_c, &m, false);
            set_status(&ui_c, &m);
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        btn_next.command(move || {
            with_model(&model_c, |m| {
                let grid = active_grid(m);
                let next = grid.page + 1;
                if next < grid.total_pages() {
                    let _ = m.change_page(next);
                }
            });
            let m = model_c.lock().expect("model lock");
            refresh_grid(&ui_c, &m, false);
            set_status(&ui_c, &m);
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        ui.page_size.bind("<<ComboboxSelected>>", move |_| {
            if let Ok(size) = combo_get(&ui_c.page_size).parse::<u32>() {
                with_model(&model_c, |m| {
                    let _ = m.change_page_size(size);
                });
                let m = model_c.lock().expect("model lock");
                refresh_grid(&ui_c, &m, false);
                set_status(&ui_c, &m);
            }
        });
    }
    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        ui.all.command(move |checked| {
            with_model(&model_c, |m| {
                let _ = m.toggle_all_results(checked);
            });
            let m = model_c.lock().expect("model lock");
            refresh_grid(&ui_c, &m, false);
            set_status(&ui_c, &m);
        });
    }

    {
        let ui_c = ui.clone();
        let model_c = model.clone();
        let nb_id = ui.notebook.id().to_string();
        ui.notebook.bind("<<NotebookTabChanged>>", move |_| {
            let idx = tcl_eval(&format!("{nb_id} index current"));
            let tab = match idx.trim() {
                "0" => Tab::Structure,
                "1" => Tab::BrowseData,
                _ => Tab::ExecuteSql,
            };
            with_model(&model_c, |m| m.switch_tab(tab));
            let m = model_c.lock().expect("model lock");
            match tab {
                Tab::Structure => refresh_structure(&ui_c, &m),
                Tab::BrowseData => refresh_browse(&ui_c, &m, false),
                Tab::ExecuteSql => refresh_sql(&ui_c, &m, false),
            }
        });
    }

    text_set(&ui.sql_query, "SELECT * FROM sqlite_master");
    place_root_window();
    ui
}

fn main() {
    init_gui_host();
    tracing_subscriber::fmt::init();
    if let Err(err) = run() {
        report_error("Turso DB Browser (Tcl/Tk)", &format!("{err:#}"));
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    let root = start_wish()?;
    let model = Arc::new(Mutex::new(AppModel::from_args(
        args.database,
        args.token,
        args.debug,
    )));
    let ui = build_ui(&root, model.clone());
    {
        let m = model.lock().expect("model lock");
        refresh_all(&ui, &m);
        text_set(&ui.sql_query, &m.query);
    }
    place_root_window();
    rstk::mainloop();
    Ok(())
}
