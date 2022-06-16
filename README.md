# `cargo vendor`, but with filtering

The core `cargo vendor` tool is useful to save all dependencies.
However, it doesn't offer any filtering; today cargo includes
all platforms, but some projects only care about Linux
for example.

More information: https://github.com/rust-lang/cargo/issues/7058

# Generating a vendor/ directory that filters out non-Linux crates

```
$ cargo vendor-filterer --linux-only
```

# TODO

- Support e.g. `--target-os=linux --target-os=wasm` to do both, but not Windows

