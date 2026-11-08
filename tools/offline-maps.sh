#!/bin/bash
# S3-15: the map chart tests, run with no network at all. `unshare -n` gives the
# process an empty network namespace — no interfaces but loopback — so anything
# that tried to fetch a map would fail rather than quietly succeed.
set -e
cd /home/claude/folio-sheets
mkdir -p /srv/jammy/src/fse
rm -rf /srv/jammy/src/fse/src; cp -r engine/src /srv/jammy/src/fse/
rm -rf /srv/jammy/src/fse/testdata; cp -r engine/testdata /srv/jammy/src/fse/
rm -rf /srv/jammy/src/fse/data; cp -r engine/data /srv/jammy/src/fse/
cp engine/Cargo.toml engine/build.rs /srv/jammy/src/fse/
unshare -n chroot /srv/jammy bash -c '
  . /root/cargoenv.sh
  ip link show 2>/dev/null | grep -c "^[0-9]" | sed "s/^/network interfaces visible: /"
  cd /src/fse && cargo test --no-default-features --lib 2>&1
' | grep -E "interfaces visible|test result|FAILED|panicked"
