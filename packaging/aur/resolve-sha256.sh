#!/usr/bin/env bash
# Copyright 2026 hrzlgnm
# SPDX-License-Identifier: MIT
#
# Resolve the release tarball checksum file and expose it as a step output.
#
# Env:
#   CHECKSUM_URL   URL of the "<version>.tar.gz.sha256" checksum file
#   GITHUB_OUTPUT  file receiving step outputs (injected by Actions)
#
# Idempotent: the output is recomputed from scratch on every run; a retry
# appends an identical line and Actions uses the last value for the key.

set -euo pipefail

: "${CHECKSUM_URL:?CHECKSUM_URL must be set}"
: "${GITHUB_OUTPUT:?GITHUB_OUTPUT must be set}"

url="$CHECKSUM_URL"
echo "getting $url"
sum=$(curl -LfsS --retry 5 --retry-delay 5 --retry-all-errors --connect-timeout 15 "$url" | cut -f1 -d ' ')
echo "sha256=$sum" >>"$GITHUB_OUTPUT"
