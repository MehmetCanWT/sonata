{
  description = "Sonata: High-fidelity native Wayland desktop & terminal music player for YouTube Music, Spotify & JioSaavn";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        runtimeTools = with pkgs; [ mpv ffmpeg yt-dlp nodejs ];
        desktopLibs = with pkgs; [
          wayland
          vulkan-loader
          libxkbcommon
          xorg.libxcb
          fontconfig
          alsa-lib
          dbus
          openssl
          sqlite
        ];
        commonMeta = {
          description = "High-fidelity native music player for YouTube Music, Spotify and JioSaavn";
          homepage = "https://github.com/MehmetCanWT/sonata";
          license = pkgs.lib.licenses.mit;
          platforms = pkgs.lib.platforms.linux;
        };
      in
      {
        packages = {
          sonata = pkgs.rustPlatform.buildRustPackage {
            pname = "sonata";
            version = "1.0.4";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "-p" "sonata-tui" ];
            cargoTestFlags = [ "-p" "sonata-core" "-p" "sonata-tui" ];
            nativeBuildInputs = with pkgs; [ pkg-config makeWrapper ];
            buildInputs = with pkgs; [ openssl sqlite ];
            postInstall = ''
              wrapProgram $out/bin/sonata \
                --prefix PATH : ${pkgs.lib.makeBinPath runtimeTools}
            '';
            meta = commonMeta // { mainProgram = "sonata"; };
          };

          sonata-desktop = pkgs.rustPlatform.buildRustPackage {
            pname = "sonata-desktop";
            version = "1.0.4";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "-p" "sonata-desktop" ];
            doCheck = false;
            nativeBuildInputs = with pkgs; [ pkg-config makeWrapper ];
            buildInputs = desktopLibs;
            postInstall = ''
              install -Dm644 assets/sonata-desktop.desktop $out/share/applications/sonata-desktop.desktop
              install -Dm644 assets/sonata-desktop.svg $out/share/icons/hicolor/scalable/apps/sonata-desktop.svg
              wrapProgram $out/bin/sonata-desktop \
                --prefix PATH : ${pkgs.lib.makeBinPath runtimeTools} \
                --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath desktopLibs}
            '';
            meta = commonMeta // { mainProgram = "sonata-desktop"; };
          };

          default = self.packages.${system}.sonata;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [ cargo rustc rustfmt clippy pkg-config ];
          buildInputs = desktopLibs ++ runtimeTools;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath desktopLibs;
        };
      });
}
