#!/bin/bash
# Clone our own git repository and vendor our own sources
# using --linux-only, and verify that `winapi` is just a stub.
set -xeuo pipefail
srcdir=$(git rev-parse --show-toplevel)
tmpd=$(mktemp -d)
tmp_git=${tmpd}/self
git clone ${srcdir} ${tmp_git}
cd ${tmp_git}
cargo-vendor-filterer --linux-only
test $(stat --printf="%s" vendor/winapi/src/lib.rs) = 0
test $(ls vendor/winapi/src | wc -l) = 1
echo "ok"
rm "${tmpd}" -rf
