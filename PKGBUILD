# Maintainer: MehmetCanWT
pkgname=sonata
pkgver=0.1.0
pkgrel=3
pkgdesc="Rust terminal music player for YouTube Music and Spotify"
arch=('x86_64' 'aarch64')
url="https://github.com/MehmetCanWT/sonata"
license=('MIT')
depends=('mpv' 'yt-dlp')
makedepends=('cargo')
optdepends=(
    'nodejs: enhanced YouTube extractor support for yt-dlp'
    'kitty: full-color album artwork in the terminal'
)
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
    cargo build --frozen --release -p sonata-tui
}

check() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo test --frozen -p sonata-core -p sonata-tui
}

package() {
    cd "$pkgname-$pkgver"
    install -Dm0755 "target/release/sonata" "$pkgdir/usr/bin/sonata"
    install -Dm0644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
