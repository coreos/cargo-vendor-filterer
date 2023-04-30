use super::common::{tempdir, vendor, VendorFormat, VendorOptions};

#[test]
fn folder() {
    let (_td, mut test_folder) = tempdir().unwrap();
    test_folder.push("vendor");
    let output = vendor(VendorOptions {
        output: Some(&test_folder),
        format: Some(VendorFormat::Dir),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    assert!(test_folder.exists());
    assert!(test_folder.is_dir());
}

fn basic_tar_test(format: VendorFormat) {
    let (_td, mut test_folder) = tempdir().unwrap();
    test_folder.push(format!("vendor.{format}"));
    let output = vendor(VendorOptions {
        output: Some(&test_folder),
        format: Some(format),
        ..Default::default()
    })
    .unwrap();
    assert!(output.status.success());
    assert!(test_folder.exists());
    assert!(test_folder.is_file());
}

#[test]
fn tar() {
    basic_tar_test(VendorFormat::Tar);
}

#[test]
fn tar_gz() {
    basic_tar_test(VendorFormat::TarGz);
}

#[test]
fn tar_zstd() {
    basic_tar_test(VendorFormat::TarZstd);
}
