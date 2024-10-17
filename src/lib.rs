/// The path we use in Cargo.toml i.e. `package.metadata.vendor-filter`
pub const CONFIG_KEY: &str = "vendor-filter";
/// The name of our binary
pub const SELF_NAME: &str = "vendor-filterer";
/// The default directory path
pub const VENDOR_DEFAULT_PATH: &str = "vendor";
/// The default path for --format=tar
pub const VENDOR_DEFAULT_PATH_TAR: &str = "vendor.tar";
/// The default path for --format=tar.zstd
pub const VENDOR_DEFAULT_PATH_TAR_ZSTD: &str = "vendor.tar.zstd";
/// The default path for --format=tar.gz
pub const VENDOR_DEFAULT_PATH_TAR_GZ: &str = "vendor.tar.gz";
/// The name of the Cargo.toml file
pub const CARGO_TOML: &str = "Cargo.toml";
/// The filename cargo writes in packages with file checksums
pub const CARGO_CHECKSUM: &str = ".cargo-checksum.json";
/// The CLI argument passed to cargo to work offline
pub const OFFLINE: &str = "--offline";
/// The CLI argument passed to cargo to work with multiple Cargo.toml-files
pub const SYNC: &str = "--sync";
/// The CLI argument passed to `cargo vendor` to respect override of the `crates.io` source when downloading crates
pub const RESPECT_SOURCE_CONFIG: &str = "--respect-source-config";
/// The CLI argument passed to `cargo vendor` to always include version in subdir name
pub const VERSIONED_DIRS: &str = "--versioned-dirs";
/// The package entry
pub const MANIFEST_KEY_PACKAGE: &str = "package";
/// Extra targets which we need to remove because Cargo validates them and will
/// error out when we've replaced the library with a stub.
pub const UNWANTED_MANIFEST_KEYS: &[&str] = &["bin", "example", "test", "bench"];
/// Cargo also checks these keys in the package section
pub const UNWANTED_PACKAGE_KEYS: &[&str] = &["links", "build"];
