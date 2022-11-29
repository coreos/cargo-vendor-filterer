use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::Package;
use cargo_metadata::{CargoOpt::AllFeatures, MetadataCommand};
use clap::Parser;
use either::Either;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufReader, Write};
use std::process::Command;
use std::vec;

/// The path we use in Cargo.toml i.e. `package.metadata.vendor-filter`
const CONFIG_KEY: &str = "vendor-filter";
/// The name of our binary
const SELF_NAME: &str = "vendor-filterer";
/// The default directory path
const VENDOR_DEFAULT_PATH: &str = "vendor";
/// The default path for --format=tar
const VENDOR_DEFAULT_PATH_TAR: &str = "vendor.tar";
/// The default path for --format=tar.zstd
const VENDOR_DEFAULT_PATH_TAR_ZSTD: &str = "vendor.tar.zstd";
/// The default path for --format=tar.gz
const VENDOR_DEFAULT_PATH_TAR_GZ: &str = "vendor.tar.gz";
/// The filename cargo writes in packages with file checksums
const CARGO_CHECKSUM: &str = ".cargo-checksum.json";
/// The CLI argument passed to cargo to work offline
const OFFLINE: &str = "--offline";

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
    fn tar_switch(&self) -> Option<&'static str> {
        match self {
            Compression::None => None,
            Compression::Gzip => Some("--gzip"),
            Compression::Zstd => Some("--zstd"),
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

    fn to_possible_value<'a>(&self) -> Option<clap::PossibleValue<'a>> {
        match self {
            Self::Dir => Some(clap::PossibleValue::new("dir")),
            Self::Tar => Some(clap::PossibleValue::new("tar")),
            Self::TarGzip => Some(clap::PossibleValue::new("tar.gz")),
            Self::TarZstd => Some(clap::PossibleValue::new("tar.zstd")),
        }
    }
}

/// Exclude a file/directory from a crate.
#[derive(PartialEq, Eq, Deserialize, Debug)]
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
    platforms: Option<Vec<String>>,
    all_features: Option<bool>,
    exclude_crate_paths: Option<Vec<CrateExclude>>,
}

/// Enhanced `cargo vendor` with filtering
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Only include crates for these targets.
    ///
    /// For example, `x86_64-unknown-linux-gnu`.
    #[clap(long)]
    platform: Option<Vec<String>>,

    /// Remove files/subdirectories in crates that match an exact path.
    ///
    /// The format is "CRATENAME#PATH".  CRATENAME is the name of a crate (without
    /// a version included).  PATH must be a relative path, and can name a regular
    /// file, symbolic link or a directory.
    ///
    /// If the filename matches a directory, it and all its contents will be removed.
    /// For example, `curl-sys#curl` will remove the vendored libcurl C sources
    /// from the `curl-sys` crate.
    ///
    /// Nonexistent paths will emit a warning, but are not currently an error.
    #[clap(long)]
    exclude_crate_path: Option<Vec<String>>,

    /// Path to Cargo.toml
    #[clap(long, value_parser)]
    manifest_path: Option<Utf8PathBuf>,

    /// Enable all features
    #[clap(long)]
    all_features: Option<bool>,

    /// Pick the output format; the only currently available option is `dir`,
    /// which writes to a directory.  The default value is `vendor`.
    #[clap(long, value_parser, default_value = "dir")]
    format: OutputTarget,

    /// The file path name to use when generating a tar stream.  It's suggested
    /// to use `--prefix=vendor`; this is not the default only for backwards
    /// compatibilty.
    #[clap(long, value_parser)]
    prefix: Option<Utf8PathBuf>,

    /// Run without accessing the network; this is passed down to e.g. `cargo metadata --offline`.
    #[clap(long)]
    offline: bool,

    /// The output path
    path: Option<Utf8PathBuf>,
}

