#!/usr/bin/env bash
# Copyright 2026 hrzlgnm
# SPDX-License-Identifier: MIT-0

version=$1
sha256sum=$2

if [[ -z "$version" || -z "$sha256sum" ]]; then
    echo "Usage: $0 <version> <sha256sum>" >&2
    exit 1
fi

cat <<EOF
# Maintainer: Valentin Batz <valentin.batz+archlinux@posteo.de>

pkgname=mdns-browser
pkgver=$version
pkgrel=1
pkgdesc="A terminal-based mDNS service browser"
arch=('x86_64')
url="https://github.com/hrzlgnm/mdns-tui-browser"
license=('MIT')
makedepends=('cargo' 'cargo-auditable' 'git' 'rust')
options=('!emptydirs')
source=("\$pkgname-v\$pkgver.tar.gz::https://github.com/hrzlgnm/\$pkgname/archive/refs/tags/v\$pkgver.tar.gz")
sha256sums=('$sha256sum')
_builddir="\$pkgname-\$pkgver"
prepare() {
    cd "\$srcdir/\$_builddir" || exit 1
    cargo fetch --locked --target "\$(rustc -vV | sed -n 's/host: //p')"
}
build() {
    cd "\$srcdir/\$_builddir" || exit 1
    cargo --locked --frozen build --release
}
check() {
    cd "\$srcdir/\$_builddir" || exit 1
    cargo test --locked --frozen
}
package() {
    install -Dm755 "\${srcdir}/\${_builddir}/target/release/mdns-tui-browser" "\$pkgdir"/usr/bin/mdns-tui-browser
    install -Dm644 "\${srcdir}/\${_builddir}/LICENSE "\$pkgdir"/usr/share/licenses/mdns-tui-browser/LICENSE
}
EOF
