use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use turso_gui_core::AppModel;

pub const FRONTENDS: &[Frontend] = &[
    Frontend {
        id: "iced",
        package: "turso-gui",
        bin: "turso-gui",
        crate_dir: "crates/iced",
        toolkit: "iced",
    },
    Frontend {
        id: "egui",
        package: "turso-gui-egui",
        bin: "turso-gui-egui",
        crate_dir: "crates/egui",
        toolkit: "egui",
    },
    Frontend {
        id: "gpui",
        package: "turso-gui-gpui",
        bin: "turso-gui-gpui",
        crate_dir: "crates/gpui",
        toolkit: "gpui",
    },
    Frontend {
        id: "tui",
        package: "turso-gui-tui",
        bin: "turso-gui-tui",
        crate_dir: "crates/tui",
        toolkit: "ratatui",
    },
    Frontend {
        id: "tk",
        package: "turso-gui-tk",
        bin: "turso-gui-tk",
        crate_dir: "crates/tk",
        toolkit: "tcl/tk",
    },
    Frontend {
        id: "dioxus",
        package: "turso-gui-dioxus",
        bin: "turso-gui-dioxus",
        crate_dir: "crates/dioxus",
        toolkit: "dioxus",
    },
];

/// GUI crates that are not part of the frontend matrix.
pub const NON_GUI_CRATES: &[&str] = &["core", "eval"];

/// AppModel methods / CLI bits every frontend is expected to wire up.
pub const REQUIRED_FEATURES: &[Feature] = &[
    Feature::new("from_args", &["from_args"]),
    Feature::new("open_path", &["open_path"]),
    Feature::new("close", &["model.close()", "m.close()", "borrow_mut().close()"]),
    Feature::new("write_changes", &["write_changes"]),
    Feature::new("revert_changes", &["revert_changes"]),
    Feature::new("switch_tab", &["switch_tab"]),
    Feature::new("structure_tab", &["Tab::Structure"]),
    Feature::new("browse_tab", &["Tab::BrowseData"]),
    Feature::new("sql_tab", &["Tab::ExecuteSql"]),
    Feature::new("select_structure_table", &["select_structure_table"]),
    Feature::new("browse_table", &["browse_table_named"]),
    Feature::new("browse_filter", &["set_browse_filter", "browse_state.filters"]),
    Feature::new("sql_filter", &["set_sql_filter", "sql_state.filters"]),
    Feature::new(
        "column_filter_boxes",
        &[
            "Filter...",
            "Filter…",
            "Filter (=",
            "rebuild_filter_row",
            "open_filter_popup",
        ],
    ),
    Feature::new("sort_column", &["sort_column"]),
    Feature::new("change_page", &["change_page"]),
    Feature::new("change_page_size", &["change_page_size"]),
    Feature::new("toggle_all_results", &["toggle_all_results"]),
    Feature::new("select_cell", &["select_cell", "selected_cell"]),
    Feature::new("execute_query", &["execute_query"]),
    Feature::new("cell_editor", &["cell_editor"]),
    Feature::new(
        "page_sizes",
        &[
            "PAGE_SIZES",
            "\"10\", \"25\", \"50\", \"100\", \"250\", \"500\", \"1000\"",
        ],
    ),
    Feature::new("clap_database", &["database: Option<String>"]),
    Feature::new("clap_token", &["token: Option<String>"]),
    Feature::new("clap_debug", &["short = 'D'"]),
];

/// Documented gaps: (frontend id, feature name). Unexpected gaps fail the suite.
pub const KNOWN_GAPS: &[(&str, &str)] = &[("tui", "close")];

pub const REQUIRED_HELP_FLAGS: &[&str] = &["--database", "--token", "--debug"];

#[derive(Debug, Clone, Copy)]
pub struct Frontend {
    pub id: &'static str,
    pub package: &'static str,
    pub bin: &'static str,
    pub crate_dir: &'static str,
    pub toolkit: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Feature {
    pub name: &'static str,
    pub needles: &'static [&'static str],
}

impl Feature {
    pub const fn new(name: &'static str, needles: &'static [&'static str]) -> Self {
        Self { name, needles }
    }
}

#[derive(Debug, Clone)]
pub struct CliOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
    pub elapsed: Duration,
}

static CARGO_LOCK: Mutex<()> = Mutex::new(());
static BUILD_ONCE: Once = Once::new();

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/eval is two levels below the workspace root")
        .to_path_buf()
}

pub fn frontend_dir(frontend: &Frontend) -> PathBuf {
    workspace_root().join(frontend.crate_dir)
}

pub fn gui_crate_names() -> Vec<String> {
    let mut names = Vec::new();
    let crates_dir = workspace_root().join("crates");
    for entry in fs::read_dir(&crates_dir).expect("read crates/") {
        let entry = entry.expect("crates/ entry");
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if NON_GUI_CRATES.contains(&name.as_str()) {
            continue;
        }
        names.push(name);
    }
    names.sort();
    names
}

pub fn catalog_crate_names() -> Vec<String> {
    let mut names: Vec<String> = FRONTENDS
        .iter()
        .map(|f| {
            f.crate_dir
                .trim_start_matches("crates/")
                .trim_start_matches("crates\\")
                .to_string()
        })
        .collect();
    names.sort();
    names
}

pub fn rust_sources(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    visit_rs(dir, &mut files);
    files.sort();
    files
}

