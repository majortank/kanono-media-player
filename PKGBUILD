# Maintainer: Kanono Contributors
pkgname=kanono-media-player
pkgver=0.1.0
pkgrel=1
pkgdesc='Modular, gapless audio player for Linux'
arch=('x86_64')
url='https://example.invalid/kanono-media-player'
license=('MIT')
depends=('alsa-lib' 'dbus' 'libxkbcommon' 'vulkan-icd-loader')
makedepends=('cargo' 'rust')
source=("${pkgname}-${pkgver}.tar.gz")
sha256sums=('SKIP')

prepare() {
  cd "${pkgname}-${pkgver}"
  export CARGO_HOME="${srcdir}/cargo-home"
  cargo fetch --locked
}

build() {
  cd "${pkgname}-${pkgver}"
  export CARGO_HOME="${srcdir}/cargo-home"
  cargo build --release --locked -p kanono-player
}

check() {
  cd "${pkgname}-${pkgver}"
  export CARGO_HOME="${srcdir}/cargo-home"
  cargo test --locked -p kanono-audio-engine
}

package() {
  cd "${pkgname}-${pkgver}"
  install -Dm755 target/release/kanono-player "${pkgdir}/usr/bin/kanono-player"
  install -Dm644 resources/kanono-media-player.desktop \
    "${pkgdir}/usr/share/applications/kanono-media-player.desktop"
  install -Dm644 assets/icons/hicolor/scalable/apps/kanono-media-player.svg \
    "${pkgdir}/usr/share/icons/hicolor/scalable/apps/kanono-media-player.svg"
}