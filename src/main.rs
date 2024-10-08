use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{
    CargoOpt::{AllFeatures, NoDefaultFeatures, SomeFeatures},
    MetadataCommand, Package,
};
use clap::Parser;
use either::Either;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{BufReader, Write};
use std::process::Command;
use std::vec;

use cargo_vendor_filterer::*;

mod tiers;

/// This is the .cargo-checksum.json in a crate/package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CargoChecksums {
    files: BTreeMap<String, String>,
    package: Option<String>,
}

/// The minimal bits of Cargo.toml we need.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CargoManifest {
    package: CargoPackage,
    features: BTreeMap<String, Vec<String>>,
}

/// The minimal bits of the `[package]` section in Cargo.toml we need.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
    edition: String,
}

/// Types of tar compression we support; gzip for compatibility, zstd is the modern baseline.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Compression {
    /// No compression
    None,
    /// Gzip is the legacy compression algorithm
    Gzip,
    /// Zstd is a modern compression baseline
    Zstd,
}

impl Compression {
    fn supported(&self) -> bool {
        match self {
            Compression::None | Compression::Gzip => true,
            Compression::Zstd => {
                // For now, assume Unix systems have an external `zstd` binary
                cfg!(not(windows))
            }
        }
    }
}

/// Output format; the default is a directory.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum OutputTarget {
    /// Write to a directory; the default path is `vendor`
    Dir,
    /// Write to an uncompressed (reproducible) tar archive; the default path is vendor.tar
    Tar,
    /// Write to a gzip-compressed (reproducible) tarball; the default path is vendor.tar.zstd
    TarGzip,
    /// Write to a zstd-compressed (reproducible) tarball; the default path is vendor.tar.zstd
    TarZstd,
}

impl Default for OutputTarget {
    fn default() -> Self {
        Self::Dir
    }
}

impl clap::ValueEnum for OutputTarget {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Dir, Self::Tar, Self::TarGzip, Self::TarZstd]
    }

    fn to_possible_value<'a>(&self) -> Option<clap::builder::PossibleValue> {
        match self {
            Self::Dir => Some(clap::builder::PossibleValue::new("dir")),
            Self::Tar => Some(clap::builder::PossibleValue::new("tar")),
            Self::TarGzip => Some(clap::builder::PossibleValue::new("tar.gz")),
            Self::TarZstd => Some(clap::builder::PossibleValue::new("tar.zstd")),
        }
    }
}

/// Exclude a file/directory from a crate.
#[derive(PartialEq, Eq, Deserialize, Debug, Hash, Clone)]
#[serde(rename_all = "kebab-case")]
struct CrateExclude {
    name: String,
    exclude: String,
}

impl CrateExclude {
    /// Parse a crate exclude in the form `CRATENAME#PATH`.
    fn parse_str(s: &str) -> Result<Self> {
        let (k, v) = s
            .split_once('#')
            .ok_or_else(|| anyhow::anyhow!("Missing '#' in crate exclude"))?;
        Ok(Self {
            name: k.to_string(),
            exclude: v.to_string(),
        })
    }
}

