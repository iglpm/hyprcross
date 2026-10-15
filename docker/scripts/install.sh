#!/usr/bin/env bash
# Install distribution packages (`install.sh <package>...`) and drop apt's lists in the same layer.

set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

apt_install "$@"
