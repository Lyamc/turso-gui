use turso_gui_eval::{
    ensure_all_debug_bins, run_bin, skip_binaries, FRONTENDS, REQUIRED_HELP_FLAGS,
};

fn require_binaries() -> bool {
    if skip_binaries() {
        eprintln!("skipping binary CLI tests (TURSO_GUI_SKIP_BINARIES=1)");
        false
    } else {
        ensure_all_debug_bins();
        true
    }
}

#[test]
fn every_frontend_prints_help_with_shared_flags() {
    if !require_binaries() {
        return;
    }

    eprintln!("\n--help");
    for frontend in FRONTENDS {
        let out = run_bin(frontend, &["--help"]);
        let text = format!("{}\n{}", out.stdout, out.stderr);
        eprintln!(
            "  {:>8}  status={}  {:>8?}",
            frontend.id, out.status, out.elapsed
        );
        assert_eq!(
            out.status, 0,
            "{} --help exited {}: {}",
            frontend.id, out.status, text
        );
        for flag in REQUIRED_HELP_FLAGS {
            assert!(
                text.contains(flag),
                "{} --help missing {flag}\n{text}",
                frontend.id
            );
        }
        assert!(
            text.contains("-d") && text.contains("-t") && text.contains("-D"),
            "{} --help missing short flags -d/-t/-D\n{text}",
            frontend.id
        );
    }
}

#[test]
fn every_frontend_prints_version() {
    if !require_binaries() {
        return;
    }

    for frontend in FRONTENDS {
        let out = run_bin(frontend, &["--version"]);
        let text = format!("{}\n{}", out.stdout, out.stderr);
        assert_eq!(
            out.status, 0,
            "{} --version exited {}: {}",
            frontend.id, out.status, text
        );
        assert!(
            text.contains("0.1.0") || text.contains(frontend.bin),
            "{} --version output unexpected:\n{text}",
            frontend.id
        );
    }
}

#[test]
fn iced_exposes_cli_mode_flags() {
    if !require_binaries() {
        return;
    }
    let iced = FRONTENDS.iter().find(|f| f.id == "iced").unwrap();
    let out = run_bin(iced, &["--help"]);
    let text = format!("{}\n{}", out.stdout, out.stderr);
    assert!(text.contains("--cli"), "iced --help missing --cli\n{text}");
    assert!(
        text.contains("--command"),
        "iced --help missing --command\n{text}"
    );
}

#[test]
fn unknown_flag_is_rejected() {
    if !require_binaries() {
        return;
    }
    for frontend in FRONTENDS {
        let out = run_bin(frontend, &["--not-a-real-flag"]);
        assert_ne!(
            out.status, 0,
            "{} accepted an unknown flag",
            frontend.id
        );
    }
}
