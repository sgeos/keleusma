# Nix flake for Keleusma development environments.
#
# Reproducible Rust toolchain plus the system dependencies needed
# to build the full workspace (including the SDL3-gated examples
# when desired). Usage:
#
#     nix develop                  # default shell: full workspace
#     nix develop .#minimal        # smaller shell: no SDL3
#
# The flake assumes the Nix `flakes` and `nix-command` experimental
# features are enabled. Add `experimental-features = nix-command flakes`
# to `~/.config/nix/nix.conf` if needed.

{
    description = "Keleusma development environments";

    inputs = {
        nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
        flake-utils.url = "github:numtide/flake-utils";
        rust-overlay = {
            url = "github:oxalica/rust-overlay";
            inputs.nixpkgs.follows = "nixpkgs";
        };
    };

    outputs = { self, nixpkgs, flake-utils, rust-overlay }:
        flake-utils.lib.eachDefaultSystem (system:
            let
                pkgs = import nixpkgs {
                    inherit system;
                    overlays = [ rust-overlay.overlays.default ];
                };
                # Rust toolchain matching the workspace's MSRV.
                # Updated when CLAUDE.md `rust-version` bumps.
                rustToolchain = pkgs.rust-bin.stable."1.88.0".default.override {
                    extensions = [ "rust-src" "clippy" "rustfmt" "rust-analyzer" ];
                    targets = [ "thumbv7em-none-eabihf" "thumbv8m.main-none-eabihf" ];
                };
                # Common build inputs required for the workspace's
                # default features.
                commonInputs = with pkgs; [
                    rustToolchain
                    pkg-config
                    cmake
                    ninja
                    cloc
                ];
                # SDL3 build dependencies for the
                # `sdl3-example` feature. The list mirrors the
                # SDL3 README's Linux build requirements; SDL3
                # builds from source via cmake in the `sdl3`
                # crate's build script.
                sdl3Inputs = with pkgs; [
                    alsa-lib
                    libpulseaudio
                    libjack2
                    sndio
                    xorg.libX11
                    xorg.libXext
                    xorg.libXrandr
                    xorg.libXcursor
                    xorg.libXfixes
                    xorg.libXi
                    xorg.libXScrnSaver
                    xorg.libXtst
                    libxkbcommon
                    libdrm
                    mesa
                    libGL
                    dbus
                    udev
                    wayland
                    pipewire
                    libdecor
                ];
            in {
                devShells.default = pkgs.mkShell {
                    buildInputs = commonInputs ++ sdl3Inputs;
                    shellHook = ''
                        echo "Keleusma dev shell (full, with SDL3 deps)"
                        echo "Toolchain: $(rustc --version)"
                    '';
                };
                devShells.minimal = pkgs.mkShell {
                    buildInputs = commonInputs;
                    shellHook = ''
                        echo "Keleusma dev shell (minimal, no SDL3)"
                        echo "Toolchain: $(rustc --version)"
                    '';
                };
            }
        );
}
