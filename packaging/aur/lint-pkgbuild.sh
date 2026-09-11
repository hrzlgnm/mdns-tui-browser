#!/usr/bin/env bash
# Copyright 2026 hrzlgnm
# SPDX-License-Identifier: MIT
#
# Generate the PKGBUILD into ~/lint, print it, for the shared
# aur-makepkg-lint action. Must run from the repository root
# (GENERATE_SCRIPT is repo-relative).
#
# Env:
#   GENERATE_SCRIPT  generator script, e.g. ./packaging/aur/generate-mdns-tui-browser.sh
#   VERSION          release version without leading v, e.g. 1.2.3
#   SHA256           tarball checksum file content
#
# Idempotent: the PKGBUILD is regenerated with overwrite on every run.

set -euo pipefail

: "${GENERATE_SCRIPT:?GENERATE_SCRIPT must be set}"
: "${VERSION:?VERSION must be set}"
: "${SHA256:?SHA256 must be set}"

mkdir -p "${HOME}/lint"
"$GENERATE_SCRIPT" "$VERSION" "$SHA256" >"${HOME}/lint/PKGBUILD"
cat "${HOME}/lint/PKGBUILD"