/// The configuration used to filter the set of dependencies.
#[derive(PartialEq, Eq, Deserialize, Debug, Default)]
#[serde(rename_all = "kebab-case")]
struct VendorFilter {
    platforms: Option<BTreeSet<String>>,
    tier: Option<tiers::Tier>,
    #[serde(default)]
    all_features: bool,
    #[serde(default)]
    no_default_features: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    features: Vec<String>,
    exclude_crate_paths: Option<HashSet<CrateExclude>>,
}

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Only include crates for these targets ('*' wildcards are supported).
    ///
    /// For example, `x86_64-unknown-linux-gnu`.
    #[arg(long)]
    platform: Option<Vec<String>>,

    /// Limit platforms to the provided tier ("1" or "2").
    #[arg(long, value_parser)]
    tier: Option<tiers::Tier>,

    /// Remove files/subdirectories in crates that match an exact path.
    /// The format is "CRATENAME#PATH". CRATENAME is the name of a crate (without
    /// a version included) or "*" as a wildcard for all crates. PATH must be a
    /// relative path, and can name a regular file, symbolic link or a directory.
    ///
    /// If the filename matches a directory, it and all its contents will be removed.
    /// For example, `curl-sys#curl` will remove the vendored libcurl C sources
    /// from the `curl-sys` crate.
    /// For example, `*#tests` will remove tests folder from all crates.
    ///
    /// Nonexistent paths will emit a warning, but are not currently an error.
    #[arg(long)]
    exclude_crate_path: Option<Vec<String>>,

    /// Path to Cargo.toml
    #[arg(long)]
    manifest_path: Option<Utf8PathBuf>,

    /// Activate all available features
    #[arg(long, default_value_t = false)]
    all_features: bool,

    /// Do not activate the `default` feature
    #[arg(long, default_value_t = false)]
    no_default_features: bool,

    /// Space or comma separated list of features to activate. Features
    /// of workspace members may be enabled with package-name/feature-name
    /// syntax. This flag may be specified multiple times, which enables all
    /// specified features.
    #[arg(long, short = 'F')]
    features: Vec<String>,

    /// Pick the output format.
    #[arg(long, default_value = "dir")]
    format: OutputTarget,

    /// The file path name to use when generating a tar stream.  It's suggested
    /// to use `--prefix=vendor`; this is not the default only for backwards
    /// compatibilty.
    #[arg(long)]
    prefix: Option<Utf8PathBuf>,

    /// Run without accessing the network; this is passed down to e.g. `cargo metadata --offline`.
    #[arg(long)]
    offline: bool,

    /// Instead of ignoring [source] configuration by default in `.cargo/config.toml` read it and
    /// use it when downloading crates from crates.io, for example
    /// ; this is passed down to e.g. `cargo vendor --respect-source-config`.
    #[arg(long)]
    respect_source_config: bool,

    /// The output path
    path: Option<Utf8PathBuf>,

    /// Additional `Cargo.toml` to sync and vendor
    #[arg(short, long, value_name = "TOML")]
    sync: Option<Vec<Utf8PathBuf>>,
}

fn filter_manifest(manifest: &mut toml::Value) {
    if let Some(t) = manifest.as_table_mut() {
        for &k in UNWANTED_MANIFEST_KEYS {
            t.remove(k);
        }
        if let Some(t) = t
            .get_mut(MANIFEST_KEY_PACKAGE)
            .and_then(|v| v.as_table_mut())
        {
            for &k in UNWANTED_PACKAGE_KEYS {
                t.remove(k);
            }
        }
    }
}

/// Compute the SHA-256 digest of the buffer and return the result in hexadecimal format
fn sha256_hexdigest(buf: &[u8]) -> Result<String> {
    // NOTE: Keep this in sync with the copy in the tests
    #[cfg(not(windows))]
    {
        let digest = openssl::hash::hash(openssl::hash::MessageDigest::sha256(), buf)?;
        Ok(hex::encode(digest))
    }
    #[cfg(windows)]
    {
        // This is a pure-Rust implementation which avoids the openssl dependency on Windows.
        // However, it may make sense here to add something like native-tls to the ecosystem
        // except for sha digests?  On Windows I'm sure there's a core crypto library for this.
        use sha2::Digest;
        let digest = sha2::Sha256::digest(buf);
        Ok(hex::encode(digest))
    }
}

/// Given a directory for a package generated by `cargo vendor`, replace it
/// with an empty package.  This follows the approach suggested here
/// https://github.com/rust-lang/cargo/issues/7058#issuecomment-697074341
///
/// Steps:
///
/// - Remove everything except Cargo.toml
/// - Create a "stub" source directory with an empty src/lib.rs
/// - Regenerate the cargo checksums
///
/// The generated package will fail to compile, but we're relying on it
/// not actually being compiled.  Entirely removing the crates would
/// require editing the dependent crates, which would be more involved.
fn replace_with_stub(path: &Utf8Path) -> Result<()> {
    let cargo_toml_path = path.join(CARGO_TOML);
    let cargo_toml_data =
        std::fs::read_to_string(&cargo_toml_path).context("Reading Cargo.toml")?;
    let mut cargo_toml_data: toml::Value =
        toml::from_str(&cargo_toml_data).with_context(|| format!("Parsing {cargo_toml_path}"))?;
    filter_manifest(&mut cargo_toml_data);

    let checksums_path = path.join(CARGO_CHECKSUM);
    let checksums = std::fs::File::open(&checksums_path).map(BufReader::new)?;
    let mut checksums: CargoChecksums =
        serde_json::from_reader(checksums).with_context(|| format!("Parsing {checksums_path}"))?;

    // Clear out everything and replace it with a fresh directory with a new `src/`
    // subdir.
    std::fs::remove_dir_all(path)?;
    std::fs::create_dir_all(path.join("src"))?;
    // Also empty out the file checksums, but keep the overall package checksum.
    checksums.files.clear();

    // Helper to both write a file and compute its sha256, storing it in the
    // cargo checksum list.
    let mut writef = |target: &Utf8Path, contents: &[u8]| {
        let fullpath = path.join(target);
        std::fs::write(fullpath, contents)?;
        let digest = sha256_hexdigest(contents)?;
        checksums.files.insert(target.to_string(), digest);
        Ok::<_, anyhow::Error>(())
    };
    let cargo_toml_data = toml::to_string(&cargo_toml_data).context("Reserializing manifest")?;
    // An empty Cargo.toml
    writef(Utf8Path::new(CARGO_TOML), cargo_toml_data.as_bytes())?;
    // And an empty source file
    writef(Utf8Path::new("src/lib.rs"), b"")?;
    // Finally, serialize the new checksums
    let mut w = std::fs::File::create(checksums_path).map(std::io::BufWriter::new)?;
    serde_json::to_writer(&mut w, &checksums)?;
    w.flush()?;
    Ok(())
}

