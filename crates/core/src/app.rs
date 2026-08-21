use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use tokio::runtime::Runtime;

use crate::db::{Db, TableColumn, TableInfo};
use crate::query::{apply_sort, build_browse_sql, is_select, wrap_select_with_filters};
use crate::types::{GridState, SortDirection, Tab};
use crate::new_runtime;

pub struct AppModel {
    rt: Runtime,
    db: Option<Arc<Db>>,
    pub db_path: String,
    pub token: Option<String>,
    pub debug: bool,
    pub active_tab: Tab,
    pub tables: Vec<TableInfo>,
    pub table_names: Vec<String>,
    pub selected_structure_table: Option<String>,
    pub selected_table_columns: Vec<TableColumn>,
    pub browse_state: GridState,
    pub sql_state: GridState,
    pub browse_table: Option<String>,
    pub cell_editor: String,
    pub query: String,
    pub loading: bool,
    pub has_changes: bool,
    pub error: Option<String>,
    pub success_msg: Option<String>,
}

impl AppModel {
    pub fn new(db_path: String, token: Option<String>, debug: bool) -> Self {
        Self {
            rt: new_runtime(),
            db: None,
            db_path,
            token,
            debug,
            active_tab: Tab::Structure,
            tables: Vec::new(),
            table_names: Vec::new(),
            selected_structure_table: None,
            selected_table_columns: Vec::new(),
            browse_state: GridState::default(),
            sql_state: GridState::default(),
            browse_table: None,
            cell_editor: String::new(),
            query: "SELECT * FROM sqlite_master".to_string(),
            loading: false,
            has_changes: false,
            error: None,
            success_msg: None,
        }
    }

    pub fn from_args(database: Option<String>, token: Option<String>, debug: bool) -> Self {
        let db_path = database.unwrap_or_else(|| "local.db".to_string());
        let mut app = Self::new(db_path, token, debug);
        if !app.db_path.is_empty() && !app.db_path.starts_with("libsql://") {
            let _ = app.connect();
        }
        app
    }

