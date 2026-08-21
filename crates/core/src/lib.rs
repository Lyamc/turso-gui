mod app;
pub mod cli;
mod db;
pub mod host;
mod query;
mod types;

pub use app::{AppModel, StatusKind};
pub use host::{
    console_flag_present, init_gui_host, report_error, setup_console, WindowPlacement,
};
pub use db::{Db, QueryResult, TableColumn, TableInfo};
pub use query::{
    apply_sort, build_browse_sql, build_filter_clauses, compile_filter, quote_ident,
    wrap_select_with_filters, PAGE_SIZES,
};
pub use types::{GridState, SortDirection, Tab};

use tokio::runtime::Runtime;

pub(crate) fn new_runtime() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime")
}
