mod event;
mod ui;

use std::io::{self, stdout, Stdout};
use std::panic::{set_hook, take_hook};

use anyhow::Result;
use clap::Parser;
use crossterm::event::{DisableMouseCapture, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::{ListState, TableState};
use ratatui::Terminal;
use tui_textarea::TextArea;
use turso_gui_core::{AppModel, Tab};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Turso / SQLite DB Browser (TUI)",
    long_about = None
)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    Insert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Popup {
    None,
    Help,
    Path { new: bool },
    Filter { col: usize, sql: bool },
}

struct App {
    model: AppModel,
    should_quit: bool,
    mode: Mode,
    popup: Popup,
    table_list: ListState,
    browse_table: TableState,
    sql_table: TableState,
    sql_editor: TextArea<'static>,
    cell_editor: TextArea<'static>,
    path_editor: TextArea<'static>,
    filter_editor: TextArea<'static>,
    show_cell_pane: bool,
    cell_focus: bool,
}

impl App {
    fn new(args: Args) -> Self {
        let model = AppModel::from_args(args.database, args.token, args.debug);
        let connected = model.is_connected();
        let mut sql_editor = textarea_from(&model.query);
        sql_editor.set_placeholder_text("SELECT * FROM table");
        let mut path_editor = textarea_from(&model.db_path);
        path_editor.set_placeholder_text("path/to.db or file.sqlite");
        let mut cell_editor = TextArea::default();
        cell_editor.set_placeholder_text("Select a cell and press Enter");
        let mut filter_editor = TextArea::default();
        filter_editor.set_placeholder_text("filter (LIKE, =, >, NULL)");

        let mut app = Self {
            model,
            should_quit: false,
            mode: if connected { Mode::Normal } else { Mode::Insert },
            popup: Popup::None,
            table_list: ListState::default(),
            browse_table: TableState::default(),
            sql_table: TableState::default(),
            sql_editor,
            cell_editor,
            path_editor,
            filter_editor,
            show_cell_pane: false,
            cell_focus: false,
        };
        if connected {
            app.after_connect();
        }
        app
    }

    fn after_connect(&mut self) {
        if self.model.selected_structure_table.is_none() {
            if let Some(name) = self.model.table_names.first().cloned() {
                let _ = self.model.select_structure_table(&name);
            }
        }
        self.sync_table_list();
        self.reset_browse_cursor();
        self.reset_sql_cursor();
        self.mode = Mode::Normal;
        self.popup = Popup::None;
        self.show_cell_pane = false;
        self.cell_focus = false;
    }

    fn sync_table_list(&mut self) {
        let idx = self
            .model
            .selected_structure_table
            .as_ref()
            .and_then(|n| self.model.table_names.iter().position(|t| t == n));
        self.table_list.select(idx.or_else(|| {
            if self.model.table_names.is_empty() {
                None
            } else {
                Some(0)
            }
        }));
    }

    fn reset_browse_cursor(&mut self) {
        reset_table_cursor(&mut self.browse_table, &self.model.browse_state);
    }

    fn reset_sql_cursor(&mut self) {
        reset_table_cursor(&mut self.sql_table, &self.model.sql_state);
    }

    fn switch_tab(&mut self, tab: Tab) {
        self.model.switch_tab(tab);
        self.cell_focus = false;
        if self.popup != Popup::None {
            return;
        }
        match tab {
            Tab::ExecuteSql => {
                self.mode = Mode::Insert;
            }
            _ => {
                self.mode = Mode::Normal;
            }
        }
    }

    fn cycle_tab(&mut self, reverse: bool) {
        let all = Tab::all();
        let i = all
            .iter()
            .position(|t| *t == self.model.active_tab)
            .unwrap_or(0);
        let next = if reverse {
            i.checked_sub(1).unwrap_or(all.len() - 1)
        } else {
            (i + 1) % all.len()
        };
        self.switch_tab(all[next]);
    }

    fn move_structure(&mut self, delta: i32) {
        let len = self.model.table_names.len();
        if len == 0 {
            return;
        }
        let i = self.table_list.selected().unwrap_or(0) as i32;
        let ni = (i + delta).clamp(0, len as i32 - 1) as usize;
        self.table_list.select(Some(ni));
    }

    fn load_structure_selected(&mut self) {
        if let Some(i) = self.table_list.selected() {
            if let Some(name) = self.model.table_names.get(i).cloned() {
                let _ = self.model.select_structure_table(&name);
            }
        }
    }

    fn cycle_browse_table(&mut self, delta: i32) {
        let len = self.model.table_names.len() as i32;
        if len == 0 {
            return;
        }
        let cur = self
            .model
            .browse_table
            .as_ref()
            .and_then(|b| self.model.table_names.iter().position(|n| n == b))
            .unwrap_or(0) as i32;
        let ni = (cur + delta).rem_euclid(len) as usize;
        let name = self.model.table_names[ni].clone();
        let _ = self.model.browse_table_named(&name);
        self.reset_browse_cursor();
        self.show_cell_pane = false;
        self.cell_focus = false;
    }

