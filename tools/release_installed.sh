#!/bin/bash
# Installed-package tests for Folio Sheet (folio-sheets before 0.19) in every test machine:
#   sheets_installed.sh VERSION [display]
# First the upgrade: the last folio-sheets release (0.18.0) installed, then folio-sheet and the
# transitional folio-sheets on top; then every audit against the installed app, the desktop-theme
# run, and on 24.04 the same theme run under Wayland.
V=$1; D=${2:-98}; OLD=${OLD_DEB:-/home/claude/out/folio-sheets_0.18.0_amd64.deb}
/home/claude/mounts.sh
for m in ${MACHINES:-/srv/jammy /srv/nolo /srv/noble}; do
  cp /home/claude/out/folio-sheet_${V}_amd64.deb /home/claude/out/folio-sheets_${V}_all.deb $m/tmp/
  [ -f "$OLD" ] && cp "$OLD" $m/tmp/old-folio-sheets.deb
  echo "== upgrade in $m"
  flock /home/claude/.dpkg.lock chroot $m bash -c "export DEBIAN_FRONTEND=noninteractive
    dpkg -r folio-sheet >/dev/null 2>&1; dpkg -r folio-sheets >/dev/null 2>&1
    if [ -f /tmp/old-folio-sheets.deb ]; then dpkg -i /tmp/old-folio-sheets.deb >/dev/null 2>&1 || apt-get -y -f install >/dev/null 2>&1; fi
    echo \"before: \$(dpkg-query -W -f='\${Package} \${Version}' folio-sheets 2>/dev/null)\"
    apt-get install -y --no-install-recommends /tmp/folio-sheet_${V}_amd64.deb /tmp/folio-sheets_${V}_all.deb >/tmp/up.log 2>&1 || echo 'FAIL upgrade (see /tmp/up.log)'
    echo \"after: \$(dpkg-query -W -f='\${Package} \${Version} \${Status}; ' folio-sheet folio-sheets 2>/dev/null)\"
    echo \"new command: \$(folio-sheet --version)  old command: \$(folio-sheets --version)\"
    [ -f /usr/share/applications/org.folio.Sheet.desktop ] && grep -q NoDisplay=true /usr/share/applications/org.folio.Sheets.desktop && echo 'desktop files: ok' || echo 'desktop files: FAIL'
    [ -d /usr/lib/folio-sheets ] && echo 'FAIL old install dir left behind' || echo 'old install dir gone: ok'"
  rm -rf $m/tmp/fsi; mkdir -p $m/tmp/fsi; cp -r /home/claude/folio-sheets/tests $m/tmp/fsi/
  echo "== installed in $m"
  chroot $m bash -c "cd /tmp/fsi; sed -i 's|^cd /tmp/fs$|cd /tmp/fsi|' tests/run_all.sh; DISPLAY=:$D bash tests/run_all.sh --installed 2>&1 | grep -v dbind; DISPLAY=:$D bash tests/theme_run.sh --installed 2>&1 | grep -E '^-- |FAIL|checks,|Traceback'"
  if [ "$m" = /srv/noble ]; then
    echo "== Wayland in $m"
    cp /home/claude/folio-suite/tests/wl.sh $m/tmp/wl.sh
    chroot $m bash -c "cd /tmp/fsi; export DISPLAY=:$D; /tmp/wl.sh bash tests/theme_run.sh --installed 2>&1 | grep -E '^-- |FAIL|checks,|Traceback'"
  fi
done
