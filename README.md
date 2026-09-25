# `cargo vendor`, but with filtering

The core `cargo vendor` tool is useful to save all dependencies.
However, it doesn't offer any filtering; today cargo includes
all platforms, but some projects only care about Linux
for example.

More information: <https://github.com/rust-lang/cargo/issues/7058>

Additionally some projects are not interested by vendoring test code
or development dependencies of used crates and these filters
are also not supported with no development planned yet.

More information: <https://github.com/rust-lang/cargo/issues/13474>
or <https://github.com/rust-lang/cargo/issues/7065>

## Generating a vendor/ directory with filtering

Here's a basic example which filters out all crates that don't target Linux;
for example this will drop out crates like `winapi-x86_64-pc-windows-gnu` and
`core-foundation` that are Windows or MacOS only.

```sh
$ cargo vendor-filterer --platform=x86_64-unknown-linux-gnu
```

You may instead want to filter by tiers:

```sh
$ cargo vendor-filterer --tier=2
```

Currently this will drop out crates such as `redox_syscall`.

An existing output directory is left alone unless `--overwrite` is given. It is
then replaced, but only removed once vendoring has succeeded, so a failed run
leaves it as it was.

You can also declaratively specify the desired vendor configuration via the [Cargo metadata](https://doc.rust-lang.org/cargo/reference/manifest.html#the-metadata-table)
key `package.metadata.vendor-filter`.  In this example, we include only tier 1 and 2 Linux platforms, and additionally remove some vendored C sources, `tests` folders
and development dependencies from all crates:

```toml
[package.metadata.vendor-filter]
platforms = ["*-unknown-linux-gnu"]
tier = "2"
all-features = true
keep-dep-kinds = "no-dev"
exclude-crate-paths = [ { name = "curl-sys", exclude = "curl" },
                        { name = "libz-sys", exclude = "src/zlib" },
                        { name = "libz-sys", exclude = "src/*.c" },
                        { name = "libz-sys", exclude = "src/zlib-ng" },
                        { name = "ring", exclude = "pregenerated/*.o" },
                        { name = "*", exclude = "tests" },
                        { name = "*", exclude = "*.md" },
                      ]
```

For workspaces, use the corresponding [workspace metadata](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-metadata-table)
key `workspace.metadata.vendor-filter`.

### Available options for for `package.metadata.vendor-filter` in Cargo.toml

- `platforms`: List of rustc target triples; this is the same values accepted by
  e.g. `cargo metadata --filter-platform`.  You can specify multiple values,
  and `*` wildcards are supported.  For example, `*-unknown-linux-gnu`.
- `tier`: This can be either "1" or "2".  It may be specified in addition to `platforms`.
- `all-features`: Enable all features of the current crate and opt into
  resolved-graph filtering.
- `no-default-features`: Do not enable the current crate's `default` feature when
  resolving packages to retain. This also opts into resolved-graph filtering.
- `features`: Enable these features of the current crate when resolving packages
  to retain. An omitted key retains the historical package-catalog behavior;
  `features = []` instead opts into resolved-graph filtering with Cargo's default
  features. A nonempty list opts in and enables those features.
- `keep-dep-kinds`: Specify which dependencies kinds to keep.
  Can be one of: all, normal, build, dev, no-normal, no-build, no-dev
- `exclude-crate-paths`: Remove files and directories from target crates.  A key
  use case for this is removing the vendored copy of C libraries embedded in
  crates like `libz-sys`, when you only want to support dynamically linking.
  `*` wildcard removes the folder from all creates (typical use case for `tests` folder).
  Supports glob patterns like `*.o`, `src/*.c`, or `**/*.a` for pattern-based exclusions.

These options have corresponding CLI flags; see `cargo vendor-filterer --help`.
From a shell, `--features ''` is equivalent to `features = []`.

## Packages replaced with a stub

A package which is filtered out stays in the vendor directory as a stub: its
`Cargo.toml` and an empty `src/lib.rs`, so that `Cargo.lock` still resolves.
The generated `Cargo.toml` starts with a comment saying where it comes from,
marks the package as a stub and lists the keys removed from its `[package]`
section. Those keys have to go, because cargo would look for a build script or
binaries the stub does not have:

```toml
[package.metadata.vendor-filter]
stub = true

[package.metadata.vendor-filter.removed-package-keys]
build = "build/main.rs"
links = "openssl"
```

A stub keeps the manifests it was generated from, so that nothing filtered out
is lost:

| File | Contents |
| ---- | -------- |
| `Cargo.toml` | the manifest after `cargo vendor` and the filtering |
| `Cargo.toml.pre-vendor-filter` | the manifest as `cargo vendor` wrote it |
| `Cargo.toml.orig` | the manifest as written by the developer, which every vendored package carries |

With `--json=PATH`, the packages replaced with a stub are also listed in a JSON
file, so that tooling does not have to walk the vendor directory to find them:

```json
{
  "stubs": [
    "libmimalloc-sys",
    "windows_x86_64_msvc"
  ]
}
```

The paths are relative to the vendor directory; what was filtered out of a
package is in its own `Cargo.toml.pre-vendor-filter`.

## Generating reproducible vendor tarballs

You can also provide `--format=tar.zstd` to output a reproducible tar archive
compressed via zstd; the default filename will be `vendor.tar.zstd`.  Similarly
there is `--format=tar.gz` for gzip, and `--format=tar` to output an uncompressed tar archive, which you
can compress however you like.  It's also strongly recommended to use `--prefix=vendor`
which has less surprising behavior when unpacked in e.g. a home directory.  For example,
`--prefix=vendor --format=tar.zstd` together.

This option requires `SOURCE_DATE_EPOCH` set in the environment, or an external `git` and the working directory must be a git repository.

With `--format=tar.zstd`, this currently requires an external `zstd` binary.

This uses the suggested logic from https://reproducible-builds.org/docs/archives/
to output a reproducible archive; in other words, another process/tool
can also perform a `git clone` of your project and regenerate the vendor
tarball using the same version of `cargo vendor-filterer` to verify it.
