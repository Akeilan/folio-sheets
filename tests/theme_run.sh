#!/bin/sh
# X2-01: ui_theme.py under a dark desktop (app dark, and app set to light) and a loud coloured one.
# theme_run.sh [--installed]   (from the Sheets tree, or against the installed package)
HERE="$(cd "$(dirname "$0")" && pwd)"
PP="$HERE/../app"
[ "$1" = "--installed" ] && PP=/usr/lib/folio-sheet
LOG=$(mktemp)
rc=0
for combo in dark: dark:light colour:; do
  t=${combo%%:*}; f=${combo#*:}
  echo "-- theme $t ${f:+(app $f)}"
  TH=$(python3 -c "import sys; sys.path.insert(0, '$HERE'); import themecheck; print(themecheck.theme_env('$t')['GTK_THEME'])")
  GTK_THEME=$TH FOLIO_THEME=$f FOLIO_TEST_SCRIPT="$HERE/ui_theme.py" PYTHONPATH="$PP" timeout 900 python3 -m folio_sheets >"$LOG" 2>&1 || rc=1
  grep -E "^FAIL|^theme|dialogs checked|checks,|Traceback" "$LOG" | head -60
done
rm -f "$LOG"
exit $rc
