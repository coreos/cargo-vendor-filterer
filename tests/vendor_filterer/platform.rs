use super::common::{tempdir, vendor, verify_no_windows, VendorOptions};

#[test]
fn linux() {
    let (_td, mut test_folder) = tempdir().unwrap();
    test_folder.push("vendor");
    let output = vendor(VendorOptions {
        output: Some(&test_folder),
        platforms: Some(&["x86_64-unknown-linux-gnu"]),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    verify_no_windows(&test_folder);
}

#[test]
fn linux_multiple() {
    let (_td, mut test_folder) = tempdir().unwrap();
    test_folder.push("vendor");
    let output = vendor(VendorOptions {
        output: Some(&test_folder),
        platforms: Some(&["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    verify_no_windows(&test_folder);
}

#[test]
fn linux_glob() {
    let (_td, mut test_folder) = tempdir().unwrap();
    test_folder.push("vendor");
    let output = vendor(VendorOptions {
        output: Some(&test_folder),
        platforms: Some(&["*-unknown-linux-gnu"]),
        tier: Some("2"),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    verify_no_windows(&test_folder);
}