impl VendorFilter {
    /// Returns true if this configuration will filter by platform
    fn enables_platform_filtering(&self) -> bool {
        self.tier.is_some()
            || self
                .platforms
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or_default()
    }

    /// Parse a value from `package.metadata.vendor-filter`.
    fn parse_json(meta: &serde_json::Value) -> Result<Option<Self>> {
        let meta = meta.as_object().and_then(|o| o.get(CONFIG_KEY));
        let meta = if let Some(m) = meta {
            m
        } else {
            return Ok(None);
        };
        let mut unused = std::collections::BTreeSet::new();
        let v: Self = serde_ignored::deserialize(meta.clone(), |path| {
            unused.insert(path.to_string());
        })?;
        for k in unused {
            eprintln!("warning: Unknown key {k} in metadata.{CONFIG_KEY}")
        }
        Ok(Some(v))
    }

    /// Parse the subset of CLI arguments that affect vendor content into a filter.
    fn parse_args(args: &Args) -> Result<Option<Self>> {
        let args_unset = args.platform.is_none()
            && args.tier.is_none()
            && !args.all_features
            && !args.no_default_features
            && args.features.is_empty()
            && args.exclude_crate_path.is_none();
        let exclude_crate_paths = args
            .exclude_crate_path
            .as_ref()
            .map(|v| {
                v.iter()
                    .map(|e| CrateExclude::parse_str(e))
                    .collect::<Result<HashSet<_>>>()
            })
            .transpose()?;
        let r = (!args_unset).then(|| Self {
            platforms: args
                .platform
                .as_ref()
                .map(|x| BTreeSet::from_iter(x.iter().cloned())),
            tier: args.tier.clone(),
            all_features: args.all_features,
            no_default_features: args.no_default_features,
            features: args.features.clone(),
            exclude_crate_paths,
        });
        Ok(r)
    }
}

/// Process CLI arguments into a filter.
fn gather_config(args: &Args) -> Result<Option<VendorFilter>> {
    // Accept config from arguments first in preference to Cargo.toml metadata.
    if let Some(f) = VendorFilter::parse_args(args)? {
        return Ok(Some(f));
    };
    // Otherwise gather from `package.metadata.vendor-filter` in Cargo.toml
    let meta = new_metadata_cmd(args.manifest_path.as_deref(), args.offline);
    let meta = meta
        .exec()
        .context("Executing cargo metadata (first run)")?;
    if let Some(root) = meta.root_package() {
        VendorFilter::parse_json(&root.metadata)
    } else {
        VendorFilter::parse_json(&meta.workspace_metadata)
    }
}

