use turso_gui_eval::{
    debug_bin_size, ensure_all_debug_bins, format_bytes, rust_line_count, rust_sources,
    skip_binaries, frontend_dir, FRONTENDS,
};

#[test]
fn source_size_is_nontrivial_for_every_frontend() {
    eprintln!("\nsource size");
    eprintln!("  {:>8}  {:>8}  {:>6}", "id", "files", "loc");
    for frontend in FRONTENDS {
        let files = rust_sources(&frontend_dir(frontend)).len();
        let loc = rust_line_count(frontend);
        eprintln!("  {:>8}  {:>8}  {:>6}", frontend.id, files, loc);
        assert!(
            files >= 1,
            "{} has no .rs files",
            frontend.id
        );
        assert!(
            loc >= 80,
            "{} source is too small ({loc} lines)",
            frontend.id
        );
        assert!(
            loc < 50_000,
            "{} source is unexpectedly huge ({loc} lines)",
            frontend.id
        );
    }
}

#[test]
fn debug_binary_sizes() {
    if skip_binaries() {
        eprintln!("skipping debug binary size tests (TURSO_GUI_SKIP_BINARIES=1)");
        return;
    }
    ensure_all_debug_bins();

    eprintln!("\ndebug binary size");
    eprintln!("  {:>8}  {:>12}  {}", "id", "bytes", "pretty");
    let mut sizes = Vec::new();
    for frontend in FRONTENDS {
        let bytes = debug_bin_size(frontend);
        eprintln!(
            "  {:>8}  {:>12}  {}",
            frontend.id,
            bytes,
            format_bytes(bytes)
        );
        assert!(
            bytes > 100_000,
            "{} debug binary is too small ({})",
            frontend.id,
            format_bytes(bytes)
        );
        assert!(
            bytes < 800_000_000,
            "{} debug binary is too large ({})",
            frontend.id,
            format_bytes(bytes)
        );
        sizes.push((frontend.id, bytes));
    }

    sizes.sort_by_key(|(_, n)| *n);
    eprintln!(
        "  smallest {} ({}), largest {} ({})",
        sizes[0].0,
        format_bytes(sizes[0].1),
        sizes.last().unwrap().0,
        format_bytes(sizes.last().unwrap().1)
    );
}