    fn page_delta(&mut self, delta: i32) {
        let (page, total) = match self.model.active_tab {
            Tab::ExecuteSql => (self.model.sql_state.page, self.model.sql_state.total_pages()),
            _ => (
                self.model.browse_state.page,
                self.model.browse_state.total_pages(),
            ),
        };
        let new_page = (page as i32 + delta).clamp(0, total.saturating_sub(1) as i32) as u32;
        if new_page != page {
            let _ = self.model.change_page(new_page);
            match self.model.active_tab {
                Tab::ExecuteSql => self.reset_sql_cursor(),
                _ => self.reset_browse_cursor(),
            }
        }
    }

    fn cycle_page_size(&mut self, next: bool) {
        let current = match self.model.active_tab {
            Tab::ExecuteSql => self.model.sql_state.page_size,
            _ => self.model.browse_state.page_size,
        };
        let sizes = turso_gui_core::PAGE_SIZES;
        let idx = sizes.iter().position(|&s| s == current).unwrap_or(3);
        let new_idx = if next {
            (idx + 1).min(sizes.len() - 1)
        } else {
            idx.saturating_sub(1)
        };
        if sizes[new_idx] != current {
            let _ = self.model.change_page_size(sizes[new_idx]);
            match self.model.active_tab {
                Tab::ExecuteSql => self.reset_sql_cursor(),
                _ => self.reset_browse_cursor(),
            }
        }
    }

    fn toggle_all_results(&mut self) {
        let all = match self.model.active_tab {
            Tab::ExecuteSql => !self.model.sql_state.all_results,
            _ => !self.model.browse_state.all_results,
        };
        let _ = self.model.toggle_all_results(all);
        match self.model.active_tab {
            Tab::ExecuteSql => self.reset_sql_cursor(),
            _ => self.reset_browse_cursor(),
        }
    }

    fn sort_current_column(&mut self) {
        let sql = self.model.active_tab == Tab::ExecuteSql;
        let headers_len = if sql {
            self.model.sql_state.headers.len()
        } else {
            self.model.browse_state.headers.len()
        };
        if headers_len == 0 {
            return;
        }
        let col = self.current_column().min(headers_len - 1);
        let _ = self.model.sort_column(col);
        if sql {
            self.reset_sql_cursor();
        } else {
            self.reset_browse_cursor();
        }
    }

    fn inspect_cell(&mut self) {
        let state = match self.model.active_tab {
            Tab::ExecuteSql => &self.sql_table,
            _ => &self.browse_table,
        };
        let Some((row, col)) = state.selected_cell() else {
            return;
        };
        self.model.select_cell(row, col);
        self.cell_editor = textarea_from(&self.model.cell_editor);
        self.cell_editor
            .set_placeholder_text("Select a cell and press Enter");
        self.show_cell_pane = true;
        self.cell_focus = false;
    }

    fn move_grid(&mut self, dr: i32, dc: i32) {
        let sql = self.model.active_tab == Tab::ExecuteSql;
        let grid = if sql {
            &self.model.sql_state
        } else {
            &self.model.browse_state
        };
        let table = if sql {
            &mut self.sql_table
        } else {
            &mut self.browse_table
        };
        move_table_sel(table, grid.rows.len(), grid.headers.len(), dr, dc);
    }

    fn jump_grid_row(&mut self, last: bool) {
        let sql = self.model.active_tab == Tab::ExecuteSql;
        let rows = if sql {
            self.model.sql_state.rows.len()
        } else {
            self.model.browse_state.rows.len()
        };
        if rows == 0 {
            return;
        }
        let table = if sql {
            &mut self.sql_table
        } else {
            &mut self.browse_table
        };
        table.select(Some(if last { rows - 1 } else { 0 }));
    }

    fn jump_grid_col(&mut self, last: bool) {
        let sql = self.model.active_tab == Tab::ExecuteSql;
        let cols = if sql {
            self.model.sql_state.headers.len()
        } else {
            self.model.browse_state.headers.len()
        };
        if cols == 0 {
            return;
        }
        let table = if sql {
            &mut self.sql_table
        } else {
            &mut self.browse_table
        };
        table.select_column(Some(if last { cols - 1 } else { 0 }));
    }

    fn current_column(&self) -> usize {
        let state = match self.model.active_tab {
            Tab::ExecuteSql => &self.sql_table,
            _ => &self.browse_table,
        };
        state.selected_column().unwrap_or(0)
    }

    fn open_filter_popup(&mut self) {
        let sql = self.model.active_tab == Tab::ExecuteSql;
        let col = self.current_column();
        let grid = if sql {
            &self.model.sql_state
        } else {
            &self.model.browse_state
        };
        if grid.headers.is_empty() {
            return;
        }
        let col = col.min(grid.headers.len().saturating_sub(1));
        let current = grid.filters.get(col).cloned().unwrap_or_default();
        self.filter_editor = textarea_from(&current);
        self.filter_editor
            .set_placeholder_text("filter (empty clears)");
        self.popup = Popup::Filter { col, sql };
        self.mode = Mode::Insert;
        self.cell_focus = false;
    }

