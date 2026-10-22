#!/bin/sh
# Rebuilds folio-sheets_<version>_amd64.deb from this tree.
# Python UI is source (app/); the Rust engine is built from engine/ (engine/build.sh).
# prebuilt/ holds the old 0.2 engine binary, used only as a test oracle.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
VERSION="$(sed -n 's/^Version: //p' "$HERE/packaging/DEBIAN/control")"
OUT="${1:-$HERE}"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp -a "$HERE/packaging/root/." "$STAGE/"
mkdir -p "$STAGE/usr/lib/folio-sheets"
cp -a "$HERE/app/folio_sheets" "$STAGE/usr/lib/folio-sheets/"
find "$STAGE/usr/lib/folio-sheets" -name '__pycache__' -prune -exec rm -rf {} +
ENGINE="${FOLIO_SHEETS_ENGINE:-$HERE/engine/out/folio_sheets_core.so}"
install -m 644 "$ENGINE" "$STAGE/usr/lib/folio-sheets/folio_sheets_core.so"
mkdir -p "$STAGE/DEBIAN"
cp "$HERE/packaging/DEBIAN/"* "$STAGE/DEBIAN/"
chmod 755 "$STAGE/DEBIAN/postinst" "$STAGE/DEBIAN/prerm" "$STAGE/DEBIAN/postrm"
dpkg-deb --root-owner-group --build -Zxz "$STAGE" "$OUT/folio-sheets_${VERSION}_amd64.deb"
echo "Built $OUT/folio-sheets_${VERSION}_amd64.deb"
