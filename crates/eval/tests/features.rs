use std::fs;

use turso_gui_eval::{
    catalog_crate_names, crate_source, feature_matrix_table, feature_present, frontend_dir,
    gui_crate_names, missing_features, Feature, FRONTENDS, REQUIRED_FEATURES,
};

#[test]
fn catalog_covers_every_gui_crate() {
    assert_eq!(
        catalog_crate_names(),
        gui_crate_names(),
        "FRONTENDS is out of date with crates/"
    );
}

#[test]
fn every_frontend_is_a_workspace_package_with_src() {
    for frontend in FRONTENDS {
        let dir = frontend_dir(frontend);
        assert!(dir.is_dir(), "{}: missing {}", frontend.id, dir.display());
        assert!(
            dir.join("Cargo.toml").is_file(),
            "{}: missing Cargo.toml",
            frontend.id
        );
        assert!(
            dir.join("src").join("main.rs").is_file(),
            "{}: missing src/main.rs",
            frontend.id
        );
        let manifest = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(
            manifest.contains(frontend.package),
            "{}: Cargo.toml does not name package {}",
            frontend.id,
            frontend.package
        );
        assert!(
            manifest.contains("turso-gui-core"),
            "{}: does not depend on turso-gui-core",
            frontend.id
        );
        assert!(
            manifest.contains(&format!("name = \"{}\"", frontend.bin))
                || manifest.contains(&format!("name = '{}'", frontend.bin)),
            "{}: binary name {} not declared",
            frontend.id,
            frontend.bin
        );
    }
}

#[test]
fn feature_parity_matrix() {
    eprintln!("\n{}", feature_matrix_table());

    let mut unexpected = Vec::new();
    for frontend in FRONTENDS {
        for name in missing_features(frontend) {
            unexpected.push(format!("{}: {name}", frontend.id));
        }
    }
    assert!(
        unexpected.is_empty(),
        "unexpected frontend feature gaps: {unexpected:?}\n\n{}",
        feature_matrix_table()
    );
}

#[test]
fn every_frontend_uses_shared_app_model() {
    for frontend in FRONTENDS {
        let src = crate_source(frontend);
        assert!(
            src.contains("AppModel"),
            "{} does not use AppModel",
            frontend.id
        );
        assert!(
            src.contains("turso_gui_core"),
            "{} does not import turso_gui_core",
            frontend.id
        );
    }
}

#[test]
fn every_required_feature_is_implemented_somewhere() {
    for feature in REQUIRED_FEATURES {
        let any = FRONTENDS.iter().any(|frontend| {
            let src = crate_source(frontend);
            feature_present(&src, feature)
        });
        assert!(any, "feature {} is not implemented by any frontend", feature.name);
    }
}

#[test]
fn clap_flags_are_consistent() {
    let database = Feature::new("database short", &["#[arg(short, long)]"]);
    for frontend in FRONTENDS {
        let src = crate_source(frontend);
        assert!(
            src.contains("struct Args"),
            "{} is missing clap Args",
            frontend.id
        );
        assert!(
            feature_present(&src, &database) || src.contains("short = 'd'"),
            "{} is missing a short/long database flag",
            frontend.id
        );
    }
}