// cargo does autodiscovery of the workspace, and as far as I can tell
// there's no way to turn this off via environment variable or CLI.
// So...forcibly hack in a dummy workspace value.
fn inject_dummy_workspace(path: &Utf8Path) -> Result<()> {
    let cargo_path = path.join("Cargo.toml");
    let cargo_toml =
        std::fs::read_to_string(&cargo_path).with_context(|| format!("Writing {path}"))?;
    let cargo_toml = cargo_toml.replace("[package]", "[workspace]\n[package]");
    std::fs::write(&cargo_path, cargo_toml)?;
    Ok(())
}

/// Given a directory for a package generated by `cargo vendor`, replace it
/// with an empty package.  This follows the approach suggested here
/// https://github.com/rust-lang/cargo/issues/7058#issuecomment-697074341
///
/// Entirely removing the crates would require editing the dependency graph,
/// which gets into more work.
fn replace_with_stub(path: &Utf8Path, offline: bool) -> Result<()> {
    inject_dummy_workspace(path).context("Injecting dummy [workspace]")?;

    // Gather metadata for just this dependency, not recursively.
    let mut command = MetadataCommand::new();
    command.current_dir(path);
    command.no_deps();
    if offline {
        command.other_options(vec![OFFLINE.to_string()]);
    }
    let meta = command.exec().context("Executing cargo metadata")?;

    let root = meta
        .packages
        .get(0)
        .ok_or_else(|| anyhow!("Failed to find root package in {path}"))?;
    let name = &root.name;
    let version = &root.version;
    let edition = &root.edition;
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
    let mut writef = |target: &Utf8Path, contents: &str| {
        let fullpath = path.join(target);
        std::fs::write(&fullpath, contents)?;
        let digest =
            openssl::hash::hash(openssl::hash::MessageDigest::sha256(), contents.as_bytes())?;
        let digest = hex::encode(digest);
        checksums.files.insert(target.to_string(), digest);
        Ok::<_, anyhow::Error>(())
    };
    let features = root
        .features
        .iter()
        .map(|(k, _)| (k.clone(), Vec::new()))
        .collect();
    let new_manifest = CargoManifest {
        package: CargoPackage {
            name: name.to_string(),
            edition: edition.to_string(),
            version: version.to_string(),
        },
        features,
    };
    let new_manifest = toml::to_string(&new_manifest)?;
    // An empty Cargo.toml
    writef(Utf8Path::new("Cargo.toml"), &new_manifest)?;
    // And an empty source file
    writef(Utf8Path::new("src/lib.rs"), "")?;
    // Finally, serialize the new checksums
    let mut w = std::fs::File::create(checksums_path).map(std::io::BufWriter::new)?;
    serde_json::to_writer(&mut w, &checksums)?;
    w.flush()?;
    Ok(())
}

impl VendorFilter {
    /// Parse a value from `package.metadata.vendor-filter`.
    fn parse_json(meta: &serde_json::Value) -> Result<Option<Self>> {
        let meta = meta.as_object().and_then(|o| o.get(CONFIG_KEY));
        let meta = if let Some(m) = meta {
            m
        } else {
            return Ok(None);
        };
        let v: Self = serde_json::from_value(meta.clone())?;
        Ok(Some(v))
    }

