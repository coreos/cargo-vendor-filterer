use super::common::{tempdir, vendor, VendorFormat, VendorOptions};
use anyhow::Result;

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

#[cfg(test)]
/// Compute the SHA-256 digest of the buffer and return the result in hexadecimal format
fn sha256_hexdigest(buf: &[u8]) -> Result<String> {
    // NOTE: Keep this in sync with the copy in the main binary
    #[cfg(feature = "openssl")]
    {
        let digest = openssl::hash::hash(openssl::hash::MessageDigest::sha256(), buf)?;
        Ok(hex::encode(digest))
    }
    #[cfg(not(feature = "openssl"))]
    {
        use sha2::Digest;
        let digest = sha2::Sha256::digest(buf);
        Ok(hex::encode(digest))
    }
}

#[cfg(test)]
fn basic_tar_test(format: VendorFormat) {
    let (_td, mut test_folder) = tempdir().unwrap();
    test_folder.push(format!("vendor.{format}"));
    let output = vendor(VendorOptions {
        output: Some(&test_folder),
        format: Some(format),
        ..Default::default()
    })
    .unwrap();
    if !output.status.success() {
        let _ = std::io::copy(
            &mut std::io::Cursor::new(output.stderr),
            &mut std::io::stderr().lock(),
        );
    }
    assert!(output.status.success());
    assert!(test_folder.exists());
    assert!(test_folder.is_file());
    let contents = std::fs::read(&test_folder).unwrap();
    let original_digest = sha256_hexdigest(&contents).unwrap();
    drop(contents);
    std::fs::remove_file(&test_folder).unwrap();
    let output = vendor(VendorOptions {
        output: Some(&test_folder),
        format: Some(format),
        ..Default::default()
    })
    .unwrap();
    if !output.status.success() {
        let _ = std::io::copy(
            &mut std::io::Cursor::new(output.stderr),
            &mut std::io::stderr().lock(),
        );
    }
    let contents = std::fs::read(&test_folder).unwrap();
    let rerun_digest = sha256_hexdigest(&contents).unwrap();
    assert_eq!(&original_digest, &rerun_digest);
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
#[cfg(not(windows))]
fn tar_zstd() {
    basic_tar_test(VendorFormat::TarZstd);
}