    fn apply_filter(&mut self) {
        let Popup::Filter { col, sql } = self.popup else {
            return;
        };
        let value = textarea_line(&self.filter_editor);
        if sql {
            let _ = self.model.set_sql_filter(col, value);
            self.reset_sql_cursor();
        } else {
            let _ = self.model.set_browse_filter(col, value);
            self.reset_browse_cursor();
        }
        self.close_popup();
    }

    fn open_path_popup(&mut self, new: bool) {
        let text = if new {
            String::new()
        } else {
            self.model.db_path.clone()
        };
        self.path_editor = textarea_from(&text);
        self.path_editor
            .set_placeholder_text("path/to.db or file.sqlite");
        self.popup = Popup::Path { new };
        self.mode = Mode::Insert;
        self.cell_focus = false;
    }

    fn apply_path(&mut self) {
        let path = textarea_line(&self.path_editor).trim().to_string();
        if path.is_empty() {
            self.model.error = Some("Path is empty".into());
            self.model.success_msg = None;
            return;
        }
        if self.model.open_path(path).is_ok() {
            self.after_connect();
        }
    }

    fn execute_sql(&mut self) {
        self.sync_query();
        self.model.switch_tab(Tab::ExecuteSql);
        if self.model.execute_query().is_ok() {
            if self.model.has_changes {
                let _ = self.model.refresh_tables();
                self.sync_table_list();
            }
            self.reset_sql_cursor();
        }
    }

    fn sync_query(&mut self) {
        self.model.query = self.sql_editor.lines().join("\n");
    }

    fn sync_cell_editor(&mut self) {
        self.model.cell_editor = self.cell_editor.lines().join("\n");
    }

    fn close_popup(&mut self) {
        let was_help = matches!(self.popup, Popup::Help);
        self.popup = Popup::None;
        if !was_help {
            self.mode = Mode::Normal;
            self.cell_focus = false;
        }
    }

    fn is_editing(&self) -> bool {
        self.mode == Mode::Insert && !matches!(self.popup, Popup::Help)
    }
}

fn textarea_from(s: &str) -> TextArea<'static> {
    if s.is_empty() {
        TextArea::default()
    } else {
        TextArea::from(s.lines().map(str::to_string))
    }
}

fn textarea_line(ta: &TextArea<'_>) -> String {
    ta.lines().join("")
}

fn reset_table_cursor(state: &mut TableState, grid: &turso_gui_core::GridState) {
    if grid.rows.is_empty() || grid.headers.is_empty() {
        state.select(None);
        state.select_column(None);
        return;
    }
    let row = state.selected().unwrap_or(0).min(grid.rows.len() - 1);
    let col = state
        .selected_column()
        .unwrap_or(0)
        .min(grid.headers.len() - 1);
    state.select(Some(row));
    state.select_column(Some(col));
}

fn move_table_sel(state: &mut TableState, rows: usize, cols: usize, dr: i32, dc: i32) {
    if rows == 0 || cols == 0 {
        state.select(None);
        state.select_column(None);
        return;
    }
    let r = state.selected().unwrap_or(0) as i32;
    let c = state.selected_column().unwrap_or(0) as i32;
    let nr = (r + dr).clamp(0, rows as i32 - 1) as usize;
    let nc = (c + dc).clamp(0, cols as i32 - 1) as usize;
    state.select(Some(nr));
    state.select_column(Some(nc));
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn setup() -> Result<Self> {
        enable_raw_mode()?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(out))?;
        Ok(Self { terminal })
    }

    fn restore() {
        let _ = disable_raw_mode();
        let mut out = stdout();
        let _ = execute!(out, LeaveAlternateScreen, DisableMouseCapture);
        let _ = io::Write::flush(&mut out);
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = disable_raw_mode();
        let _ = self.terminal.show_cursor();
    }
}

fn install_panic_hook() {
    let original = take_hook();
    set_hook(Box::new(move |info| {
        TerminalGuard::restore();
        original(info);
    }));
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.debug {
        tracing_subscriber::fmt()
            .with_writer(io::stderr)
            .with_ansi(false)
            .init();
    }

    install_panic_hook();
    let mut tui = TerminalGuard::setup()?;
    tui.terminal.hide_cursor()?;
    tui.terminal.clear()?;

    let mut app = App::new(args);
    let result = run_app(&mut tui, &mut app);

    drop(tui);
    result
}

fn run_app(tui: &mut TerminalGuard, app: &mut App) -> Result<()> {
    while !app.should_quit {
        tui.terminal.draw(|f| ui::draw(f, app))?;
        match crossterm::event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                event::handle_key(app, key);
            }
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
    Ok(())
}
