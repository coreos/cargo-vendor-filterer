use super::common::{project_root, tempdir, vendor, VendorOptions};
use std::fs;

#[test]
fn do_not_vendor_if_folder_exists() {
    let (_test_folder, path) = tempdir().unwrap();
    let output = vendor(VendorOptions {
        output: Some(&path),
        ..Default::default()
    })
    .unwrap();
    assert!(!output.status.success());
}

#[test]
fn default_output_folder() {
    let mut root = project_root().unwrap();
    root.push("vendor");
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    let output = vendor(VendorOptions::default()).unwrap();
    assert!(output.status.success());
    assert!(root.exists());
    assert!(root.is_dir());
    fs::remove_dir_all(&root).unwrap();
}
