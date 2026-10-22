#!/bin/bash
# Build the engine (release) in the Ubuntu 22.04 build root and put the module at
# engine/out/folio_sheets_core.so. Run the Rust tests first.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
"$HERE/b.sh" test --no-default-features --lib | tail -3
"$HERE/b.sh" build --release | tail -2
mkdir -p "$HERE/out"
cp /srv/jammy/src/fse/target/release/libfolio_sheets_core.so "$HERE/out/folio_sheets_core.so"
echo "engine: $HERE/out/folio_sheets_core.so"