    /// Parse the subset of CLI arguments that affect vendor content into a filter.
    fn parse_args(args: &Args) -> Result<Option<Self>> {
        let args_unset = args.platform.is_none()
            && args.all_features.is_none()
            && args.exclude_crate_path.is_none();
        let exclude_crate_paths = args
            .exclude_crate_path
            .as_ref()
            .map(|v| {
                v.iter()
                    .map(|e| CrateExclude::parse_str(e))
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let r = (!args_unset).then(|| Self {
            platforms: args.platform.clone(),
            all_features: args.all_features,
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
    let meta = new_metadata_cmd(args);
    let meta = meta
        .exec()
        .context("Executing cargo metadata (first run)")?;
    meta.root_package()
        .and_then(|r| VendorFilter::parse_json(&r.metadata).transpose())
        .transpose()
}

/// Given a crate, remove matching files/directories in excludes.
fn process_excludes(path: &Utf8PathBuf, name: &str, excludes: &[&str]) -> Result<()> {
    let mut matched = false;
    for exclude in excludes.iter().map(Utf8Path::new) {
        if exclude.is_absolute() {
            anyhow::bail!("Invalid absolute path in crate exclude {name} {exclude}");
        }
        let path = path.join(exclude);

        if path.exists() {
            std::fs::remove_dir_all(path)?;
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
        .args(&["log", "-1", "--pretty=%ct"])
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

    Command::new("tar")
        .args(&[
            "-c",
            "-C",
            srcdir.as_str(),
            "--sort=name",
            "--owner=0",
            "--group=0",
            "--numeric-owner",
            "--pax-option=exthdr.name=%d/PaxHeaders/%f,delete=atime,delete=ctime",
        ])
        .args(compress.tar_switch())
        .args(prefix.map(|prefix| format!("--transform=s,^.,./{prefix},")))
        .arg(format!("--mtime=@{source_date_epoch}"))
        .args(["-f", dest.as_str(), "."])
        .status()
        .context("Failed to execute tar")?;
    Ok(())
}

fn new_metadata_cmd(args: &Args) -> MetadataCommand {
    let mut command = MetadataCommand::new();
    if args.offline {
        command.other_options(vec![OFFLINE.to_string()]);
    }
    if let Some(path) = args.manifest_path.as_deref() {
        command.manifest_path(path);
    }
    command
}

fn add_packages_for_platform(
    args: &Args,
    config: &VendorFilter,
    packages: &mut HashMap<cargo_metadata::PackageId, cargo_metadata::Package>,
    platform: Option<&str>,
) -> Result<()> {
    let mut command = new_metadata_cmd(args);

    if config.all_features.unwrap_or_default() {
        command.features(AllFeatures);
    }

    if let Some(platform) = platform {
        command.other_options(vec![format!("--filter-platform={platform}")]);
    }
    let meta = command.exec().context("Executing cargo metadata")?;
    for package in meta.packages {
        packages.insert(package.id.clone(), package);
    }
    Ok(())
}

fn get_root_package(args: &Args) -> Result<Option<Package>> {
    let mut command = new_metadata_cmd(args);
    command.no_deps();

    let meta = command.exec().context("Executing cargo metadata")?;
    Ok(meta.root_package().cloned())
}

/// Parse the output of `rustc --print target-list`
fn get_target_list() -> Result<HashSet<String>> {
    let o = Command::new("rustc")
        .args(["--print", "target-list"])
        .output()
        .context("Failed to invoke rustc --print target-list")?;
    let buf = String::from_utf8(o.stdout)?;
    Ok(buf.lines().map(|s| s.trim().to_string()).collect())
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

fn expand_platforms<'a, 'b>(
    platforms: &'b [&'b str],
    target_list: &'a [(&str, ParsedPlatform)],
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

    let tempdir = match args.format {
        OutputTarget::Tar | OutputTarget::TarGzip | OutputTarget::TarZstd => {
            Some(tempfile::tempdir_in(".")?)
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

    let mut packages = HashMap::new();
    let have_platform_filters = config
        .platforms
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or_default();
    let mut expanded_platforms = None;
    if have_platform_filters {
        eprintln!("Gathering metadata for platforms");
        let target_list = get_target_list()?;
        let target_list: Vec<(&str, ParsedPlatform)> = target_list
            .iter()
            .map(|platform| (platform.as_str(), platform.split('-').collect()))
            .collect();
        let platforms: Vec<_> = config
            .platforms
            .iter()
            .flatten()
            .map(|s| s.as_str())
            .collect();
        let platforms = expand_platforms(&platforms, &target_list)?;
        for platform in platforms.iter() {
            add_packages_for_platform(&args, &config, &mut packages, Some(platform))?;
        }
        expanded_platforms = Some(platforms);
    } else {
        add_packages_for_platform(&args, &config, &mut packages, None)?;
    }

    // Run `cargo vendor` which will capture all dependencies.
    let manifest_path = args
        .manifest_path
        .as_ref()
        .map(|o| ["--manifest-path", o.as_str()]);
    let status = Command::new("cargo")
        .args(&["vendor"])
        .args(args.offline.then(|| OFFLINE))
        .args(manifest_path.iter().flatten())
        .arg(&*output_dir)
        .status()?;
    if !status.success() {
        anyhow::bail!("Failed to execute cargo vendor: {:?}", status);
    }

    let root = get_root_package(&args)?;

    // Create a mapping of name -> [package versions]
    let mut pkgs_by_name = BTreeMap::<_, Vec<_>>::new();
    for pkg in packages.values() {
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
        let name_version = format!("{name}-{}", pkg.version);
        v.push((name_version, pkg));
    }

    // Split pkgs_by_name into ones that actually have multiple versions or not.
    let mut unversioned_packages = BTreeMap::new();
    let mut multiversioned_packages = BTreeMap::new();
    for (name, versions) in pkgs_by_name {
        let mut versions = versions.into_iter().peekable();
        let first = versions.next().unwrap();
        if versions.peek().is_some() {
            for (version, pkg) in std::iter::once(first).chain(versions) {
                multiversioned_packages.insert(version, pkg);
            }
        } else {
            assert!(unversioned_packages.insert(name, first.1).is_none());
        }
    }

    let mut package_filenames = BTreeMap::new();
    for (name, pkg) in unversioned_packages {
        let name_path = output_dir.join(name);
        if !name_path.exists() {
            anyhow::bail!("Failed to find vendored dependency: {name}");
        }
        package_filenames.insert(name.to_string(), pkg);
    }

    // When writing out packages that have multiple versions, `cargo vendor`
    // appears to use an algorithm where the first (or highest version?)
    // is just $name, then all other versions end up as $name-$version.
    // We build up a map of those here to their original package.
    for (namever, pkg) in multiversioned_packages {
        let name = &pkg.name;
        let namever_path = output_dir.join(&namever);
        let name_path = output_dir.join(name);
        if namever_path.exists() {
            package_filenames.insert(namever, pkg);
        } else if name_path.exists() {
            package_filenames.insert(pkg.name.to_string(), pkg);
        } else {
            anyhow::bail!("Failed to find vendored dependency: {namever}");
        }
    }

    // Index the excludes into a mapping from crate name -> [list of excludes].
    let excludes = config
        .exclude_crate_paths
        .as_deref()
        .unwrap_or_default()
        .iter()
        .try_fold(HashMap::<&str, Vec<&str>>::new(), |mut m, v| {
            let name = v.name.as_str();
            let exclude = v.exclude.as_str();
            let a = m.entry(name).or_default();
            a.push(exclude);
            Ok::<_, anyhow::Error>(m)
        })?;

    // A reusable buffer (silly optimization to avoid allocating lots of path buffers)
    let mut pbuf = Utf8PathBuf::from(&*output_dir);
    let mut unreferenced = HashSet::new();

    // Find and physically delete unreferenced packages, and apply filters.
    for entry in output_dir.read_dir_utf8()? {
        let entry = entry?;
        let name = entry.file_name();
        pbuf.push(name);

        if !package_filenames.contains_key(name) {
            replace_with_stub(&pbuf, args.offline)
                .with_context(|| format!("Replacing with stub: {name}"))?;
            eprintln!("Replacing unreferenced package with stub: {name}");
            assert!(unreferenced.insert(name.to_string()));
        }

        if let Some(excludes) = excludes.get(name) {
            process_excludes(&pbuf, name, excludes)?;
        }

        let r = pbuf.pop();
        debug_assert!(r);
    }

    // For tar archives, generate them now from the temporary directory.
    let prefix = args.prefix.as_deref();
    match args.format {
        OutputTarget::Tar => {
            generate_tar_from(&*output_dir, &final_output_path, prefix, Compression::None)?
        }
        OutputTarget::TarGzip => {
            generate_tar_from(&*output_dir, &final_output_path, prefix, Compression::Gzip)?
        }
        OutputTarget::TarZstd => {
            generate_tar_from(&*output_dir, &final_output_path, prefix, Compression::Zstd)?
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
        json!({ "platforms": ["aarch64-unknown-linux-gnu"], "all-features": true}),
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
