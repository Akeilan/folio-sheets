#!/bin/bash
# Every Sheets UI audit and the command walk, from the copy at /tmp/fs (engine/sync_app.sh) or,
# with --installed, against the installed package. Prints one line per script.
# DISPLAY must be set (Xvfb).
cd /tmp/fs
PP=app
[ "$1" = "--installed" ] && { PP=/usr/lib/folio-sheets; shift; }
FAIL=0
for t in ${@:-tests/*_audit.py tests/command_walk.py}; do
  out=$(FOLIO_TEST_SCRIPT=$t PYTHONPATH=$PP timeout 900 python3 -m folio_sheets 2>&1)
  rc=$?
  p=$(echo "$out" | grep -c '^PASS')
  f=$(echo "$out" | grep -c '^FAIL')
  extra=""
  if [ "$(basename $t)" = command_walk.py ]; then extra=" $(echo "$out" | grep -E '^ran |^not yet|^ERRORS' | tr '\n' ' ')"; fi
  echo "$(basename $t .py) rc=$rc pass=$p fail=$f$extra"
  if [ $rc != 0 ] || [ $f != 0 ]; then FAIL=1; echo "$out" | grep -E '^FAIL|^ERROR|Traceback|Error' | head -8; fi
done
exit $FAIL
