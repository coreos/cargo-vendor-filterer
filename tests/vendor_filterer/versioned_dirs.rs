use super::common::{
    tempdir, vendor, verify_crate_is_no_stub, write_file_create_parents, VendorOptions,
};

#[test]
fn multiple_versions_without_flag() {
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
        bitflags = "1.3.2"
        hex = "0.4.3"
        bar = { path="../B/" }
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_a, "src/lib.rs", "").unwrap();
    let _manifest_b = write_file_create_parents(
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
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    verify_crate_is_no_stub(&output_folder, "bitflags");
    verify_crate_is_no_stub(&output_folder, "hex");
    verify_crate_is_no_stub(&output_folder, "hex-0.3.2");
}

#[test]
fn only_one_version() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = write_file_create_parents(
        &test_folder,
        "Cargo.toml",
        r#"
        [package]
        name = "foo"
        version = "0.1.0"

        [dependencies]
        bitflags = "1.3.2"
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
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    verify_crate_is_no_stub(&output_folder, "bitflags-1.3.2");
    verify_crate_is_no_stub(&output_folder, "hex-0.4.3");
}

#[test]
fn multiple_versions() {
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
        bitflags = "1.3.2"
        hex = "0.4.3"
        bar = { path="../B/" }
    "#,
    )
    .unwrap();
    write_file_create_parents(&dep_a, "src/lib.rs", "").unwrap();
    let _manifest_b = write_file_create_parents(
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
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    verify_crate_is_no_stub(&output_folder, "bitflags-1.3.2");
    verify_crate_is_no_stub(&output_folder, "hex-0.4.3");
    verify_crate_is_no_stub(&output_folder, "hex-0.3.2");
}
