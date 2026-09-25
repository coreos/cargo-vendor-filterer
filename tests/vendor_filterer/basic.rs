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
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--overwrite"), "{stderr}");
}

#[test]
fn overwrite_existing_folder() {
    let (_td, test_folder) = tempdir().unwrap();
    let path = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&path),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    let marker = path.join("marker");
    fs::write(&marker, "from the first run").unwrap();

    let output = vendor(VendorOptions {
        output: Some(&path),
        overwrite: true,
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    // The directory was replaced, and nothing is left next to it.
    assert!(!marker.exists());
    assert!(path.join("hex").exists());
    assert!(!test_folder.join(".vendor.pre-overwrite").exists());
}

#[test]
fn overwrite_keeps_folder_on_failure() {
    let (_td, test_folder) = tempdir().unwrap();
    let path = test_folder.join("vendor");
    let output = vendor(VendorOptions {
        output: Some(&path),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    let marker = path.join("marker");
    fs::write(&marker, "from the first run").unwrap();

    // Writing the report into a missing directory fails only after cargo
    // vendor has run.
    let report = test_folder.join("missing/report.json");
    let output = vendor(VendorOptions {
        output: Some(&path),
        overwrite: true,
        json: Some(&report),
        ..Default::default()
    })
    .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("report.json"), "{stderr}");
    // The directory is back as it was.
    assert_eq!(fs::read_to_string(&marker).unwrap(), "from the first run");
    assert!(path.join("hex").exists());
    assert!(!test_folder.join(".vendor.pre-overwrite").exists());
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
