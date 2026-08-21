use std::fs;
use std::time::{Duration, Instant};

use turso_gui_core::{AppModel, Tab, PAGE_SIZES};
use turso_gui_eval::{
    ensure_all_debug_bins, run_bin, seeded_model, skip_binaries, unique_temp_dir, FRONTENDS,
};

fn assert_within(label: &str, elapsed: Duration, budget: Duration) {
    eprintln!("  {label:<28} {elapsed:?} (budget {budget:?})");
    assert!(
        elapsed <= budget,
        "{label} took {elapsed:?}, budget {budget:?}"
    );
}

#[test]
fn shared_model_browse_filter_sort_page() {
    let dir = unique_temp_dir("browse");
    let rows = 200u32;

    let started = Instant::now();
    let mut model = seeded_model(&dir, rows);
    assert_within("seed+connect+browse", started.elapsed(), Duration::from_secs(20));

    assert!(model.is_connected());
    assert_eq!(model.browse_table.as_deref(), Some("people"));
    assert_eq!(model.browse_state.total_records, rows as u64);
    assert!(!model.browse_state.rows.is_empty());
    assert!(model.browse_state.headers.iter().any(|h| h == "name"));

    model.switch_tab(Tab::BrowseData);
    let started = Instant::now();
    model
        .set_browse_filter(1, "name-1".into())
        .expect("browse filter");
    assert_within("browse filter", started.elapsed(), Duration::from_secs(5));
    assert!(
        model
            .browse_state
            .rows
            .iter()
            .all(|row| row.get(1).is_some_and(|v| v.contains("name-1"))),
        "filter did not constrain name column: {:?}",
        model.browse_state.rows
    );

    let started = Instant::now();
    model.sort_column(1).expect("sort");
    assert_within("browse sort", started.elapsed(), Duration::from_secs(5));
    assert!(model.browse_state.sort.is_some());

    model.browse_state.filters.clear();
    model.change_page_size(PAGE_SIZES[0]).expect("page size 10");
    assert_eq!(model.browse_state.page_size, 10);
    assert!(model.browse_state.rows.len() <= 10);

    let started = Instant::now();
    model.change_page(1).expect("page 1");
    assert_within("change page", started.elapsed(), Duration::from_secs(5));
    assert_eq!(model.browse_state.page, 1);

    let started = Instant::now();
    model.toggle_all_results(true).expect("return all");
    assert_within("toggle all results", started.elapsed(), Duration::from_secs(5));
    assert!(model.browse_state.all_results);
    assert_eq!(model.browse_state.rows.len(), rows as usize);

    model.select_cell(0, 1);
    assert_eq!(model.browse_state.selected_cell, Some((0, 1)));
    assert!(!model.cell_editor.is_empty());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn shared_model_sql_and_transactions() {
    let dir = unique_temp_dir("sql");
    let mut model = seeded_model(&dir, 50);
    model.switch_tab(Tab::ExecuteSql);

    let started = Instant::now();
    model.query = "SELECT name, city FROM people ORDER BY name LIMIT 20".into();
    model.execute_query().expect("select");
    assert_within("sql select", started.elapsed(), Duration::from_secs(5));
    assert_eq!(model.sql_state.headers, ["name", "city"]);
    assert_eq!(model.sql_state.rows.len(), 20);

    model.query = "INSERT INTO people (name, city) VALUES ('tx-row', 'nowhere')".into();
    model.execute_query().expect("insert");
    assert!(model.has_changes);

    let started = Instant::now();
    model.revert_changes().expect("revert");
    assert_within("revert", started.elapsed(), Duration::from_secs(5));
    assert!(!model.has_changes);

    model.query = "SELECT COUNT(*) FROM people WHERE name = 'tx-row'".into();
    model.execute_query().expect("count after revert");
    let count = model.sql_state.rows.first().and_then(|r| r.first()).cloned();
    assert_eq!(count.as_deref(), Some("0"));

    model.query = "INSERT INTO people (name, city) VALUES ('kept-row', 'here')".into();
    model.execute_query().expect("insert kept");
    let started = Instant::now();
    model.write_changes().expect("write");
    assert_within("write", started.elapsed(), Duration::from_secs(5));
    assert!(!model.has_changes);

    let path = model.db_path.clone();
    model.close();
    assert!(!model.is_connected());
    model.db_path = path;
    model.connect().expect("reconnect");
    model.query = "SELECT COUNT(*) FROM people WHERE name = 'kept-row'".into();
    model.execute_query().expect("count after write");
    let count = model.sql_state.rows.first().and_then(|r| r.first()).cloned();
    assert_eq!(count.as_deref(), Some("1"));

    model.switch_tab(Tab::Structure);
    model
        .select_structure_table("people")
        .expect("structure");
    assert!(
        model
            .selected_table_columns
            .iter()
            .any(|c| c.name == "name")
    );
    assert!(model.structure_sql().to_uppercase().contains("CREATE"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn connect_rejects_remote_urls() {
    let mut model = AppModel::new("libsql://example.turso.io".into(), None, false);
    let err = model.connect().expect_err("remote should fail");
    assert!(
        err.to_string().to_lowercase().contains("remote"),
        "unexpected error: {err}"
    );
}

#[test]
fn help_startup_latency() {
    if skip_binaries() {
        eprintln!("skipping startup latency tests (TURSO_GUI_SKIP_BINARIES=1)");
        return;
    }
    ensure_all_debug_bins();

    eprintln!("\n--help startup");
    for frontend in FRONTENDS {
        let _warmup = run_bin(frontend, &["--help"]);
        let out = run_bin(frontend, &["--help"]);
        assert_eq!(out.status, 0, "{} --help failed", frontend.id);
        assert_within(
            &format!("{} --help", frontend.id),
            out.elapsed,
            Duration::from_secs(8),
        );
    }
}
