use std::collections::BTreeSet;
use std::fs;

use cargo_vendor_filterer::CARGO_TOML_PRE_VENDOR_FILTER;

use super::common::{is_stub, tempdir, vendor, VendorOptions};

#[test]
#[serial_test::parallel]
fn json_report_lists_the_stubs() {
    let (_td, mut test_folder) = tempdir().unwrap();
    let report_path = test_folder.join("report.json");
    test_folder.push("vendor");
    let output = vendor(VendorOptions {
        output: Some(&test_folder),
        platforms: Some(&["x86_64-unknown-linux-gnu"]),
        json: Some(&report_path),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());

    let report = fs::read_to_string(&report_path).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    let stubs: BTreeSet<String> = report["stubs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_owned())
        .collect();
    assert!(!stubs.is_empty());

    // Exactly the packages marked as a stub are reported, and each of them
    // keeps the manifest it was generated from.
    for entry in test_folder.read_dir_utf8().unwrap() {
        let dir = entry.unwrap().file_name().to_owned();
        assert_eq!(
            is_stub(&test_folder, &dir),
            stubs.contains(&dir),
            "{dir} is reported wrongly"
        );
    }
    for dir in &stubs {
        let original = test_folder.join(dir).join(CARGO_TOML_PRE_VENDOR_FILTER);
        assert!(original.exists(), "{dir} kept no pre-filter manifest");
    }
}
