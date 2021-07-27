let
  nixpkgs = import <nixpkgs> { overlays = [ (import <rust-overlay>) ]; };
  unstable = import (fetchTarball "channel:nixos-unstable") {};
in
  with nixpkgs;
  pkgs.mkShell {
    buildInputs = [
      openssl
      pkg-config
      nasm
      (nixpkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
	extensions = [ "rust-src" "rust-std" "clippy-preview" ];
	targets = [ "wasm32-unknown-unknown"];
      }))
      cmake
      zlib
      postgresql_13
    ];
    shellHook = ''
      export DATABASE_URL="postgres://postgres:123@localhost/coil-testdb"
    '';
}

