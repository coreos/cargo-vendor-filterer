# `cargo vendor`, but with filtering

The core `cargo vendor` tool is useful to save all dependencies.
However, it doesn't offer any filtering; today cargo includes
all platforms, but some projects only care about Linux
for example.

More information: https://github.com/rust-lang/cargo/issues/7058

# Generating a vendor/ directory with filtering

Here's a basic example which filters out all crates that don't target Linux;
for example this will drop out crates like `winapi-x86_64-pc-windows-gnu` and
`core-foundation` that are Windows or MacOS only.

```
$ cargo vendor-filterer --platform=x86_64-unknown-linux-gnu
```

You may instead want to filter by tiers:

```
$ cargo vendor-filterer --tier=2
```

Currently this will drop out crates such as `redox_syscall`.

You can also declaratively specify the desired vendor configuration via the [Cargo metadata](https://doc.rust-lang.org/cargo/reference/manifest.html#the-metadata-table)
key `package.metadata.vendor-filter`.  In this example, we include only tier 1 and 2 Linux platforms, and additionally remove some vendored C sources:

```
[package.metadata.vendor-filter]
platforms = ["*-unknown-linux-gnu"]
tier = "2"
all-features = true
exclude-crate-paths = [ { name = "curl-sys", exclude = "curl" },
                        { name = "libz-sys", exclude = "src/zlib" },
                        { name = "libz-sys", exclude = "src/zlib-ng" },
                      ]
```

For workspaces, use the corresponding [workspace metadata](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-metadata-table)
key `workspace.metadata.vendor-filter`.

## Available options for for `package.metadata.vendor-filter` in Cargo.toml

- `platforms`: List of rustc target triples; this is the same values accepted by
  e.g. `cargo metadata --filter-platform`.  You can specify multiple values,
  and `*` wildcards are supported.  For example, `*-unknown-linux-gnu`.
- `tier`: This can be either "1" or "2".  It may be specified in addition to `platforms`.
- `all-features`: Enable all features of the current crate when vendoring.
- `exclude-crate-paths`: Remove files and directories from target crates.  A key
  use case for this is removing the vendored copy of C libraries embedded in
  crates like `libz-sys`, when you only want to support dynamically linking.

All of these options have corresponding CLI flags; see `cargo vendor-filterer --help`.

# Generating reproducible vendor tarballs

You can also provide `--format=tar.zstd` to output a reproducible tar archive
compressed via zstd; the default filename will be `vendor.tar.zstd`.  Similarly
there is `--format=tar.gz` for gzip, and `--format=tar` to output an uncompressed tar archive, which you
can compress however you like.  It's also strongly recommended to use `--prefix=vendor`
which has less surprising behavior when unpacked in e.g. a home directory.  For example,
`--prefix=vendor --format=tar.zstd` together.

This option requires:

 - An external GNU `tar` program
 - An external `gzip` or `zstd` program (for `--format=tar.gz` and `--format=tar.zstd` respectively)
 - `SOURCE_DATE_EPOCH` set in the environment, or an external `git` and the working directory must be a git repository

This uses the suggested code from https://reproducible-builds.org/docs/archives/
to output a reproducible archive; in other words, another process/tool
can also perform a `git clone` of your project and regenerate the vendor
tarball to verify it.
