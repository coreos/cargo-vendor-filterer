use super::common::{
    tempdir, vendor, verify_crate_is_no_stub, write_file_create_parents, VendorOptions,
};

fn verify_crate_is_stub(output_folder: &camino::Utf8Path, name: &str) {
    let crate_dir = output_folder.join(name);
    let crate_lib = crate_dir.join("src/lib.rs");
    assert!(
        crate_lib.exists(),
        "Package does not show up in the vendor dir"
    );
    assert_eq!(
        crate_lib.metadata().unwrap().len(),
        0,
        "Package was retained instead of being replaced with a stub"
    );
    assert_eq!(crate_dir.read_dir_utf8().unwrap().count(), 3);
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
