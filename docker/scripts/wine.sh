#!/usr/bin/env bash
# Wine, to run Windows test binaries and programs (`wine.sh x86_64` or `wine.sh i686`), with a
# prefix initialised at /opt/wine-prefix that hyprcross-runner copies for each user.

set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

arch="$1"

if [ "$arch" = "i686" ]; then
    dpkg --add-architecture i386
    apt_install wine wine32:i386
else
    apt_install wine wine64
fi

# No Mono or Gecko prompts: tests don't need .NET or a browser engine.
WINEPREFIX=/opt/wine-prefix \
    WINEDLLOVERRIDES="mscoree,mshtml=" \
    WINEDEBUG=-all \
    wineboot --init
WINEPREFIX=/opt/wine-prefix wineserver --wait
chmod -R a+rX /opt/wine-prefix
