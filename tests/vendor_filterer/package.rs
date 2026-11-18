use camino::{Utf8Path, Utf8PathBuf};

use super::common::{
    tempdir, vendor, verify_crate_is_no_stub, verify_crate_is_stub, write_file_create_parents,
    VendorOptions,
};

/// A workspace where `shipped` and `tool` are built and `other` is not.
///
/// - `hex` is only enabled by `other`; `shipped` reaches it through the weak
///   `lib/std = ["hex?/std"]` feature, which must not enable it.
/// - `bitflags` is a default-only dependency of `shipped`.
/// - `once_cell` is behind `tool/extra`.
/// - `memchr` is only used by `other`.
fn workspace(dir: &Utf8Path, root_extra: &str) -> Utf8PathBuf {
    let manifest = write_file_create_parents(
        dir,
        "Cargo.toml",
        &format!(
            r#"
            [workspace]
            members = ["lib", "shipped", "tool", "other"]
            resolver = "2"
            {root_extra}
            "#
        ),
    )
    .unwrap();
    let members = [
        (
            "lib",
            r#"
            [package]
            name = "lib"
            version = "0.1.0"
            edition = "2021"

            [dependencies]
            hex = { version = "0.4", optional = true, default-features = false }

            [features]
            std = ["hex?/std"]
            "#,
        ),
        (
            "shipped",
            r#"
            [package]
            name = "shipped"
            version = "0.1.0"
            edition = "2021"

            [dependencies]
            lib = { path = "../lib", features = ["std"] }
            bitflags = { version = "1.3", optional = true }

            [features]
            default = ["dep:bitflags"]
            "#,
        ),
        (
            "tool",
            r#"
            [package]
            name = "tool"
            version = "0.1.0"
            edition = "2021"

            [dependencies]
            once_cell = { version = "1", optional = true }

            [features]
            extra = ["dep:once_cell"]
            "#,
        ),
        (
            "other",
            r#"
            [package]
            name = "other"
            version = "0.1.0"
            edition = "2021"

            [dependencies]
            lib = { path = "../lib", features = ["hex"] }
            memchr = "2"
            "#,
        ),
    ];
    for (member, contents) in members {
        write_file_create_parents(dir, &format!("{member}/Cargo.toml"), contents).unwrap();
        write_file_create_parents(dir, &format!("{member}/src/lib.rs"), "").unwrap();
    }
    manifest
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn selected_packages() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = workspace(&test_folder, "");
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest),
        packages: &["shipped", "tool"],
        features: &["tool/extra"],
        no_default_features: true,
        ..Default::default()
    })
    .unwrap();
    assert_success(&output);
    verify_crate_is_no_stub(&output_folder, "once_cell");
    verify_crate_is_stub(&output_folder, "bitflags");
    verify_crate_is_stub(&output_folder, "hex");
    verify_crate_is_stub(&output_folder, "memchr");
}

#[test]
fn feature_for_one_member_only_applies_to_it() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = workspace(&test_folder, "");
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest),
        packages: &["shipped", "tool"],
        features: &["tool/extra"],
        ..Default::default()
    })
    .unwrap();
    assert_success(&output);
    verify_crate_is_no_stub(&output_folder, "once_cell");
    verify_crate_is_no_stub(&output_folder, "bitflags");
    verify_crate_is_stub(&output_folder, "hex");
}

#[test]
fn dependency_feature_owned_by_one_selected_member() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = workspace(&test_folder, "");
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest),
        packages: &["shipped", "tool"],
        features: &["once_cell/std"],
        ..Default::default()
    })
    .unwrap();
    assert_success(&output);
    verify_crate_is_no_stub(&output_folder, "once_cell");
}

#[test]
fn without_selection_other_members_count() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = workspace(&test_folder, "");
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest),
        features: &["tool/extra"],
        no_default_features: true,
        ..Default::default()
    })
    .unwrap();
    assert_success(&output);
    verify_crate_is_no_stub(&output_folder, "hex");
    verify_crate_is_no_stub(&output_folder, "memchr");
}

#[test]
fn selected_packages_from_workspace_metadata() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = workspace(
        &test_folder,
        r#"
        [workspace.metadata.vendor-filter]
        packages = ["shipped"]
        no-default-features = true
        "#,
    );
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest),
        ..Default::default()
    })
    .unwrap();
    assert_success(&output);
    verify_crate_is_stub(&output_folder, "bitflags");
    verify_crate_is_stub(&output_folder, "hex");
    verify_crate_is_stub(&output_folder, "once_cell");
    verify_crate_is_stub(&output_folder, "memchr");
}

#[test]
fn selected_packages_with_sync_is_rejected() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = workspace(&test_folder, "");
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest),
        sync: vec![&manifest],
        packages: &["shipped"],
        ..Default::default()
    })
    .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be combined with --sync"));
    assert!(!output_folder.exists());
}
