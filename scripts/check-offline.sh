#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Verifies that the application makes no network connection while sync is
# disabled (roadmap C2), on macOS:
#
#     scripts/check-offline.sh target/release/bundle/macos/Scoplen.app
#
# The application runs under a sandbox profile that kills the process on any
# IP network operation: connecting out, accepting, or binding a port. It must
# start, open its local data, render its first screen, and stay idle for
# SPL_SMOKE_LINGER_SECS seconds without being killed. A control run of curl
# under the same profile must be killed, which proves the profile is in force.
set -euo pipefail

app="${1:?usage: check-offline.sh <path to Scoplen.app>}"
binary="$app/Contents/MacOS/scoplen"
profile='(version 1)(allow default)
(deny network-outbound (remote ip) (with send-signal SIGKILL))
(deny network-inbound (local ip) (with send-signal SIGKILL))
(deny network-bind (local ip) (with send-signal SIGKILL))'

status=0
sandbox-exec -p "$profile" /usr/bin/curl -s -m 5 http://192.0.2.1/ >/dev/null 2>&1 || status=$?
if [ "$status" -ne 137 ]; then
  echo "control failed: curl was not killed by the profile (exit $status)" >&2
  exit 1
fi

data="$(mktemp -d)"
cleanup() {
  security delete-generic-password -s com.scoplen.client -a "local-database-key:$data" >/dev/null 2>&1 || true
  rm -rf "$data"
}
trap cleanup EXIT

status=0
SPL_DATA_DIR="$data" SPL_SMOKE_TEST=1 SPL_SMOKE_LINGER_SECS="${SPL_SMOKE_LINGER_SECS:-15}" \
  sandbox-exec -p "$profile" "$binary" || status=$?
if [ "$status" -ne 0 ]; then
  echo "the application exited with $status under the no-network profile; 137 means it touched the network" >&2
  exit 1
fi
test -s "$data/local.db" || { echo "the local store was not created" >&2; exit 1; }
echo "offline check passed: no network operation, local store created"
