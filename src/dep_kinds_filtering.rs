use crate::{Args, VendorFilter};
use anyhow::{Context, Result};
use camino::Utf8Path;
use clap::{builder::PossibleValue, ValueEnum};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

/// Kinds of dependencies that shall be included.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DepKinds {
    All,
    Normal,
    Build,
    Dev,
    NoNormal,
    NoBuild,
    NoDev,
}

impl ValueEnum for DepKinds {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::All,
            Self::Normal,
            Self::Build,
            Self::Dev,
            Self::NoNormal,
            Self::NoBuild,
            Self::NoDev,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Self::All => PossibleValue::new("all"),
            Self::Normal => PossibleValue::new("normal"),
            Self::Build => PossibleValue::new("build"),
            Self::Dev => PossibleValue::new("dev"),
            Self::NoNormal => PossibleValue::new("no-normal"),
            Self::NoBuild => PossibleValue::new("no-build"),
            Self::NoDev => PossibleValue::new("no-dev"),
        })
    }
}

impl std::fmt::Display for DepKinds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value()
            .expect("No values are skipped")
            .get_name()
            .fmt(f)
    }
}

/// Filter out unwanted dependency kinds.
///
/// Replicates logic from add_packages_for_platform() but uses cargo tree
/// because cargo metadata does not implement dependency kinds filtering.
/// Ref: <https://github.com/rust-lang/cargo/issues/10718>
/// Cargo tree is NOT intended for automatic processing so this function
/// explicitly does not replace the add_packages_for_platform() entirely.
pub(crate) fn filter_dep_kinds(
    args: &Args,
    config: &VendorFilter,
    packages: &mut HashMap<cargo_metadata::PackageId, &cargo_metadata::Package>,
    platform: Option<&str>,
) -> Result<()> {
    // exit early when no dependency kinds filtering is requested
    match config.keep_dep_kinds {
        None | Some(DepKinds::All) => return Ok(()),
        Some(_) => (),
    };

    let required_packages = get_required_packages(
        &args.get_all_manifest_paths(),
        args.offline,
        config,
        platform,
    )?;

    packages.retain(|_, package| {
        required_packages.contains(&(
            Cow::Borrowed(&package.name),
            Cow::Borrowed(&package.version),
        ))
    });
    Ok(())
}

