#!/bin/sh
# Runs once, as root and offline, inside a non-disposable Token Factory sandbox
# started from the pinned Python base. Its filesystem becomes the user's private
# Ion sandbox image. Inputs arrive as mounted files under /ion-setup; nothing
# here receives or stores a credential.
set -eu
s=/ion-setup
test "$(id -u)" = 0

# Refuse a conflicting identity rather than inheriting another user's home.
if getent passwd 1000 >/dev/null || getent group 1000 >/dev/null; then exit 3; fi
groupadd --gid 1000 ion
useradd --uid 1000 --gid 1000 --home-dir /home/ion --create-home --shell /usr/sbin/nologin ion
install -d -m 0755 -o root -g root /ion /usr/local/libexec
install -d -m 0700 -o ion -g ion /home/ion/.config /home/ion/.config/ion /ion/work

# Model worker: one bounded completion per disposable run, as UID 1000.
install -m 0555 -o root -g root "$s/ion-harness-worker" /usr/local/bin/ion-harness-worker
install -m 0444 -o root -g root "$s/host.json" /ion/host.json

# Command worker: root supervisor, nonroot child, seccomp socket denial.
install -m 0555 -o root -g root "$s/ion_job_worker.py" /usr/local/bin/ion-job-worker
install -m 0555 -o root -g root "$s/ion_job_guard.py" /usr/local/libexec/ion-job-guard.py
install -m 0555 -o root -g root "$s/ion_job_runtime.py" /usr/local/libexec/ion_job_runtime.py
install -m 0555 -o root -g root "$s/ion_job_landlock.py" /usr/local/libexec/ion_job_landlock.py
install -m 0444 -o root -g root "$s/job-host.json" /ion/job-host.json
# The guard accepts only kernels named here. This run executes on the same
# sandbox fleet as later commands, so it records the kernel actually in use.
python3 -I -S -c 'import json,platform; open("/usr/local/libexec/ion-job-kernels.json","w").write(json.dumps({"version":1,"releases":[platform.release()]},separators=(",",":")))'
chmod 0444 /usr/local/libexec/ion-job-kernels.json
test -e /usr/lib/x86_64-linux-gnu/libseccomp.so.2

# Remove native privilege elevation from the image.
find / -xdev -type f -perm /6000 -exec chmod a-s {} +

# ABI check without a provider login or request: no arguments are accepted.
status=0
/usr/local/bin/ion-harness-worker --invalid-contract >/tmp/ion-worker.out 2>/tmp/ion-worker.err || status=$?
test "$status" = 2 && test ! -s /tmp/ion-worker.out
rm -f /tmp/ion-worker.out /tmp/ion-worker.err
/usr/local/bin/ion-job-worker --version | grep -qx 'ion-job-worker/1'

rm -rf "$s"
echo ion-sandbox-ready
