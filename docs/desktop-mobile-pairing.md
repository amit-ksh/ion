# Pair Ion desktop with the Android companion

Desktop Ion **0.1.25** adds the native pairing code under **Settings → Daemon → Connection
details → Show pairing QR**. Enable the managed daemon listener and apply any
pending settings first. On Android Ion 1.0.2 or later, open **Pair desktop →
Scan to pair**, allow camera access, scan the code, review the address and tap
**Add**. Both devices need a route to the same private network or tailnet.

The code uses the versioned JSON contract already supported by the mobile
scanner. It includes the desktop name, current listener address and existing
full-control daemon token. It does not create a restricted gateway invite or
grant a new token. Regenerating the token invalidates old codes.

The QR remains hidden until requested. Hide it with the same button or Escape
while that button is focused. Navigating to another settings page or applying
listener/token changes clears it. A generation counter prevents an older
background request from restoring a cleared code. QR data is not written to
disk, sent to an external encoder or logged by this feature.

Address selection, QR encoding and rasterization run on the background
executor. Frames read only the cached image and address. Address selection
prefers the private IPv4 address chosen by the operating system's route and
falls back to the existing hostname. On machines with multiple networks, the
chosen address must also be reachable by the phone; the mobile pairing form
still allows reviewing and editing the scanned details.

The older source-checkout helper, `bun scripts/mobile-pairing.ts`, remains
available for desktop versions without the native button.
