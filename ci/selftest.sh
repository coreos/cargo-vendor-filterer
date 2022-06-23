#!/bin/bash
# Clone our own git repository and vendor our own sources
# using --linux-only, and verify that `winapi` is just a stub.
set -xeuo pipefail
srcdir=$(git rev-parse --show-toplevel)
tmpd=$(mktemp -d)
tmp_git=${tmpd}/self
git clone ${srcdir} ${tmp_git}
cd ${tmp_git}

# This exists right now, but we will test removing it
hex_benches=hex/benches

verify_no_windows() {
    (cd $1
     test $(stat --printf="%s" winapi/src/lib.rs) = 0
     test $(ls winapi/src | wc -l) = 1
    )
}

echo "Verifying linux"
cargo-vendor-filterer --platform=x86_64-unknown-linux-gnu  --platform=aarch64-unknown-linux-gnu --exclude-crate-path='hex#benches' target/vendor
test '!' -d vendor # We overrode the default
verify_no_windows target/vendor
test '!' -d "${hex_benches}"
rm target/vendor -rf
echo "ok linux only"

echo "Verifying linux as subcommand"
cargo vendor-filterer --platform=x86_64-unknown-linux-gnu
verify_no_windows vendor
test '!' -f vendor.tar.zstd
rm vendor -rf
echo "ok linux only subcommand"

echo "Verifying linux + output to tar zstd"
cargo vendor-filterer --platform=x86_64-unknown-linux-gnu --format=tar.zstd mycrate-5.2.7.tar.zstd
zstdcat mycrate-5.2.7.tar.zstd | tar tf - > out.txt
grep -qF './anyhow' out.txt
rm -v mycrate-5.2.7.tar.zstd out.txt
echo "ok linux + output to tar"

echo "Verifying linux + output to tar"
cargo vendor-filterer --platform=x86_64-unknown-linux-gnu --format=tar
tar tf - < vendor.tar > out.txt
grep -qF './anyhow' out.txt
rm -v vendor.tar out.txt
echo "ok linux + output to tar"


# Default
cargo-vendor-filterer
test -d vendor/"${hex_benches}"
test $(stat --printf="%s" vendor/winapi/src/lib.rs) != 0
rm vendor -rf
echo "ok default"

echo "Verifying via config"
sed -i -e s,'^### ',, Cargo.toml
cargo-vendor-filterer
verify_no_windows vendor
test '!' -d "${hex_benches}"
rm vendor -rf
echo "ok linux only via config"

rm "${tmpd}" -rf
echo "selftest succeeded"
