use crate::vendor_filterer::common::{verify_no_macos, verify_no_windows};

use super::common::{tempdir, vendor, write_file_create_parents, VendorOptions};

#[test]
fn basic_sync() {
    let (_td, test_folder) = tempdir().unwrap();
    let dep_a = test_folder.join("A");
    let dep_b = test_folder.join("B");
    let manifest_a = write_file_create_parents(
        &dep_a,
        "Cargo.toml",
        r#"
        [package]
        name = "foo"
        version = "0.1.0"

        [dependencies]
        bitflags = "1.3"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_a, "src/lib.rs", "").unwrap();
    let manifest_b = write_file_create_parents(
        &dep_b,
        "Cargo.toml",
        r#"
        [package]
        name = "bar"
        version = "0.1.0"

        [dependencies]
        hex = "0.4"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_b, "src/lib.rs", "").unwrap();
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest_a),
        sync: vec![&manifest_b],
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    let bitflags = output_folder.join("bitflags");
    assert!(bitflags.exists());
    let hex = output_folder.join("hex");
    assert!(hex.exists());
}

#[test]
fn sync_with_platform_filter() {
    let (_td, test_folder) = tempdir().unwrap();
    let dep_a = test_folder.join("A");
    let dep_b = test_folder.join("B");
    let manifest_a = write_file_create_parents(
        &dep_a,
        "Cargo.toml",
        r#"
        [package]
        name = "foo"
        version = "0.1.0"

        [dependencies]
        bitflags = "1.3"

        [target.'cfg(windows)'.dependencies]
        windows-sys = "*"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_a, "src/lib.rs", "").unwrap();
    let manifest_b = write_file_create_parents(
        &dep_b,
        "Cargo.toml",
        r#"
        [package]
        name = "bar"
        version = "0.1.0"

        [dependencies]
        hex = "0.4"

        [target.'cfg(macos)'.dependencies]
        core-foundation = "0.9"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_b, "src/lib.rs", "").unwrap();
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest_a),
        sync: vec![&manifest_b],
        platforms: Some(&["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    let bitflags = output_folder.join("bitflags");
    assert!(bitflags.exists());
    let hex = output_folder.join("hex");
    assert!(hex.exists());
    verify_no_macos(&output_folder);
    verify_no_windows(&output_folder);
}

#[test]
fn multiple_syncs() {
    let (_td, test_folder) = tempdir().unwrap();
    let dep_a = test_folder.join("A");
    let dep_b = test_folder.join("B");
    let dep_c = test_folder.join("C");
    let dep_d = test_folder.join("D");
    let manifest_a = write_file_create_parents(
        &dep_a,
        "Cargo.toml",
        r#"
        [package]
        name = "foo"
        version = "0.1.0"

        [dependencies]
        bitflags = "1.3"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_a, "src/lib.rs", "").unwrap();
    let manifest_b = write_file_create_parents(
        &dep_b,
        "Cargo.toml",
        r#"
        [package]
        name = "bar"
        version = "0.1.0"

        [dependencies]
        hex = "0.4"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_b, "src/lib.rs", "").unwrap();
    let manifest_c = write_file_create_parents(
        &dep_c,
        "Cargo.toml",
        r#"
        [package]
        name = "cez"
        version = "0.1.0"

        [dependencies]
        toml = "0.4"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_c, "src/lib.rs", "").unwrap();
    let manifest_d = write_file_create_parents(
        &dep_d,
        "Cargo.toml",
        r#"
        [package]
        name = "dav"
        version = "0.1.0"

        [dependencies]
        anyhow = "*"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_d, "src/lib.rs", "").unwrap();
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest_a),
        sync: vec![&manifest_b, &manifest_c, &manifest_d],
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    let bitflags = output_folder.join("bitflags");
    assert!(bitflags.exists());
    let hex = output_folder.join("hex");
    assert!(hex.exists());
    let toml = output_folder.join("toml");
    assert!(toml.exists());
    let anyhow = output_folder.join("anyhow");
    assert!(anyhow.exists());
}

#[test]
fn sync_platform_with_exclude() {
    let (_td, test_folder) = tempdir().unwrap();
    let dep_a = test_folder.join("A");
    let dep_b = test_folder.join("B");
    let manifest_a = write_file_create_parents(
        &dep_a,
        "Cargo.toml",
        r#"
        [package]
        name = "foo"
        version = "0.1.0"

        [dependencies]
        bitflags = "1.3"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_a, "src/lib.rs", "").unwrap();
    let manifest_b = write_file_create_parents(
        &dep_b,
        "Cargo.toml",
        r#"
        [package]
        name = "bar"
        version = "0.1.0"

        [dependencies]
        hex = "0.4"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_b, "src/lib.rs", "").unwrap();
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest_a),
        sync: vec![&manifest_b],
        exclude_crate_paths: Some(&["hex#benches"]),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    let bitflags = output_folder.join("bitflags");
    assert!(bitflags.exists());
    let hex = output_folder.join("hex").join("benches");
    assert!(!hex.exists());
}

#[test]
fn filter_without_manifest_path() {
    let (_td, test_folder) = tempdir().unwrap();
    let dep_a = test_folder.join("A");
    let _manifest_a = write_file_create_parents(
        &dep_a,
        "Cargo.toml",
        r#"
        [package]
        name = "foo"
        version = "0.1.0"

        [dependencies]
        bitflags = "1.3"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_a, "src/lib.rs", "").unwrap();
    assert!(std::env::set_current_dir(&dep_a).is_ok());
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    let bitflags = output_folder.join("bitflags");
    assert!(bitflags.exists());
    let bitflags_lib = bitflags.join("src/lib.rs");
    assert!(bitflags_lib.exists());
    // Check that this was not filtered out
    assert_ne!(bitflags_lib.metadata().unwrap().len(), 0);
}

#[test]
fn filter_without_manifest_but_sync() {
    let (_td, test_folder) = tempdir().unwrap();
    let dep_a = test_folder.join("A");
    let dep_b = test_folder.join("B");
    let _manifest_a = write_file_create_parents(
        &dep_a,
        "Cargo.toml",
        r#"
        [package]
        name = "foo"
        version = "0.1.0"

        [dependencies]
        bitflags = "1.3"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_a, "src/lib.rs", "").unwrap();
    let manifest_b = write_file_create_parents(
        &dep_b,
        "Cargo.toml",
        r#"
        [package]
        name = "bar"
        version = "0.1.0"

        [dependencies]
        hex = "0.4"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_b, "src/lib.rs", "").unwrap();
    assert!(std::env::set_current_dir(&dep_a).is_ok());
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        sync: vec![&manifest_b],
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    let bitflags = output_folder.join("bitflags");
    assert!(bitflags.exists());
    let bitflags_lib = bitflags.join("src/lib.rs");
    assert!(bitflags_lib.exists());
    // Check that this was not filtered out
    assert_ne!(bitflags_lib.metadata().unwrap().len(), 0);
    let hex = output_folder.join("hex");
    assert!(hex.exists());
    let hex_lib = hex.join("src/lib.rs");
    assert!(hex_lib.exists());
    // Check that this was not filtered out
    assert_ne!(hex_lib.metadata().unwrap().len(), 0);
}