/// Given a crate, remove matching files/directories in excludes.
fn process_excludes(path: &Utf8PathBuf, name: &str, excludes: &HashSet<&str>) -> Result<()> {
    let mut matched = false;
    for exclude in excludes.iter().map(Utf8Path::new) {
        if exclude.is_absolute() {
            anyhow::bail!("Invalid absolute path in crate exclude {name} {exclude}");
        }
        let path = path.join(exclude);

        let meta = match path.symlink_metadata() {
            Ok(r) => Ok(Some(r)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }?;
        if let Some(meta) = meta {
            if meta.is_dir() {
                std::fs::remove_dir_all(path)?;
            } else {
                std::fs::remove_file(path)?;
            }
            eprintln!("Removed from crate {name}: {exclude}");
            matched = true;
        } else {
            eprintln!("Warning: No match for exclude for crate {name}: {exclude}");
        }
    }
    if matched {
        let checksums_path = path.join(CARGO_CHECKSUM);
        let checksums = std::fs::File::open(&checksums_path).map(BufReader::new)?;
        let mut checksums: CargoChecksums = serde_json::from_reader(checksums)
            .with_context(|| format!("Parsing {checksums_path}"))?;
        let orig = checksums.files.len();
        checksums.files.retain(|k, _| {
            let k = Utf8Path::new(k);
            for exclude in excludes.iter().map(Utf8Path::new) {
                if k.starts_with(exclude) {
                    return false;
                }
            }
            true
        });
        assert_ne!(orig, checksums.files.len());
        let mut w = std::fs::File::create(checksums_path).map(std::io::BufWriter::new)?;
        serde_json::to_writer(&mut w, &checksums)?;
    }
    Ok(())
}

/// Return the timestamp of the latest git commit in seconds since the Unix epoch.
fn git_source_date_epoch(dir: &Utf8Path) -> Result<u64> {
    let o = Command::new("git")
        .args(["log", "-1", "--pretty=%ct"])
        .current_dir(dir)
        .output()?;
    if !o.status.success() {
        anyhow::bail!("git exited with an error: {:?}", o);
    }
    let buf = String::from_utf8(o.stdout).context("Failed to parse git log output")?;
    let r = buf.trim().parse()?;
    Ok(r)
}

/// Generate a reproducible tarball with optional zstd compression.
fn generate_tar_from(
    srcdir: &Utf8Path,
    dest: &Utf8Path,
    prefix: Option<&Utf8Path>,
    compress: Compression,
) -> Result<()> {
    const GZIP_MTIME: u32 = 0;
    const GZIP_COMPRESSION: u32 = 6; // default level of the CLI
    let envkey = "SOURCE_DATE_EPOCH";
    let source_date_epoch_env = std::env::var_os(envkey);
    let source_date_epoch_env = source_date_epoch_env
        .as_ref()
        .map(|v| {
            v.to_str()
                .map(Cow::Borrowed)
                .ok_or_else(|| anyhow!("Invalid value for {envkey}"))
        })
        .transpose()?;
    let source_date_epoch = source_date_epoch_env.map(Ok).unwrap_or_else(|| {
        git_source_date_epoch(Utf8Path::new(".")).map(|v| Cow::Owned(v.to_string()))
    })?;
    let timestamp: u64 = source_date_epoch.parse()?;

    let output = std::fs::File::create(dest)?;
    let mut helper = None;
    let output: Box<dyn Write> = match compress {
        Compression::None => Box::new(output),
        Compression::Gzip => Box::new(
            flate2::GzBuilder::new()
                .mtime(GZIP_MTIME)
                .write(output, flate2::Compression::new(GZIP_COMPRESSION)),
        ),
        Compression::Zstd => {
            let mut subhelper = Command::new("zstd");
            subhelper.stdin(std::process::Stdio::piped());
            subhelper.stdout(output);
            let mut subhelper = subhelper.spawn()?;
            let o = subhelper.stdin.take().unwrap();
            helper = Some(subhelper);
            Box::new(o)
        }
    };
    let output = std::io::BufWriter::new(output);
    let mut archive = tar::Builder::new(output);

    for entry in walkdir::WalkDir::new(srcdir).sort_by_file_name() {
        let entry = entry?;
        let path = entry.path();
        let subpath = path.strip_prefix(srcdir)?;
        let subpath = if let Some(p) = camino::Utf8Path::from_path(subpath) {
            let p = prefix
                .map(|prefix| Cow::Owned(prefix.join(p)))
                .unwrap_or(Cow::Borrowed(p));
            Utf8Path::new("./").join(&*p)
        } else {
            anyhow::bail!("Invalid non-UTF8 path: {path:?}");
        };
        let metadata = path.symlink_metadata()?;
        let mut h = tar::Header::new_gnu();
        h.set_metadata_in_mode(&metadata, tar::HeaderMode::Deterministic);
        h.set_mtime(timestamp);
        h.set_uid(0);
        h.set_gid(0);
        h.set_cksum();
        if metadata.is_dir() {
            archive.append_data(&mut h, subpath, std::io::Cursor::new([]))?;
        } else if metadata.is_file() {
            let src = std::fs::File::open(path).map(std::io::BufReader::new)?;
            archive.append_data(&mut h, subpath, src)?;
        } else if metadata.is_symlink() {
            let target = path.read_link()?;
            archive.append_link(&mut h, subpath, target)?;
        } else {
            eprintln!("Ignoring unexpected special file: {path:?}");
            continue;
        }
    }

    let mut output = archive.into_inner()?;
    output.flush()?;
    drop(output);

    if let Some(mut helper) = helper {
        let st = helper.wait()?;
        if !st.success() {
            anyhow::bail!("Compression program for {compress:?} failed: {st:?}");
        }
    }

    Ok(())
}

impl Args {
    /// Return all manifest (aka `Cargo.toml`) to parse
    fn get_all_manifest_paths(&self) -> Vec<Option<&Utf8Path>> {
        // We have to always add the original manifest path, even if it's `None`
        // to ensure that the cargo-commands are run at least once.
        let mut all_manifest_paths = vec![self.manifest_path.as_deref()];
        // Then add additional manifests, if there are any.
        if let Some(s) = &self.sync {
            for p in s {
                all_manifest_paths.push(Some(p.as_path()));
            }
        }
        all_manifest_paths
    }

    /// Find the root package
    fn get_root_package(&self) -> Result<Option<Package>> {
        let mut command = new_metadata_cmd(self.manifest_path.as_deref(), self.offline);
        command.no_deps();

        let meta = command.exec().context("Executing cargo metadata")?;
        Ok(meta.root_package().cloned())
    }
}

fn new_metadata_cmd(path: Option<&Utf8Path>, offline: bool) -> MetadataCommand {
    let mut command = MetadataCommand::new();
    if offline {
        command.other_options(vec![OFFLINE.to_string()]);
    }
    if let Some(p) = path {
        command.manifest_path(p);
    }
    command
}

fn get_unfiltered_packages(
    args: &Args,
    config: &VendorFilter,
) -> Result<HashMap<cargo_metadata::PackageId, cargo_metadata::Package>> {
    let all_manifest_paths = args.get_all_manifest_paths();
    let mut packages = HashMap::new();
    for manifest_path in all_manifest_paths {
        let mut command = new_metadata_cmd(manifest_path, args.offline);
        if config.all_features {
            command.features(AllFeatures);
        }
        if config.no_default_features {
            command.features(NoDefaultFeatures);
        }
        if !config.features.is_empty() {
            command.features(SomeFeatures(config.features.clone()));
        }
        let meta = command.exec().context("Executing cargo metadata")?;
        meta.packages
            .into_iter()
            .map(|pkg| (pkg.id.clone(), pkg))
            .for_each(|(id, pkg)| {
                packages.insert(id, pkg);
            });
    }
    Ok(packages)
}

/// Using the filter configuration, add references to the `packages` map that
/// point into the `all_packages` set we already have (to avoid duplicating memory).
fn add_packages_for_platform<'p>(
    args: &Args,
    config: &VendorFilter,
    all_packages: &'p HashMap<cargo_metadata::PackageId, cargo_metadata::Package>,
    packages: &mut HashMap<cargo_metadata::PackageId, &'p cargo_metadata::Package>,
    platform: Option<&str>,
) -> Result<()> {
    let all_manifest_paths = args.get_all_manifest_paths();
    for manifest_path in all_manifest_paths {
        let mut command = new_metadata_cmd(manifest_path, args.offline);
        if config.all_features {
            command.features(AllFeatures);
        }
        if config.no_default_features {
            command.features(NoDefaultFeatures);
        }
        if !config.features.is_empty() {
            command.features(SomeFeatures(config.features.clone()));
        }
        if let Some(platform) = platform {
            command.other_options(vec![format!("--filter-platform={platform}")]);
        }

        let meta = command.exec().context("Executing cargo metadata")?;
        for package in meta.packages {
            let package = all_packages
                .get(&package.id)
                .ok_or_else(|| anyhow!("Failed to find package {}", package.name))
                .unwrap();
            packages.insert(package.id.clone(), package);
        }
    }
    Ok(())
}

