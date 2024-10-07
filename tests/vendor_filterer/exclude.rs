use super::common::{tempdir, vendor, verify_no_windows, VendorOptions};

#[test]
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
