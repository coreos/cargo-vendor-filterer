use super::common::{tempdir, vendor, verify_no_windows, VendorOptions};

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
