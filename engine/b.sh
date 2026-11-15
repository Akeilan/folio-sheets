#!/bin/bash
# sync and build the engine in the jammy chroot; args: cargo subcommand args
# the agent proxy's port changes between sessions: hand the current one to the build root
if [ -n "${HTTPS_PROXY:-${https_proxy:-}}" ]; then
  p="${HTTPS_PROXY:-$https_proxy}"
  printf "export https_proxy='%s'\nexport HTTPS_PROXY='%s'\nexport no_proxy='%s'\n" "$p" "$p" "${no_proxy:-${NO_PROXY:-localhost,127.0.0.1}}" > /srv/jammy/root/proxyenv.sh
fi
mkdir -p /srv/jammy/src/fse
rm -rf /srv/jammy/src/fse/src; cp -r /home/claude/folio-sheets/engine/src /srv/jammy/src/fse/
rm -rf /srv/jammy/src/fse/testdata; cp -r /home/claude/folio-sheets/engine/testdata /srv/jammy/src/fse/; rm -rf /srv/jammy/src/fse/data; cp -r /home/claude/folio-sheets/engine/data /srv/jammy/src/fse/; cp /home/claude/folio-sheets/engine/Cargo.toml /home/claude/folio-sheets/engine/build.rs /srv/jammy/src/fse/
cd /
chroot /srv/jammy bash -c ". /root/cargoenv.sh; cd /src/fse && cargo $* 2>&1"
