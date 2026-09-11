#!/usr/bin/env bash
# Copyright 2026 hrzlgnm
# SPDX-License-Identifier: MIT
#
# Regenerate ~/aur/PKGBUILD, verify it builds, and push the update.
# Must run from the repository root (GENERATE_SCRIPT is repo-relative).
#
# Env:
#   GENERATE_SCRIPT  generator script, e.g. ./packaging/aur/generate-mdns-tui-browser.sh
#   VERSION          release version without leading v, e.g. 1.2.3
#   SHA256           tarball checksum file content
#
# Idempotent: regeneration overwrites and the commit happens only on actual
# changes, while the push always runs, so a re-run after a committed-but-
# unpushed attempt just pushes instead of exiting early without pushing.

set -euo pipefail

: "${GENERATE_SCRIPT:?GENERATE_SCRIPT must be set}"
: "${VERSION:?VERSION must be set}"
: "${SHA256:?SHA256 must be set}"

"$GENERATE_SCRIPT" "$VERSION" "$SHA256" >"${HOME}/aur/PKGBUILD"
cd "${HOME}/aur" || exit 1
if [[ -n "$(git status --porcelain)" ]]; then
    makepkg --printsrcinfo >.SRCINFO
    makepkg
    makepkg --install --noconfirm
    git config user.name "hrzlgnm"
    git config user.email "hrzlgnm@users.noreply.github.com"
    git add PKGBUILD .SRCINFO
    git commit -m "New upstream release $VERSION"
else
    echo "No changes (already committed on previous retry or up to date)"
fi
# Always push: on re-run after commit-but-push-failed the worktree is clean,
# so exiting early would skip the push. A failed push stays a failed attempt.
git push origin master
