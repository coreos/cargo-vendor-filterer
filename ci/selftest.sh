#!/bin/bash
# Clone our own git repository and vendor our own sources
# using --linux-only, and verify that `winapi` is just a stub.
set -xeuo pipefail
srcdir=$(git rev-parse --show-toplevel)
tmpd=$(mktemp -d)
tmp_git=${tmpd}/self
git clone ${srcdir} ${tmp_git}
cd ${tmp_git}

echo "Test errors"
if cargo-vendor-filterer --platform=x86_64-unknown-linux-gnu --platform=aarch64-unknown-linux-gnu 2>err.txt; then
    exit 1
fi
grep -q 'Specifying multiple targets is not currently supported' err.txt
echo "ok errors"

verify_no_windows() {
    test $(stat --printf="%s" vendor/winapi/src/lib.rs) = 0
    test $(ls vendor/winapi/src | wc -l) = 1
}

echo "Verifying linux"
cargo-vendor-filterer --platform=x86_64-unknown-linux-gnu
verify_no_windows
rm vendor -rf
echo "ok linux only"

echo "Verifying linux as subcommand"
cargo vendor-filterer --platform=x86_64-unknown-linux-gnu
verify_no_windows
rm vendor -rf
echo "ok linux only subcommand"

# Default
cargo-vendor-filterer
test $(stat --printf="%s" vendor/winapi/src/lib.rs) != 0
rm vendor -rf
echo "ok default"

echo "Verifying linux via config"
sed -i -e s,'^### ',, Cargo.toml
cargo-vendor-filterer
verify_no_windows
rm vendor -rf
echo "ok linux only via config"

rm "${tmpd}" -rf
