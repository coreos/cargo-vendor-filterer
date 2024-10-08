use super::common::{tempdir, vendor, write_file_create_parents, VendorOptions};

#[test]
fn manifest_path() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = write_file_create_parents(
        &test_folder,
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
    write_file_create_parents(&test_folder, "src/lib.rs", "").unwrap();
    let output_folder = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&output_folder),
        manifest_path: Some(&manifest),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    let bitflags = output_folder.join("bitflags");
    assert!(bitflags.exists());
}

#[test]
fn metadata() {
    let (_td, test_folder) = tempdir().unwrap();
    let manifest = write_file_create_parents(
        &test_folder,
        "Cargo.toml",
        r#"
        [package]    
        name = "foo"
        version = "0.1.0"

        [dependencies]
        hex = "0.4"
        libz-sys = "1.1.16"

        [package.metadata.vendor-filter]
        exclude-crate-paths = [ { name = "hex", exclude = "benches" }, { name = "libz-sys", exclude = "src/smoke.c" } ]
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
    if !output.status.success() {
        let _ = std::io::copy(
            &mut std::io::Cursor::new(&output.stderr),
            &mut std::io::stderr(),
        );
    }
    assert!(output.status.success());
    let hex = output_folder.join("hex");
    assert!(hex.exists());
    assert!(!hex.join("benches").exists());
}
