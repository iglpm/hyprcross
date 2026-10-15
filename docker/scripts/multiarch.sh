#!/usr/bin/env bash
# Let `apt-get install libfoo-dev:<arch>` fetch the target's libraries (`multiarch.sh arm64`),
# typically from a `pre_build` step in Hyprcross.toml. Only configures apt: installs nothing.

set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

arch="$1"
native="$(dpkg --print-architecture)"

dpkg --add-architecture "$arch"

# i386 lives in the main archive next to amd64; everything else is on ports.ubuntu.com.
if [ "$arch" = "i386" ] || [ "$native" != "amd64" ]; then
    exit 0
fi

sources=/etc/apt/sources.list.d/ubuntu.sources
# Pin the existing stanzas to the native architecture, so apt doesn't ask the main archive
# for packages it doesn't carry.
sed -i "/^Types: deb/a Architectures: ${native}" "$sources"

. /etc/os-release
cat > /etc/apt/sources.list.d/ubuntu-ports.sources <<EOF
Types: deb
URIs: http://ports.ubuntu.com/ubuntu-ports/
Suites: ${VERSION_CODENAME} ${VERSION_CODENAME}-updates ${VERSION_CODENAME}-security
Components: main universe
Architectures: ${arch}
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg
EOF
