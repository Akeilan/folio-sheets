#!/bin/bash
# runui.sh <probe.py> [display]  -- run a UI probe from /srv/jammy/tmp/b6 inside the app (source tree at /tmp/fs)
D=${2:-96}
chroot /srv/jammy bash -c "DISPLAY=:$D xdpyinfo" >/dev/null 2>&1 || { pkill -f "^Xvfb :$D " 2>/dev/null; rm -f /tmp/.X$D-lock /tmp/.X11-unix/X$D; setsid -f Xvfb :$D -screen 0 1600x1000x24 -nolisten tcp </dev/null >/dev/null 2>&1; sleep 2; }
cp /home/claude/b6/*.py /srv/jammy/tmp/b6/
chroot /srv/jammy bash -c "cd /tmp/fs; FOLIO_CONFIG_DIR=/tmp/b6/cfg-$$ FOLIO_TEST_SCRIPT=/tmp/b6/$1 DISPLAY=:$D PYTHONPATH=app timeout ${T:-600} python3 -m folio_sheets ${ARGS:-} 2>&1" | grep -vE 'dbind|dconf|^$|already in the tag table'
