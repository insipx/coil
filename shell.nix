let
  moz_overlay = (import "/home/insipx/.config/nixpkgs/overlays/rust-overlay.nix");
  nixpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
  unstable = import (fetchTarball "channel:nixos-unstable") {};
  rustnightly = ((nixpkgs.rustChannelOf { date = "2020-08-17"; channel = "nightly"; }).rust.override { extensions = [ "rust-src" "rust-analysis" "rustfmt-preview" ]; targets = ["wasm32-unknown-unknown"]; });

in
  with nixpkgs;
  pkgs.mkShell {
    buildInputs = [
      openssl
      pkg-config
      nasm
      rustnightly
      unstable.cargo-expand
      cmake
      zlib
      postgresql
    ];
    shellHook = ''
              export PGDATA=$PWD/.tmp/postgres_data
              export PGHOST=$PWD/.tmp/postgres
              export LOG_PATH=$PWD/.tmp/postgres/LOG
              export PGDATABASE=postgres
              export DATABASE_URL="postgres://postgres?host=$PGHOST"
              if [ ! -d $PGHOST ]; then
                mkdir -p $PGHOST
              fi
              if [ ! -d $PGDATA ]; then
                echo 'initializing postgresql database...'
                initdb $PGDATA --auth=trust >/dev/null
              fi
              pg_ctl start -l $LOG_PATH -o "-c listen_addresses= -c unix_socket_directories=$PGHOST"
    '';
}

