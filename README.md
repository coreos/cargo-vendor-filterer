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

You can also declaratively specify the desired vendor configuration via the [Cargo metadata](https://doc.rust-lang.org/cargo/reference/manifest.html#the-metadata-table)
key `package.metadata.vendor-filter`:

```
[package.metadata.vendor-filter]
platforms = ["x86_64-unknown-linux-gnu"]
all-features = true
exclude-crate-paths = [ { name = "curl-sys", exclude = "curl" },
                        { name = "libz-sys", exclude = "src/zlib" },
                        { name = "libz-sys", exclude = "src/zlib-ng" },
                      ]
```

## Available options for for `package.metadata.vendor-filter` in Cargo.toml

- `platforms`: List of rustc target triples; this is the same values accepted by
  e.g. `cargo metadata --filter-platform`.  At the moment, only one exact platform can be specified
  and wildcards are not supported.
- `all-features`: Enable all features of the current crate when vendoring.
- `exclude-crate-paths`: Remove files and directories from target crates.  A key
  use case for this is removing the vendored copy of C libraries embedded in
  crates like `libz-sys`, when you only want to support dynamically linking.

All of these options have corresponding CLI flags; see `cargo vendor-filterer --help`.

# Generating reproducible vendor tarballs

The output from this project is a directory; however, many projects will
want to serialize this to a single file archive (such as tar) in order
to attach to e.g. a Github/Gitlab release.

For more information on how to do this, see https://reproducible-builds.org/docs/archives/

An example script:

```
#!/usr/bin/bash
set -xeuo pipefail
# Vendor dependencies; this assumes you are using metadata in Cargo.toml
cargo vendor-filterer
# Gather the timestamp from git; https://reproducible-builds.org/docs/source-date-epoch/
SOURCE_DATE_EPOCH=$(git log -1 --pretty=%ct)
# Example from https://reproducible-builds.org/docs/archives/ modified to also use zstd compression
tar --sort=name \
      --mtime="@${SOURCE_DATE_EPOCH}" \
      --owner=0 --group=0 --numeric-owner \
      --pax-option=exthdr.name=%d/PaxHeaders/%f,delete=atime,delete=ctime \
      --zstd \
      -cf vendor.tar.zstd vendor
rm vendor -rf
```

# TODO

We only support a single `--platform` right now, so if e.g.
you use `--platform=x86_64-unknown-linux-gnu` and there's a crate
dependency only set on e.g. `aarch64-unknown-linux-gnu`, it
will be missing.

A future enhancement will support something like 
`--platform=*-unknown-linux-gnu --platform=wasm`.
