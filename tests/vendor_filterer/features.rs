use super::common::{
    tempdir, vendor, verify_crate_is_no_stub, verify_crate_is_stub, write_file_create_parents,
    VendorOptions,
};
use std::process::Command;

fn tree_contains_package(tree: &str, package: &str) -> bool {
    tree.lines().any(|line| {
        line.split_once(" v")
            .is_some_and(|(name, _)| name == package)
    })
}

#[test]
fn native_tls_feature_excludes_rustls_and_ring_on_linux() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = write_file_create_parents(
        &test_folder,
        "Cargo.toml",
        r#"
        [package]
        name = "reqwest-feature-filter-test"
        version = "0.1.0"
        edition = "2021"

        [dependencies]
        reqwest = { version = "=0.12.28", default-features = false }

        [features]
        native-tls = ["reqwest/default-tls"]
        rustls = ["reqwest/rustls-tls"]

        [package.metadata.vendor-filter]
        platforms = ["x86_64-unknown-linux-gnu"]
        features = ["native-tls"]
    "#,
    )
    .unwrap();
    write_file_create_parents(&test_folder, "src/lib.rs", "").unwrap();

    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest),
        ..Default::default()
    })
    .unwrap();

    assert!(
        output.status.success(),
        "vendor-filterer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    verify_crate_is_no_stub(&output_folder, "native-tls");
    verify_crate_is_stub(&output_folder, "rustls");
    verify_crate_is_stub(&output_folder, "ring");
}

#[test]
fn disabled_transitive_feature_does_not_retain_ring() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = write_file_create_parents(
        &test_folder,
        "Cargo.toml",
        r#"
        [package]
        name = "rustls-webpki-feature-filter-test"
        version = "0.1.0"
        edition = "2021"

        [dependencies]
        rustls-webpki = { version = "=0.103.6", default-features = false }

        [features]
        aws-lc = ["rustls-webpki/aws-lc-rs"]
        ring = ["rustls-webpki/ring"]

        [package.metadata.vendor-filter]
        platforms = ["x86_64-unknown-linux-gnu"]
        no-default-features = true
        features = ["aws-lc"]
    "#,
    )
    .unwrap();
    write_file_create_parents(&test_folder, "src/lib.rs", "").unwrap();

    let selected_tree = Command::new("cargo")
        .args([
            "tree",
            "--prefix",
            "none",
            "--no-default-features",
            "--features",
            "aws-lc",
        ])
        .current_dir(&test_folder)
        .output()
        .unwrap();
    assert!(
        selected_tree.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&selected_tree.stderr)
    );
    let selected_tree =
        String::from_utf8(selected_tree.stdout).expect("cargo tree stdout is not UTF-8");
    assert!(!tree_contains_package(&selected_tree, "ring"));
    assert!(tree_contains_package(&selected_tree, "aws-lc-rs"));

    let all_features_tree = Command::new("cargo")
        .args(["tree", "--prefix", "none", "--all-features"])
        .current_dir(&test_folder)
        .output()
        .unwrap();
    assert!(
        all_features_tree.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&all_features_tree.stderr)
    );
    let all_features_tree =
        String::from_utf8(all_features_tree.stdout).expect("cargo tree stdout is not UTF-8");
    assert!(tree_contains_package(&all_features_tree, "ring"));

    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest),
        ..Default::default()
    })
    .unwrap();

    assert!(
        output.status.success(),
        "vendor-filterer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    verify_crate_is_no_stub(&output_folder, "rustls-webpki");
    verify_crate_is_no_stub(&output_folder, "aws-lc-rs");
    verify_crate_is_stub(&output_folder, "ring");
}

#[test]
fn all_features_retains_all_feature_dependencies() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = write_file_create_parents(
        &test_folder,
        "Cargo.toml",
        r#"
        [package]
        name = "all-features-filter-test"
        version = "0.1.0"
        edition = "2021"

        [dependencies]
        rustls-webpki = { version = "=0.103.6", default-features = false }

        [features]
        aws-lc = ["rustls-webpki/aws-lc-rs"]
        ring = ["rustls-webpki/ring"]

        [package.metadata.vendor-filter]
        platforms = ["x86_64-unknown-linux-gnu"]
        all-features = true
    "#,
    )
    .unwrap();
    write_file_create_parents(&test_folder, "src/lib.rs", "").unwrap();

    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest),
        ..Default::default()
    })
    .unwrap();

    assert!(
        output.status.success(),
        "vendor-filterer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // `all-features = true` is an explicit resolved-graph request, not merely
    // an instruction for the catalog query.
    verify_crate_is_no_stub(&output_folder, "aws-lc-rs");
    verify_crate_is_no_stub(&output_folder, "ring");
}
