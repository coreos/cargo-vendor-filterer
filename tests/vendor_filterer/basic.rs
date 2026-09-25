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
    // The directory was replaced.
    assert!(!marker.exists());
    assert!(path.join("hex").exists());
}

/// A CI pipeline passes --overwrite on every run, whether the directory
/// exists or not.
#[test]
fn overwrite_repeatedly() {
    let (_td, test_folder) = tempdir().unwrap();
    let path = test_folder.join("vendor");
    for _ in 0..3 {
        let output = vendor(VendorOptions {
            output: Some(&path),
            overwrite: true,
            ..Default::default()
        })
        .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stderr}");
    }
    // Nothing is left next to the vendor directory.
    let entries: Vec<_> = test_folder
        .read_dir_utf8()
        .unwrap()
        .map(|e| e.unwrap().file_name().to_owned())
        .collect();
    assert_eq!(entries, ["vendor"]);
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
