#!/bin/bash
# one audit from the copy in /srv/jammy/tmp/fs: one.sh name [display]
chroot /srv/jammy bash -c "cd /tmp/fs; FOLIO_TEST_SCRIPT=tests/$1.py DISPLAY=:${2:-98} PYTHONPATH=app timeout 900 python3 -m folio_sheets 2>&1 | grep -vE 'dbind|dconf|^$'"
