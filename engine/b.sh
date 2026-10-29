#!/bin/bash
# sync and build the engine in the jammy chroot; args: cargo subcommand args
mkdir -p /srv/jammy/src/fse
rm -rf /srv/jammy/src/fse/src; cp -r /home/claude/folio-sheets/engine/src /srv/jammy/src/fse/
rm -rf /srv/jammy/src/fse/testdata; cp -r /home/claude/folio-sheets/engine/testdata /srv/jammy/src/fse/; rm -rf /srv/jammy/src/fse/data; cp -r /home/claude/folio-sheets/engine/data /srv/jammy/src/fse/; cp /home/claude/folio-sheets/engine/Cargo.toml /home/claude/folio-sheets/engine/build.rs /srv/jammy/src/fse/
cd /
chroot /srv/jammy bash -c ". /root/cargoenv.sh; cd /src/fse && cargo $* 2>&1"
