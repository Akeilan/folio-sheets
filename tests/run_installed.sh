#!/bin/sh
# Run a script inside the installed Folio Sheet, the way every other app in the suite does:
#   run_installed.sh /path/to/script.py
# The suite's release gate (X4-05) uses this to have the app write down its own surface.
HERE="$(cd "$(dirname "$0")" && pwd)"
S="${1:?a script to run}"
export FOLIO_TEST_SCRIPT="$S"
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$(mktemp -d)}" HOME="${SHEET_TEST_HOME:-$(mktemp -d)}"
export FOLIO_TESTS="$HERE"
exec timeout "${UI_TIMEOUT:-300}" folio-sheet