/// Parse the output of `rustc --print target-list`
fn get_target_list(tier: Option<&tiers::Tier>) -> Result<HashSet<String>> {
    if let Some(tier) = tier {
        Ok(tier.targets().map(|v| v.to_string()).collect())
    } else {
        let o = Command::new("rustc")
            .args(["--print", "target-list"])
            .output()
            .context("Failed to invoke rustc --print target-list")?;
        let buf = String::from_utf8(o.stdout)?;
        Ok(buf.lines().map(|s| s.trim().to_string()).collect())
    }
}

type ParsedPlatform<'a> = SmallVec<[&'a str; 4]>;

fn platform_matches(platform: &ParsedPlatform, o: &ParsedPlatform) -> bool {
    if platform.len() != o.len() {
        return false;
    }
    for (t, p) in platform.iter().zip(o.iter()) {
        if *p == "*" {
            continue;
        }
        if p != t {
            return false;
        }
    }
    true
}

fn expand_one_platform<'t>(
    platform: &str,
    target_list: &'t [(&str, ParsedPlatform)],
) -> Vec<&'t str> {
    let platform_parts: ParsedPlatform = platform.split('-').collect();
    let mut r = Vec::new();
    for (target, target_parts) in target_list {
        if platform_matches(target_parts, &platform_parts) {
            r.push(*target)
        }
    }
    r
}

