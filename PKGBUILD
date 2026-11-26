# Maintainer: MehmetCanWT
pkgname=sonata
pkgver=1.0.10
pkgrel=1
pkgdesc="Rust terminal music player for YouTube Music and Spotify"
arch=('x86_64' 'aarch64')
url="https://github.com/MehmetCanWT/sonata"
license=('MIT')
depends=('mpv' 'yt-dlp' 'ffmpeg' 'nodejs')
makedepends=('cargo' 'pkgconf')
optdepends=(
    'ghostty: full-color and animated album artwork in the terminal'
    'kitty: full-color and animated album artwork in the terminal'
    'xdg-utils: open GitHub releases from Settings'
)
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')

prepare() {
    cd "$pkgname-$pkgver"
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "$pkgname-$pkgver"
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release -p sonata-tui
}

check() {
    cd "$pkgname-$pkgver"
    cargo test --frozen -p sonata-core -p sonata-tui
}

package() {
    cd "$pkgname-$pkgver"
    install -Dm0755 "target/release/sonata" "$pkgdir/usr/bin/sonata"
    install -Dm0644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm0644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