    fn block<T>(&self, fut: impl Future<Output = T>) -> T {
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| self.rt.block_on(fut))
        } else {
            self.rt.block_on(fut)
        }
    }

    pub fn is_connected(&self) -> bool {
        self.db.is_some()
    }

    pub fn status_text(&self) -> Option<(&str, StatusKind)> {
        if self.loading {
            Some(("Loading...", StatusKind::Info))
        } else if let Some(e) = &self.error {
            Some((e.as_str(), StatusKind::Error))
        } else if let Some(s) = &self.success_msg {
            Some((s.as_str(), StatusKind::Success))
        } else {
            None
        }
    }

    pub fn clear_status(&mut self) {
        self.error = None;
        self.success_msg = None;
    }

    pub fn connect(&mut self) -> Result<()> {
        self.clear_status();
        self.loading = true;
        let path = self.db_path.clone();
        let token = self.token.clone();
        let debug = self.debug;
        let result = self.block(async move { Db::open(&path, token, debug).await });
        match result {
            Ok(db) => {
                self.db = Some(Arc::new(db));
                self.loading = false;
                self.has_changes = false;
                self.refresh_tables()
            }
            Err(e) => {
                self.loading = false;
                self.error = Some(e.to_string());
                Err(e)
            }
        }
    }

    pub fn close(&mut self) {
        let path = self.db_path.clone();
        let token = self.token.clone();
        let debug = self.debug;
        *self = Self::new(path, token, debug);
    }

    pub fn open_path(&mut self, path: String) -> Result<()> {
        self.db_path = path;
        self.connect()
    }

    pub fn switch_tab(&mut self, tab: Tab) {
        self.active_tab = tab;
    }

    pub fn write_changes(&mut self) -> Result<()> {
        self.clear_status();
        let db = self.require_db()?;
        let result = self.block(async move { db.commit_transaction().await });
        self.finish_write_op(result)
    }

    pub fn revert_changes(&mut self) -> Result<()> {
        self.clear_status();
        let db = self.require_db()?;
        let result = self.block(async move { db.rollback_transaction().await });
        self.finish_write_op(result)
    }

    fn finish_write_op(&mut self, result: Result<()>) -> Result<()> {
        match result {
            Ok(()) => {
                self.success_msg = Some("Success".into());
                self.has_changes = false;
                self.refresh_tables()
            }
            Err(e) => {
                self.error = Some(e.to_string());
                Err(e)
            }
        }
    }

    pub fn refresh_tables(&mut self) -> Result<()> {
        let db = self.require_db()?;
        let result = self.block(async move { db.list_tables_full().await });
        match result {
            Ok(tables) => {
                self.tables = tables;
                self.table_names = self.tables.iter().map(|t| t.name.clone()).collect();
                if self.browse_table.is_none() {
                    if let Some(name) = self.table_names.first().cloned() {
                        return self.browse_table_named(&name);
                    }
                }
                Ok(())
            }
            Err(e) => {
                self.error = Some(e.to_string());
                Err(e)
            }
        }
    }

    pub fn select_structure_table(&mut self, name: &str) -> Result<()> {
        self.clear_status();
        self.selected_structure_table = Some(name.to_string());
        let db = self.require_db()?;
        let table = name.to_string();
        let result = self.block(async move { db.get_table_columns(&table).await });
        match result {
            Ok(cols) => {
                self.selected_table_columns = cols;
                Ok(())
            }
            Err(e) => {
                self.error = Some(e.to_string());
                Err(e)
            }
        }
    }

    pub fn structure_sql(&self) -> &str {
        let name = self
            .selected_structure_table
            .as_ref()
            .or(self.browse_table.as_ref());
        name.and_then(|n| self.tables.iter().find(|t| &t.name == n))
            .map(|t| t.sql.as_str())
            .unwrap_or("")
    }

    pub fn browse_table_named(&mut self, name: &str) -> Result<()> {
        self.clear_status();
        self.browse_table = Some(name.to_string());
        self.browse_state.page = 0;
        self.browse_state.filters.clear();
        self.browse_state.sort = None;
        self.browse_state.selected_cell = None;
        self.loading = true;
        let count_res = self.count_browse_table();
        let data_res = self.load_browse_data();
        self.loading = false;
        count_res.and(data_res)
    }

    fn count_browse_table(&mut self) -> Result<()> {
        let db = self.require_db()?;
        let table = match &self.browse_table {
            Some(t) if !t.is_empty() => t.clone(),
            _ => return Ok(()),
        };
        let result = self.block(async move { db.count_table(&table).await });
        match result {
            Ok(n) => {
                self.browse_state.total_records = n;
                Ok(())
            }
            Err(e) => {
                self.error = Some(e.to_string());
                Err(e)
            }
        }
    }

    pub fn set_browse_filter(&mut self, index: usize, value: String) -> Result<()> {
        if index >= self.browse_state.filters.len() {
            self.browse_state.filters.resize(index + 1, String::new());
        }
        self.browse_state.filters[index] = value;
        self.browse_state.page = 0;
        self.load_browse_data()
    }

    pub fn set_sql_filter(&mut self, index: usize, value: String) -> Result<()> {
        if index >= self.sql_state.filters.len() {
            self.sql_state.filters.resize(index + 1, String::new());
        }
        self.sql_state.filters[index] = value;
        self.sql_state.page = 0;
        self.execute_query()
    }

    pub fn sort_column(&mut self, index: usize) -> Result<()> {
        let next = match self.active_grid().sort {
            Some((col, SortDirection::Asc)) if col == index => Some((index, SortDirection::Desc)),
            Some((col, SortDirection::Desc)) if col == index => None,
            _ => Some((index, SortDirection::Asc)),
        };
        let state = self.active_grid_mut();
        state.sort = next;
        state.page = 0;
        match self.active_tab {
            Tab::ExecuteSql => self.execute_query(),
            _ => self.load_browse_data(),
        }
    }

    pub fn change_page(&mut self, page: u32) -> Result<()> {
        match self.active_tab {
            Tab::ExecuteSql => {
                self.sql_state.page = page;
                self.execute_query()
            }
            _ => {
                self.browse_state.page = page;
                self.load_browse_data()
            }
        }
    }

    pub fn change_page_size(&mut self, size: u32) -> Result<()> {
        match self.active_tab {
            Tab::ExecuteSql => {
                self.sql_state.page_size = size;
                self.sql_state.page = 0;
                self.execute_query()
            }
            _ => {
                self.browse_state.page_size = size;
                self.browse_state.page = 0;
                self.load_browse_data()
            }
        }
    }

    pub fn toggle_all_results(&mut self, all: bool) -> Result<()> {
        match self.active_tab {
            Tab::ExecuteSql => {
                self.sql_state.all_results = all;
                self.sql_state.page = 0;
                self.execute_query()
            }
            _ => {
                self.browse_state.all_results = all;
                self.browse_state.page = 0;
                self.load_browse_data()
            }
        }
    }

    pub fn resize_column(&mut self, index: usize, width: f32) {
        let widths = match self.active_tab {
            Tab::ExecuteSql => &mut self.sql_state.widths,
            _ => &mut self.browse_state.widths,
        };
        if index < widths.len() {
            widths[index] = width.max(40.0);
        }
    }

    pub fn adjust_column_width(&mut self, index: usize, delta: f32) {
        let widths = match self.active_tab {
            Tab::ExecuteSql => &mut self.sql_state.widths,
            _ => &mut self.browse_state.widths,
        };
        if index < widths.len() {
            widths[index] = (widths[index] + delta).max(40.0);
        }
    }

    pub fn select_cell(&mut self, row: usize, col: usize) {
        let state = match self.active_tab {
            Tab::ExecuteSql => &mut self.sql_state,
            _ => &mut self.browse_state,
        };
        state.selected_cell = Some((row, col));
        self.cell_editor = state.cell(row, col).unwrap_or("").to_string();
    }

    pub fn select_row(&mut self, row: usize) {
        let state = match self.active_tab {
            Tab::ExecuteSql => &mut self.sql_state,
            _ => &mut self.browse_state,
        };
        state.selected_cell = Some((row, 0));
        self.cell_editor = state.row_display(row);
    }

    pub fn load_browse_data(&mut self) -> Result<()> {
        self.clear_status();
        let db = self.require_db()?;
        let table = match &self.browse_table {
            Some(t) if !t.is_empty() => t.clone(),
            _ => return Ok(()),
        };
        let (limit, offset) = self.browse_state.limit_offset();
        let sql = build_browse_sql(
            &table,
            &self.browse_state.headers,
            &self.browse_state.filters,
            self.browse_state.sort,
        );
        self.loading = true;
        let result = self.block(async move { db.query(&sql, limit, offset).await });
        self.loading = false;
        match result {
            Ok(res) => {
                self.browse_state.apply_result(res.headers, res.rows);
                Ok(())
            }
            Err(e) => {
                self.error = Some(e.to_string());
                Err(e)
            }
        }
    }

    pub fn execute_query(&mut self) -> Result<()> {
        self.clear_status();
        let db = self.require_db()?;
        let sql = self.query.clone();
        let select = is_select(&sql);
        let final_sql = if select {
            let filtered =
                wrap_select_with_filters(&sql, &self.sql_state.headers, &self.sql_state.filters);
            apply_sort(filtered, &self.sql_state.headers, self.sql_state.sort)
        } else {
            sql
        };
        let is_write = !select;
        let begin_first = is_write && !self.has_changes;
        let (limit, offset) = if select {
            self.sql_state.limit_offset()
        } else {
            (None, None)
        };

        self.loading = true;
        let result = self.block(async move {
            if begin_first {
                let _ = db.begin_transaction().await;
            }
            db.query(&final_sql, limit, offset).await
        });
        self.loading = false;

        match result {
            Ok(res) => {
                self.sql_state.apply_result(res.headers, res.rows);
                if !select {
                    self.sql_state.total_records = self.sql_state.rows.len() as u64;
                    self.has_changes = true;
                } else {
                    self.sql_state.total_records = self.sql_state.rows.len() as u64;
                }
                Ok(())
            }
            Err(e) => {
                self.error = Some(e.to_string());
                Err(e)
            }
        }
    }

    pub fn active_grid(&self) -> &GridState {
        match self.active_tab {
            Tab::ExecuteSql => &self.sql_state,
            _ => &self.browse_state,
        }
    }

    pub fn active_grid_mut(&mut self) -> &mut GridState {
        match self.active_tab {
            Tab::ExecuteSql => &mut self.sql_state,
            _ => &mut self.browse_state,
        }
    }

    fn require_db(&self) -> Result<Arc<Db>> {
        self.db
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No database connected"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Error,
    Success,
}

impl AppModel {
    pub fn status_kind(&self) -> Option<StatusKind> {
        self.status_text().map(|(_, k)| k)
    }
}