fn expand_platforms<'b>(
    platforms: &'b [&'b str],
    target_list: &[(&str, ParsedPlatform)],
) -> Result<Vec<String>> {
    let r = platforms
        .iter()
        .flat_map(|&platform| {
            if platform.contains('*') {
                Either::Left(expand_one_platform(platform, target_list).into_iter())
            } else {
                Either::Right([platform].into_iter())
            }
        })
        .map(ToOwned::to_owned) // Clone to avoid need for common lifetimes
        .collect();
    Ok(r)
}

/// Deletes unreferenced packages from the vendor directory.
fn delete_unreferenced_packages(
    output_dir: &Utf8Path,
    package_filenames: &BTreeMap<Cow<'_, str>, &Package>,
    excludes: &HashMap<&str, HashSet<&str>>,
) -> Result<()> {
    // A reusable buffer (silly optimization to avoid allocating lots of path buffers)
    let mut pbuf = Utf8PathBuf::from(&output_dir);
    let mut unreferenced = HashSet::new();

    // Deleting files while iterating a `read_dir` produces undefined behaviour.
    let mut entries = Vec::new();
    for entry in output_dir.read_dir_utf8()? {
        entries.push(entry?);
    }

    // Find and physically delete unreferenced packages, and apply filters.
    for entry in entries {
        let name = entry.file_name();
        pbuf.push(name);

        if !package_filenames.contains_key(&Cow::Borrowed(name)) {
            replace_with_stub(&pbuf).with_context(|| format!("Replacing with stub: {name}"))?;
            eprintln!("Replacing unreferenced package with stub: {name}");
            assert!(unreferenced.insert(name.to_string()));
        }

        if let Some(crate_excludes) = excludes.get(name) {
            process_excludes(&pbuf, name, crate_excludes)?;
        }
        if let Some(generic_excludes) = excludes.get("*") {
            process_excludes(&pbuf, name, generic_excludes)?;
        }

        let r = pbuf.pop();
        debug_assert!(r);
    }

    Ok(())
}

/// Return the filename cargo vendor would use for a package which has multiple versions present
fn package_versioned_filename(p: &Package) -> String {
    format!("{}-{}", p.name, p.version)
}

