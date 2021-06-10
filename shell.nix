let
  # moz_overlay = (import "/home/insipx/.config/nixpkgs/overlays/rust-overlay.nix");
  nixpkgs = import <nixpkgs> { overlays = [ ]; };
  unstable = import (fetchTarball "channel:nixos-unstable") {};
  # rustnightly = ((nixpkgs.rustChannelOf { date = "2021-03-26"; channel = "nightly"; }).rust.override { extensions = [ "rust-src" "rust-analysis" ]; });

in
  with nixpkgs;
  pkgs.mkShell {
    buildInputs = [
      openssl
      pkg-config
      nasm
      unstable.rustup
      unstable.cargo-expand
      cmake
      zlib
      postgresql_13
    ];
    shellHook = ''
      export DATABASE_URL="postgres://postgres:123@localhost/coil-testdb"
    '';
}

