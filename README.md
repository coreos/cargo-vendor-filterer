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

# TODO

We only support a single `--platform` right now, so if e.g.
you use `--platform=x86_64-unknown-linux-gnu` and there's a crate
dependency only set on e.g. `aarch64-unknown-linux-gnu`, it
will be missing.

A future enhancement will support something like 
`--platform=*-unknown-linux-gnu --platform=wasm`.