/// Returns the set of required packages to satisfy filters specified in config
fn get_required_packages<'a>(
    manifest_paths: &[Option<&Utf8Path>],
    offline: bool,
    config: &VendorFilter,
    platform: Option<&str>,
) -> Result<HashSet<(Cow<'a, str>, Cow<'a, cargo_metadata::semver::Version>)>> {
    let keep_dep_kinds = config.keep_dep_kinds.expect("keep_dep_kinds not set");
    let mut required_packages = HashSet::new();
    for manifest_path in manifest_paths {
        let mut cargo_tree = std::process::Command::new("cargo");
        cargo_tree
            .arg("tree")
            .args(["--quiet", "--prefix", "none"]) // ignore non-relevant output
            .args(["--edges", &keep_dep_kinds.to_string()]); // key filter not available with metadata
        if offline {
            cargo_tree.arg("--offline");
        }
        if let Some(manifest_path) = manifest_path {
            cargo_tree.args(["--manifest-path", manifest_path.as_str()]);
        }
        if config.all_features {
            cargo_tree.arg("--all-features");
        }
        if config.no_default_features {
            cargo_tree.arg("--no-default-features");
        }
        if !config.features.is_empty() {
            cargo_tree.arg("--features").args(&config.features);
        }
        match platform {
            Some(platform) => cargo_tree.arg(format!("--target={platform}")),
            None => {
                // different than in cargo metadata the default is current platform only
                cargo_tree.arg("--target=all")
            }
        };
        let output = cargo_tree.output()?;
        if !output.status.success() {
            anyhow::bail!(
                "Failed to execute cargo tree: {:?}",
                String::from_utf8(output.stderr).expect("Invalid cargo tree output")
            );
        }
        let output_str = String::from_utf8(output.stdout).expect("Invalid cargo tree output");
        for line in output_str.lines() {
            let tokens: Vec<&str> = line.split(' ').collect();
            let [package, version, ..] = tokens.as_slice() else {
                anyhow::bail!("Invalid output received from cargo tree: {line}");
            };
            if version.len() < 5 || version.contains("feature") {
                continue; // skip invalid entries and "feature" list
            }
            // need to remove the initial "v" character that the cargo tree is printing in package name
            // Ref: <https://doc.rust-lang.org/cargo/commands/cargo-tree.html>
            // The PR requesting the v to be removed (or configurable) was closed:
            // <https://github.com/rust-lang/cargo/issues/13120>
            let version = version
                .strip_prefix('v')
                .with_context(|| format!("Invalid version: {}", tokens[1]))?;
            let version = cargo_metadata::semver::Version::parse(version)
                .with_context(|| format!("Cannot parse version {version} for {package}"))?;
            required_packages.insert((Cow::Owned(package.to_string()), Cow::Owned(version)));
        }
    }
    Ok(required_packages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use serde_json::json;

    #[test]
    fn test_dep_kind_dev_only() {
        let mut own_cargo_toml = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        own_cargo_toml.push("Cargo.toml");
        let rp = get_required_packages(
            &[Some(&own_cargo_toml)],
            false,
            &serde_json::from_value(json!({ "keep-dep-kinds": "dev"})).unwrap(),
            Some("x86_64-pc-windows-gnu"),
        );
        match rp {
            Ok(rp) => assert_eq!(rp.len(), 3), // own package + once_cell + serial_test dev dependencies
            Err(e) => panic!("Got error: {e:?}"),
        }
    }

    #[test]
    fn test_dep_kind_all_number() {
        let mut own_cargo_toml = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        own_cargo_toml.push("Cargo.toml");
        let rp = get_required_packages(
            &[Some(&own_cargo_toml)],
            false,
            &serde_json::from_value(json!({ "keep-dep-kinds": "all", "--all-features": true}))
                .unwrap(),
            None, // all platforms
        );
        match rp {
            Ok(rp) => assert!(rp.len() > 90), // all features, all platforms list is long
            Err(e) => panic!("Got error: {e:?}"),
        }
    }

    #[test]
    fn test_dep_kind_normal_vs_no_build() {
        let mut own_cargo_toml = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        own_cargo_toml.push("Cargo.toml");

        let rp_normal = get_required_packages(
            &[Some(&own_cargo_toml)],
            false,
            &serde_json::from_value(json!({ "keep-dep-kinds": "normal"})).unwrap(),
            Some("x86_64-pc-windows-gnu"),
        );

        // no-build => normal + dev dependencies, so including once_call, serial_test...
        let rp_no_build = get_required_packages(
            &[Some(&own_cargo_toml)],
            false,
            &serde_json::from_value(json!({ "keep-dep-kinds": "no-build"})).unwrap(),
            Some("x86_64-pc-windows-gnu"),
        );

        // if once_cell is also a normal dependency, it is not removed from the list
        match (rp_normal, rp_no_build) {
            (Ok(rp_normal), Ok(rp_no_build)) => assert!(
                rp_normal.len() < rp_no_build.len(),
                "Filtering does not work. Got {} normal and {} no-build dependencies",
                rp_normal.len(),
                rp_no_build.len()
            ),
            _ => panic!("One of get_required_packages() calls failed"),
        }
    }

    #[test]
    fn test_dep_kind_build_vs_no_dev() {
        let mut own_cargo_toml = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        own_cargo_toml.push("Cargo.toml");

        let rp_build = get_required_packages(
            &[Some(&own_cargo_toml)],
            false,
            &serde_json::from_value(json!({ "keep-dep-kinds": "build"})).unwrap(),
            Some("x86_64-unknown-linux-gnu"),
        );

        // no-dev => build + normal so the list shall be larger
        let rp_no_dev = get_required_packages(
            &[Some(&own_cargo_toml)],
            false,
            &serde_json::from_value(json!({ "keep-dep-kinds": "no-dev"})).unwrap(),
            Some("x86_64-unknown-linux-gnu"),
        );
        match (rp_build, rp_no_dev) {
            (Ok(rp_build), Ok(rp_no_dev)) => assert!(
                rp_build.len() < rp_no_dev.len(),
                "Filtering does not work. Got {} build and {} no-dev dependencies",
                rp_build.len(),
                rp_no_dev.len()
            ),
            _ => panic!("One of get_required_packages() calls failed"),
        }
    }
}