/// An inner version of `main`; the primary code.
fn run() -> Result<()> {
    let mut args = std::env::args().collect::<Vec<_>>();
    // When invoked as a subcommand of `cargo`, it passes the subcommand name as
    // the second argument, which is a bit inconvenient for us.  Special case that.
    if args.get(1).map(|s| s.as_str()) == Some(SELF_NAME) {
        args.remove(1);
    }

    let args = Args::parse_from(args);

    let (had_config, config) = if let Some(c) = gather_config(&args)? {
        (true, c)
    } else {
        (false, VendorFilter::default())
    };
    if !had_config {
        eprintln!("NOTE: No vendor filtering enabled");
    }

    let compression = match args.format {
        OutputTarget::Tar | OutputTarget::Dir => Compression::None,
        OutputTarget::TarGzip => Compression::Gzip,
        OutputTarget::TarZstd => Compression::Zstd,
    };
    if !compression.supported() {
        anyhow::bail!("Compression format {compression:?} is not supported on this platform");
    }

    let tempdir = match args.format {
        OutputTarget::Tar | OutputTarget::TarGzip | OutputTarget::TarZstd => {
            let target_basedir = args.path.as_ref().and_then(|p| p.parent());
            Some(tempfile::tempdir_in(
                target_basedir.unwrap_or_else(|| ".".into()),
            )?)
        }
        OutputTarget::Dir => None,
    };
    let tempdir_path: Option<&Utf8Path> = tempdir
        .as_ref()
        .map(|td| td.path().try_into())
        .transpose()?;
    let final_output_path = args.path.clone().unwrap_or_else(|| {
        match args.format {
            OutputTarget::Dir => VENDOR_DEFAULT_PATH,
            OutputTarget::Tar => VENDOR_DEFAULT_PATH_TAR,
            OutputTarget::TarGzip => VENDOR_DEFAULT_PATH_TAR_GZ,
            OutputTarget::TarZstd => VENDOR_DEFAULT_PATH_TAR_ZSTD,
        }
        .into()
    });
    let output_dir = tempdir_path
        .map(|v| Cow::Owned(v.join("vendor")))
        .unwrap_or_else(|| match args.format {
            OutputTarget::Dir => Cow::Borrowed(final_output_path.as_path()),
            _ => unreachable!(),
        });

    if output_dir.exists() {
        anyhow::bail!("Refusing to operate on extant directory: {}", output_dir);
    }

    eprintln!("Gathering metadata");
    // We need to gather the full, unfiltered metadata to canonically know what
    // `cargo vendor` will do.
    let all_packages = get_unfiltered_packages(&args, &config)?;
    let root = args.get_root_package()?;

    // Create a mapping of name -> [package versions]
    let mut pkgs_by_name = BTreeMap::<_, Vec<_>>::new();
    for pkg in all_packages.values() {
        let name = pkg.name.as_str();
        // Skip ourself
        if let Some(root) = root.as_ref() {
            if pkg.id == root.id {
                continue;
            }
        }
        // Also skip anything local
        if pkg.source.as_ref().is_none() {
            eprintln!("Skipping {name}");
            continue;
        }

        let v = pkgs_by_name.entry(name).or_default();
        v.push(pkg);
    }

    // And now do the filtered set
    let mut packages = HashMap::new();
    let mut expanded_platforms = None;
    if config.enables_platform_filtering() {
        eprintln!("Gathering metadata for platforms");
        let target_list = get_target_list(config.tier.as_ref())?;
        let target_list: Vec<(&str, ParsedPlatform)> = target_list
            .iter()
            .map(|platform| (platform.as_str(), platform.split('-').collect()))
            .collect();
        // If the user provided an explicit platform list, it may have globs.  Expand it with the known target list.
        let platforms: Vec<_> = if let Some(platforms) = config.platforms.as_ref() {
            let platforms: Vec<_> = platforms.iter().map(|s| s.as_str()).collect();
            expand_platforms(&platforms, &target_list)?
        } else {
            // Here the user didn't provide a platform list; we're just filtering by tier.
            assert!(config.tier.is_some());
            let mut v: Vec<_> = target_list.into_iter().map(|v| v.0.to_string()).collect();
            v.sort();
            v
        };
        for platform in platforms.iter() {
            add_packages_for_platform(
                &args,
                &config,
                &all_packages,
                &mut packages,
                Some(platform),
            )?;
        }
        expanded_platforms = Some(platforms);
    } else {
        add_packages_for_platform(&args, &config, &all_packages, &mut packages, None)?;
    }

    // Run `cargo vendor` which will capture all dependencies.
    let manifest_path = args
        .manifest_path
        .as_ref()
        .map(|o| ["--manifest-path", o.as_str()]);
    let mut builder = Command::new("cargo");
    builder
        .args(["vendor"])
        .args(args.offline.then_some(OFFLINE))
        .args(args.respect_source_config.then_some(RESPECT_SOURCE_CONFIG))
        .args(manifest_path.iter().flatten());
    if let Some(sync) = args.sync {
        for s in sync {
            builder.args([SYNC, s.as_str()]);
        }
    }
    let status = builder.arg(&*output_dir).status()?;
    if !status.success() {
        anyhow::bail!("Failed to execute cargo vendor: {:?}", status);
    }

    // Determine the set of vendored components we want to keep, by intersecting
    // the all_packages map with the filtered one, returning an index by the
    // directory name that will have been generated by `cargo vendor`.
    let mut package_filenames = BTreeMap::new();
    for (_name, mut pkgs) in pkgs_by_name {
        // Reverse sort - greater version is lower index
        pkgs.sort_by(|a, b| b.version.cmp(&a.version));
        // SAFETY: The package set must be non-empty
        let (first, rest) = pkgs.split_first().unwrap();

        if packages.contains_key(&first.id) {
            package_filenames.insert(Cow::Borrowed(first.name.as_str()), *first);
        }
        for &pkg in rest {
            if packages.contains_key(&pkg.id) {
                package_filenames.insert(Cow::Owned(package_versioned_filename(pkg)), pkg);
            }
        }
    }

    // Index the excludes into a mapping from crate name -> [list of excludes].
    let mut excludes: HashMap<&str, HashSet<&str>> = HashMap::new();
    if let Some(exclude_paths) = &config.exclude_crate_paths {
        for ex_path in exclude_paths {
            let e = excludes.entry(ex_path.name.as_str()).or_default();
            e.insert(ex_path.exclude.as_str());
        }
    }

    delete_unreferenced_packages(&output_dir, &package_filenames, &excludes)?;

    // For tar archives, generate them now from the temporary directory.
    let prefix = args.prefix.as_deref();
    match args.format {
        OutputTarget::Tar | OutputTarget::TarGzip | OutputTarget::TarZstd => {
            generate_tar_from(&output_dir, &final_output_path, prefix, compression)?
        }
        OutputTarget::Dir => {
            if prefix.is_some() {
                anyhow::bail!("Cannot use --prefix with non-tar --format");
            }
        }
    };

    if !had_config {
        eprintln!("Notice: No vendor filtering enabled");
    } else if let Some(platforms) = expanded_platforms {
        eprintln!("Filtered to target platforms: {:?}", platforms);
    }

    eprintln!("Generated: {final_output_path}");
    Ok(())
}

