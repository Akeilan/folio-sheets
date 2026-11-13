#!/bin/bash
# Folio Sheet release checks: the Rust engine's tests, every audit and the command walk from source
# on 22.04, the .deb, then the installed package on 22.04, 22.04 without the extras and 24.04, with
# a Wayland pass on 24.04.   XD=<X display> tools/release.sh
set -u
R=/home/claude/folio-sheets
XD=${XD:-98}
V=$(sed -n 's/^Version: //p' $R/packaging/DEBIAN/control)
/home/claude/mounts.sh >/dev/null 2>&1
pgrep -x Xvfb -a | grep -q ":$XD " || { rm -f /tmp/.X$XD-lock /tmp/.X11-unix/X$XD; setsid -f Xvfb :$XD -screen 0 1600x1000x24 -nolisten tcp </dev/null >/dev/null 2>&1; sleep 1; }
rm -rf /srv/jammy/tmp/fs && mkdir -p /srv/jammy/tmp/fs && rsync -a --exclude .git $R/ /srv/jammy/tmp/fs/
if [ -z "${INSTALLED_ONLY:-}" ]; then
  echo "== engine (Rust): built fresh, so the package cannot ship an older one"
  bash $R/engine/build.sh 2>&1 | grep -E "test result|^error|engine:" | tail -3
  bash $R/engine/sync_app.sh >/dev/null 2>&1
  echo "== engine (Rust)"
  chroot /srv/jammy bash -c "cd /tmp/fs/engine && cargo test -q --no-default-features --lib 2>&1 | grep -E 'test result|^error|FAILED'"
  echo "== audits and the walk (source)"
  chroot /srv/jammy bash -c "cd /tmp/fs && DISPLAY=:$XD bash tests/run_all.sh 2>&1 | grep -vE 'dbind|^$'"
fi
echo "== deb"
sh $R/build-deb.sh /home/claude/out && ls -la /home/claude/out/folio-sheet_${V}_amd64.deb
echo "== X4-05: against the app it replaces"
bash /home/claude/folio-suite/tools/compare-release.sh folio-sheet /home/claude/folio-sheets /home/claude/out/folio-sheet_${V}_amd64.deb || \
  echo "X4-05 FAILED: this candidate shows less than the app it replaces"
echo "== installed"
bash $R/tools/release_installed.sh $V $XD
exit 0
