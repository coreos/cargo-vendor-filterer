use super::common::{tempdir, vendor, verify_crate_is_no_stub, verify_no_windows, VendorOptions};

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
fn linux_and_windows_with_dep_kind_filter() {
    let (_td, mut test_folder) = tempdir().unwrap();
    test_folder.push("vendor");
    let output = vendor(VendorOptions {
        output: Some(&test_folder),
        platforms: Some(&["x86_64-unknown-linux-gnu", "x86_64-pc-windows-gnu"]),
        keep_dep_kinds: Some("no-dev"),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    // A package needed by one platform only has to survive the dependency kind
    // filtering of the other platforms: anstyle-wincon is a normal dependency
    // on windows only, libc on linux only.
    verify_crate_is_no_stub(&test_folder, "anstyle-wincon");
    verify_crate_is_no_stub(&test_folder, "libc");
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
