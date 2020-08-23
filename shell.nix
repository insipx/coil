with import <nixpkgs> { };
let
  moz_overlay = (import "/home/insipx/.config/nixpkgs/overlays/rust-overlay.nix");
  nixpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
  ruststable = (nixpkgs.latest.rustChannels.stable.rust.override { extensions = [ "rust-src" "rust-analysis" "rustfmt-preview" ];});
in
  with nixpkgs;
  pkgs.mkShell {
    buildInputs = [
      openssl
      pkg-config
      nasm
      rustup
      ruststable
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

