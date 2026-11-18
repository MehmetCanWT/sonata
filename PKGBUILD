# Maintainer: MehmetCanWT
pkgname=sonata
pkgver=0.1.0
pkgrel=1
pkgdesc="Fast, modular terminal music player for YouTube Music and Spotify with HD album art, synced lyrics, Hi-Fi audio enhancer, and color-only themes"
arch=('x86_64' 'aarch64')
url="https://github.com/MehmetCanWT/sonata"
license=('MIT')
depends=('mpv' 'yt-dlp')
makedepends=('cargo')
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')

prepare() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release
}

check() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo test --frozen
}

package() {
    cd "$pkgname-$pkgver"
    install -Dm0755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
    install -Dm0644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