/// Output
fn main() {
    if let Err(e) = run() {
        // I prefer seeing errors like error: While processing foo: No such file or directory
        // instead of multi-line.
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
}

#[test]
fn test_parse_config() {
    use serde_json::json;

    let valid = vec![
        json!({}),
        json!({ "platforms": ["aarch64-unknown-linux-gnu"]}),
        json!({ "platforms": ["aarch64-unknown-linux-gnu"], "no-default-features": true}),
        json!({ "platforms": ["*-unknown-linux-gnu"], "tier": "2", "no-default-features": false}),
        json!({ "platforms": ["*-unknown-linux-gnu"], "tier": "Two", "no-default-features": false}),
        json!({ "platforms": ["aarch64-unknown-linux-gnu"], "all-features": true, "no-default-features": false}),
        json!({ "platforms": ["aarch64-unknown-linux-gnu"], "no-default-features": true}),
        json!({ "platforms": ["aarch64-unknown-linux-gnu"], "no-default-features": true, "features": ["first-feature", "second-feature"]}),
    ];
    for case in valid {
        let _: VendorFilter = serde_json::from_value(case).unwrap();
    }
    let filter = json!({ "exclude-crate-paths": [ { "name": "hex", "exclude": "benches" }, { "name": "curl", "exclude": "curl" } ]});
    let r: VendorFilter = serde_json::from_value(filter).unwrap();
    assert_eq!(r.exclude_crate_paths.unwrap().len(), 2);
}

#[test]
fn test_parse_checksums() {
    use serde_json::json;

    let valid = vec![
        json!({
          "files": {
            "src/lib.rs": "af7f3c1dc4a7612f3519b812f8d6f9298f0f8af2e999ee3a23e6e9a5ddce5d75",
          },
          "package": null
        }),
        json!({
          "files": {
            "src/lib.rs": "af7f3c1dc4a7612f3519b812f8d6f9298f0f8af2e999ee3a23e6e9a5ddce5d75",
          },
          "package": "433cfd6710c9986c576a25ca913c39d66a6474107b406f34f91d4a8923395241"
        }),
    ];
    for case in valid {
        let _: CargoChecksums = serde_json::from_value(case).unwrap();
    }
}

#[test]
fn test_platforms() {
    let targets = [
        "powerpc64le-unknown-linux-gnu",
        "x86_64-unknown-linux-gnux32",
        "arm-linux-androideabi",
        "x86_64-unknown-linux-gnu",
        "mipsel-sony-psp",
        "wasm32-wasi",
        "x86_64-sun-solaris",
    ];
    let target_list: Vec<(&str, ParsedPlatform)> = targets
        .iter()
        .map(|platform| (*platform, platform.split('-').collect()))
        .collect();
    // Verify we pass through literals
    for &target in targets.iter() {
        let targets = [target];
        let v = expand_platforms(targets.as_slice(), &target_list).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0], target);
    }

    let linux_spec = ["*-unknown-linux-gnu"];
    let mut linuxes = expand_platforms(&linux_spec, &target_list).unwrap();
    linuxes.sort();
    assert_eq!(linuxes.len(), 2);
    assert_eq!(linuxes[0], "powerpc64le-unknown-linux-gnu");
    assert_eq!(linuxes[1], "x86_64-unknown-linux-gnu");
}

#[test]
fn test_filter_manifest() {
    let mut v: toml::Value = toml::from_str(
        r#"    
[package]
name = "kernel32-sys"


[lib]
name = "kernel32"

[[bin]]
name = "somebin"

[[example]]
name = "someexample"

[[test]]
name = "sometest"

[[example]]
name = "otherexample"

[[bench]]
name = "somebench"
"#,
    )
    .unwrap();
    let t = v.as_table().unwrap();
    for &k in UNWANTED_MANIFEST_KEYS {
        assert!(t.contains_key(k), "expected {k}");
    }
    filter_manifest(&mut v);
    let t = v.as_table().unwrap();
    for &k in UNWANTED_MANIFEST_KEYS {
        assert!(!t.contains_key(k));
    }
}

#[test]
fn test_cli() {
    use clap::CommandFactory;
    Args::command().debug_assert()
}
