# `cargo vendor`, but with filtering

The core `cargo vendor` tool is useful to save all dependencies.
However, it doesn't offer any filtering; today cargo includes
all platforms, but some projects only care about Linux
for example.

More information: https://github.com/rust-lang/cargo/issues/7058

# Generating a vendor/ directory that filters out non-Linux crates

```
$ cargo vendor-filterer --platform=x86_64-unknown-linux-gnu
```

# TODO

We only support a single `--platform` right now, so if e.g.
you use `--platform=x86_64-unknown-linux-gnu` and there's a crate
dependency only set on e.g. `aarch64-unknown-linux-gnu`, it
will be missing.

A future enhancement will support something like 
`--platform=*-unknown-linux-gnu --platform=wasm`.
