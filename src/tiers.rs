use std::str::FromStr;

use serde::Deserialize;

/// See https://doc.rust-lang.org/nightly/rustc/platform-support.html#tier-1-with-host-tools
const TIER1: &[&str] = &[
    "aarch64-unknown-linux-gnu",
    "i686-pc-windows-gnu",
    "i686-pc-windows-msvc",
    "i686-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-gnu",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
];

/// See https://doc.rust-lang.org/nightly/rustc/platform-support.html#tier-2-with-host-tools
const TIER2: &[&str] = &[
    "aarch64-apple-darwin",
    "aarch64-pc-windows-msvc",
    "aarch64-unknown-linux-musl",
    "arm-unknown-linux-gnueabi",
    "arm-unknown-linux-gnueabihf",
    "armv7-unknown-linux-gnueabihf",
    "mips-unknown-linux-gnu",
    "mips64-unknown-linux-gnuabi64",
    "mips64el-unknown-linux-gnuabi64",
    "mipsel-unknown-linux-gnu",
    "powerpc-unknown-linux-gnu",
    "powerpc64-unknown-linux-gnu",
    "powerpc64le-unknown-linux-gnu",
    "riscv64gc-unknown-linux-gnu",
    "s390x-unknown-linux-gnu",
    "x86_64-unknown-freebsd",
    "x86_64-unknown-illumos",
    "x86_64-unknown-linux-musl",
    "x86_64-unknown-netbsd",
];

/// The possible values of select Rust platform "tiers".
/// There is a third tier, but this API is about limited/curated tiers.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) enum Tier {
    One,
    Two,
}

impl Tier {
    /// List the targets for this tier.
    pub(crate) fn targets(&self) -> impl Iterator<Item = &'static str> {
        match self {
            Tier::One => either::Left(TIER1.iter()),
            Tier::Two => either::Right(TIER1.iter().chain(TIER2.iter())),
        }
        .copied()
    }
}

impl FromStr for Tier {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let r = match s {
            "1" => Self::One,
            "2" => Self::Two,
            o => anyhow::bail!("Invalid tier {o}"),
        };
        Ok(r)
    }
}
