#!/bin/bash
# copy the app + tests + fresh engine into the chroot at /tmp/fs
rm -rf /srv/jammy/tmp/fs; mkdir -p /srv/jammy/tmp/fs
cp -r /home/claude/folio-sheets/app /home/claude/folio-sheets/tests /home/claude/folio-sheets/packaging /srv/jammy/tmp/fs/
cp /srv/jammy/src/fse/target/release/libfolio_sheets_core.so /srv/jammy/tmp/fs/app/folio_sheets_core.so
