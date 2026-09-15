use super::common::{tempdir, vendor, verify_no_windows, write_file_create_parents, VendorOptions};

#[test]
#[serial_test::parallel]
fn linux_multiple_platforms() {
    let (_td, mut test_folder) = tempdir().unwrap();
    test_folder.push("vendor");
    let output = vendor(VendorOptions {
        output: Some(&test_folder),
        platforms: Some(&["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]),
        exclude_crate_paths: Some(&["hex#benches", "*#tests"]),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    verify_no_windows(&test_folder);
    test_folder.push("hex/benches");
    assert!(!test_folder.exists());
    test_folder.push("../tests");
    assert!(!test_folder.exists());
}

#[test]
#[serial_test::parallel]
fn windows_with_dep_kind_filter_normal() {
    let (_td, mut test_folder) = tempdir().unwrap();
    test_folder.push("vendor-test2");
    let output = vendor(VendorOptions {
        output: Some(&test_folder),
        platforms: Some(&["x86_64-pc-windows-gnu"]),
        keep_dep_kinds: Some("normal"),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    test_folder.push("serial_test/tests"); // crate replaced with a stub, so tests folder is removed
    assert!(!test_folder.exists());
    test_folder.push("../openssl/examples"); // openssl removed because defined only for non-windows
    assert!(!test_folder.exists());
}

#[test]
#[serial_test::parallel]
fn exclude_with_glob_patterns() {
    let (_td, mut test_folder) = tempdir().unwrap();
    test_folder.push("vendor");
    let output = vendor(VendorOptions {
        output: Some(&test_folder),
        platforms: Some(&["x86_64-unknown-linux-gnu"]),
        exclude_crate_paths: Some(&["hex#*.md", "*#benches", "libz-sys#src/*.c"]),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    let hex_dir = test_folder.join("hex");
    assert!(hex_dir.exists());
    assert!(!hex_dir.join("README.md").exists());
    assert!(!hex_dir.join("CHANGELOG.md").exists());
    assert!(!hex_dir.join("benches").exists());
    // Check that .c files were removed from libz-sys if it exists
    let libz_sys_dir = test_folder.join("libz-sys");
    if libz_sys_dir.exists() {
        let src_dir = libz_sys_dir.join("src");
        if src_dir.exists() {
            for entry in src_dir.read_dir_utf8().unwrap() {
                let entry = entry.unwrap();
                assert!(entry.path().extension() != Some("c"));
            }
        }
    }
}

/// Regression test: --exclude-crate-path must work when --versioned-dirs is also active.
///
/// With --versioned-dirs, cargo vendor names directories as `<name>-<version>` even when there
/// is only one version of a crate.  The exclude lookup must still match against the bare crate
/// name supplied by the user (e.g. `hex#benches`) and resolve it to the versioned directory
/// name on disk (e.g. `hex-0.4.3`).
#[test]
#[serial_test::parallel]
fn exclude_with_versioned_dirs_single_version() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = write_file_create_parents(
        &test_folder,
        "Cargo.toml",
        r#"
        [package]
        name = "foo"
        version = "0.1.0"

        [dependencies]
        hex = "0.4.3"
    "#,
    )
    .unwrap();
    write_file_create_parents(&test_folder, "src/lib.rs", "").unwrap();
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest),
        versioned_dirs: true,
        exclude_crate_paths: Some(&["hex#benches"]),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    // With --versioned-dirs the crate is under hex-0.4.3/, not hex/
    let hex_dir = output_folder.join("hex-0.4.3");
    assert!(hex_dir.exists(), "hex-0.4.3 directory should exist");
    assert!(
        !hex_dir.join("benches").exists(),
        "hex-0.4.3/benches should have been excluded"
    );
}

/// Same as above but with two versions of the same crate to exercise the multi-version path.
#[test]
#[serial_test::parallel]
fn exclude_with_versioned_dirs_multiple_versions() {
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
        hex = "0.4.3"
        bar = { path="../B/" }
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_a, "src/lib.rs", "").unwrap();
    write_file_create_parents(
        &dep_b,
        "Cargo.toml",
        r#"
        [package]
        name = "bar"
        version = "0.1.0"

        [dependencies]
        hex = "0.3.2"
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_b, "src/lib.rs", "").unwrap();
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest_a),
        versioned_dirs: true,
        exclude_crate_paths: Some(&["hex#benches"]),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    // Both versioned hex directories should exist and have benches excluded.
    for hex_dir_name in &["hex-0.4.3", "hex-0.3.2"] {
        let hex_dir = output_folder.join(hex_dir_name);
        assert!(hex_dir.exists(), "{hex_dir_name} directory should exist");
        assert!(
            !hex_dir.join("benches").exists(),
            "{hex_dir_name}/benches should have been excluded"
        );
    }
}
