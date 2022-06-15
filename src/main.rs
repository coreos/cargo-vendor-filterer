use anyhow::Result;
use cargo_metadata::{CargoOpt::AllFeatures, MetadataCommand, Package, PackageId};
use clap::Parser;
use std::collections::BTreeMap;

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
}

type Packages = BTreeMap<PackageId, Package>;

// This code derived from https://github.com/rust-secure-code/cargo-supply-chain/blob/master/src/common.rs
fn metadata_command(args: Args) -> MetadataCommand {
    let mut command = MetadataCommand::new();
    if args.all_features {
        command.features(AllFeatures);
    }
    let args = args
        .linux_only
        .then(|| String::from("--filter-platform=x86_64-unknown-linux-gnu"))
        .into_iter();
    command.other_options(args.collect::<Vec<_>>());
    command
}

fn sourced_dependencies(metadata_args: Args) -> Result<Packages> {
    let command = metadata_command(metadata_args);
    let meta = command.exec().map_err(anyhow::Error::msg)?;
    let r = meta
        .packages
        .iter()
        .map(|package| (package.id.clone(), package.clone()))
        .collect();
    Ok(r)
}

fn run() -> Result<()> {
    let args = Args::parse();
    let packages = sourced_dependencies(args)?;

    println!("{:?}", packages.keys());
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{:#}", e);
        std::process::exit(1);
    }
}
