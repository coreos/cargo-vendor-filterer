use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use cargo_metadata::{CargoOpt::AllFeatures, MetadataCommand};
use clap::Parser;
use std::collections::{BTreeMap, HashSet};
use std::process::Command;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum OutputTarget {
    Dir,
    //    Tar,
}

impl clap::ValueEnum for OutputTarget {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Dir]
    }

    fn to_possible_value<'a>(&self) -> Option<clap::PossibleValue<'a>> {
        match self {
            Self::Dir => Some(clap::PossibleValue::new("dir")),
        }
    }
}

/// Enhanced `cargo vendor` with filtering
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Only include crates for a Linux build
    #[clap(long)]
    linux_only: bool,

    /// Exclude the given crates
    #[clap(long)]
    exclude: Vec<String>,

    /// Enable all features
    #[clap(long)]
    all_features: bool,

    #[clap(long, value_parser)]
    format: OutputTarget,

    /// The output path
    #[clap(default_value = "vendor")]
    path: Utf8PathBuf,
}

struct MetadataArgs {
    linux_only: bool,
    all_features: bool,
}

impl From<&Args> for MetadataArgs {
    fn from(args: &Args) -> Self {
        Self {
            linux_only: args.linux_only,
            all_features: args.all_features,
        }
    }
}

// This code derived from https://github.com/rust-secure-code/cargo-supply-chain/blob/master/src/common.rs
fn metadata_command(args: MetadataArgs) -> MetadataCommand {
    let mut command = MetadataCommand::new();
    if args.all_features {
        command.features(AllFeatures);
    }
    // TODO: verify by cross checking all tier1 platforms that the dependency set is exactly
    // the same.
    let args = args
        .linux_only
        .then(|| String::from("--filter-platform=x86_64-unknown-linux-gnu"))
        .into_iter();
    command.other_options(args.collect::<Vec<_>>());
    command
}

fn run() -> Result<()> {
    let args = Args::parse();
    let command = metadata_command((&args).into());
    let meta = command.exec().map_err(anyhow::Error::msg)?;

    let packages = &meta.packages;

    if args.path.exists() {
        anyhow::bail!("Refusing to operate on extant directory: {}", args.path);
    }

    let status = Command::new("cargo")
        .args(&["vendor"])
        .arg(args.path.as_str())
        .status()?;
    if !status.success() {
        anyhow::bail!("Failed to execute cargo vendor: {:?}", status);
    }

    let root = meta.root_package().map(|p| &p.id);

    let mut pkgs_by_name = BTreeMap::<_, Vec<_>>::new();
    for pkg in packages {
        if let Some(rootid) = root {
            if &pkg.id == rootid {
                continue;
            }
        }
        let name = pkg.name.as_str();

        let v = pkgs_by_name.entry(name).or_default();
        let name_version = format!("{name}-{}", pkg.version);
        v.push((name_version, pkg));
    }

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
        let name_path = args.path.join(name);
        if !name_path.exists() {
            anyhow::bail!("Failed to find vendored dependency: {name}");
        }
        package_filenames.insert(name.to_string(), pkg);
    }

    for (namever, pkg) in multiversioned_packages {
        let namever_path = args.path.join(&namever);
        let name_path = args.path.join(&pkg.name);
        if name_path.exists() {
            package_filenames.insert(pkg.name.to_string(), pkg);
        } else if namever_path.exists() {
            package_filenames.insert(namever, pkg);
        } else {
            anyhow::bail!("Failed to find vendored dependency: {namever}");
        }
    }

    let mut pbuf = args.path.clone();
    let mut unreferenced = HashSet::new();
    // First pass, find and physically delete unreferenced packages, also
    // gathering up the set of packages that we deleted.
    for entry in args.path.read_dir_utf8()? {
        let entry = entry?;
        let name = entry.file_name();
        pbuf.push(name);

        if !package_filenames.contains_key(name) {
            println!("Removing unreferenced package: {name}");
            std::fs::remove_dir_all(&pbuf)?;
            assert!(unreferenced.insert(name.to_string()));
        }

        debug_assert!(pbuf.pop());
    }

    // Remove the dependency information for deleted packages
    for entry in args.path.read_dir_utf8()? {
        let entry = entry?;
        let name = entry.file_name();
        pbuf.push(name);
        pbuf.push("Cargo.lock");

        let lockf =
            cargo_lock::Lockfile::load(&pbuf).with_context(|| format!("Failed to load {pbuf}"))?;

        debug_assert!(pbuf.pop());
        debug_assert!(pbuf.pop());
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{:#}", e);
        std::process::exit(1);
    }
}