fn visit_rs(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name == "target" || name == ".git" {
            continue;
        }
        if path.is_dir() {
            visit_rs(&path, files);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

pub fn crate_source(frontend: &Frontend) -> String {
    let mut out = String::new();
    for path in rust_sources(&frontend_dir(frontend)) {
        if let Ok(text) = fs::read_to_string(&path) {
            out.push_str(&text);
            out.push('\n');
        }
    }
    out
}

pub fn rust_line_count(frontend: &Frontend) -> usize {
    rust_sources(&frontend_dir(frontend))
        .iter()
        .filter_map(|p| fs::read_to_string(p).ok())
        .map(|text| text.lines().count())
        .sum()
}

pub fn feature_present(source: &str, feature: &Feature) -> bool {
    feature.needles.iter().any(|needle| source.contains(needle))
}

pub fn is_known_gap(frontend_id: &str, feature: &str) -> bool {
    KNOWN_GAPS
        .iter()
        .any(|(id, name)| *id == frontend_id && *name == feature)
}

pub fn missing_features(frontend: &Frontend) -> Vec<&'static str> {
    let source = crate_source(frontend);
    REQUIRED_FEATURES
        .iter()
        .filter(|feature| !feature_present(&source, feature))
        .filter(|feature| !is_known_gap(frontend.id, feature.name))
        .map(|feature| feature.name)
        .collect()
}

pub fn target_debug_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CARGO_TARGET_DIR") {
        PathBuf::from(dir).join("debug")
    } else {
        workspace_root().join("target").join("debug")
    }
}

pub fn debug_bin_path(frontend: &Frontend) -> PathBuf {
    target_debug_dir().join(format!(
        "{}{}",
        frontend.bin,
        std::env::consts::EXE_SUFFIX
    ))
}

pub fn skip_binaries() -> bool {
    matches!(
        std::env::var("TURSO_GUI_SKIP_BINARIES").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

pub fn cargo() -> Command {
    let mut cmd = Command::new(env!("CARGO"));
    cmd.current_dir(workspace_root());
    cmd.env("CARGO_TERM_COLOR", "never");
    cmd
}

pub fn ensure_debug_bin(frontend: &Frontend) {
    let _guard = CARGO_LOCK.lock().expect("cargo lock");
    if debug_bin_path(frontend).is_file() {
        return;
    }
    let status = cargo()
        .args(["build", "-p", frontend.package, "--bin", frontend.bin])
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn cargo build for {}: {e}", frontend.id));
    assert!(
        status.success(),
        "cargo build -p {} failed with {status}",
        frontend.package
    );
    assert!(
        debug_bin_path(frontend).is_file(),
        "expected debug binary at {}",
        debug_bin_path(frontend).display()
    );
}

pub fn ensure_all_debug_bins() {
    if skip_binaries() {
        return;
    }
    BUILD_ONCE.call_once(|| {
        for frontend in FRONTENDS {
            ensure_debug_bin(frontend);
        }
    });
}

pub fn run_bin(frontend: &Frontend, args: &[&str]) -> CliOutput {
    ensure_debug_bin(frontend);
    let started = Instant::now();
    let output = Command::new(debug_bin_path(frontend))
        .args(args)
        .current_dir(workspace_root())
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("{} failed to spawn: {e}", frontend.bin));
    CliOutput {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        elapsed: started.elapsed(),
    }
}

pub fn debug_bin_size(frontend: &Frontend) -> u64 {
    ensure_debug_bin(frontend);
    fs::metadata(debug_bin_path(frontend))
        .map(|m| m.len())
        .unwrap_or(0)
}

pub fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes as f64 >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB)
    } else {
        format!("{:.1} KiB", bytes as f64 / KIB)
    }
}

pub fn unique_temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "turso-gui-eval-{}-{}-{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

pub fn seeded_model(dir: &Path, rows: u32) -> AppModel {
    let path = dir.join("bench.db");
    let mut model = AppModel::new(path.to_string_lossy().into_owned(), None, false);
    model
        .connect()
        .unwrap_or_else(|e| panic!("connect {}: {e}", path.display()));
    model.query =
        "CREATE TABLE people (id INTEGER PRIMARY KEY, name TEXT NOT NULL, city TEXT NOT NULL)"
            .into();
    model.execute_query().expect("create table");
    model.write_changes().expect("commit schema");

    let mut values = Vec::with_capacity(rows as usize);
    for i in 0..rows {
        values.push(format!("('name-{i}', 'city-{}')", i % 10));
    }
    model.query = format!("INSERT INTO people (name, city) VALUES {}", values.join(", "));
    model.execute_query().expect("insert rows");
    model.write_changes().expect("commit rows");
    model
        .browse_table_named("people")
        .expect("browse people");
    model
}

pub fn feature_matrix_table() -> String {
    let mut out = String::from("feature                 ");
    for frontend in FRONTENDS {
        out.push_str(&format!("{:<8}", frontend.id));
    }
    out.push('\n');
    for feature in REQUIRED_FEATURES {
        out.push_str(&format!("{:<24}", feature.name));
        for frontend in FRONTENDS {
            let src = crate_source(frontend);
            let present = feature_present(&src, feature);
            let cell = if present {
                "ok"
            } else if is_known_gap(frontend.id, feature.name) {
                "gap"
            } else {
                "MISS"
            };
            out.push_str(&format!("{cell:<8}"));
        }
        out.push('\n');
    }
    out
}
