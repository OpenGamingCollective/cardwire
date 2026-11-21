# Adapted from <https://tonyfinn.com/blog/arch-packages-with-nix/>
{
  pkgs,
  toolchain,
  sourceDate,
  ...
}:
let
  cargoToml = fromTOML (builtins.readFile ../Cargo.toml);
  version = cargoToml.workspace.package.version;
in
pkgs.stdenv.mkDerivation {
  pname = "cardwire";
  inherit version;
  src = ./..;

  cargoDeps = pkgs.symlinkJoin {
    name = "cargo-vendor-dir";
    paths = [
      (pkgs.rustPlatform.importCargoLock {
        lockFile = ../Cargo.lock;
      })
      # cardwire-ebpf builds the core with ebpf target
      (pkgs.rustPlatform.importCargoLock {
        lockFile = "${toolchain}/lib/rustlib/src/rust/library/Cargo.lock";
      })
    ];
  };

  __structuredAttrs = true;

  nativeBuildInputs = [
    pkgs.cargo-deb
    pkgs.bpf-linker
    pkgs.pkg-config
    pkgs.rustPlatform.cargoSetupHook
    pkgs.patchelf
    pkgs.cargo-zigbuild
    pkgs.zig
    toolchain
  ];

  buildInputs = [
    pkgs.udev
    pkgs.sqlite
  ];

  env = {
    RUSTC_BOOTSTRAP = 1;
    RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
    SOURCE_DATE_EPOCH = toString sourceDate;
    GLIBC_TARGET = "2.39";
    ZIG_LOCAL_CACHE_DIR = "./zig-cache";
    ZIG_GLOBAL_CACHE_DIR = "./zig-cache";
    CARGO_ZIGBUILD_CACHE_DIR = "./cargo-zig-cache";
    NIX_ARCH =
      if pkgs.stdenv.hostPlatform.isAarch64 then
        "aarch64-unknown-linux-gnu"
      else
        "x86_64-unknown-linux-gnu";
    NIX_FOREIGN_INTERP =
      if pkgs.stdenv.hostPlatform.isAarch64 then
        "/lib/ld-linux-aarch64.so.1"
      else
        "/lib64/ld-linux-x86-64.so.2";
  };

  buildPhase = ''
    mkdir -p zig-cache
    mkdir -p cargo-zig-cache

    cargo zigbuild --release --frozen --target "$NIX_ARCH.$GLIBC_TARGET" --bins

    patchelf --set-interpreter "$NIX_FOREIGN_INTERP" --remove-rpath ./target/$NIX_ARCH/release/cardwire

    # nixpkgs' libsqlite3.so has no SONAME, so the NEEDED entry is the dev-only name
    patchelf --set-interpreter "$NIX_FOREIGN_INTERP" --remove-rpath --replace-needed libsqlite3.so libsqlite3.so.0 ./target/$NIX_ARCH/release/cardwired

    patchelf --set-interpreter "$NIX_FOREIGN_INTERP" --remove-rpath ./target/$NIX_ARCH/release/cardwire-gui

    cargo deb -p cardwire-daemon --no-build --target "$NIX_ARCH" --no-strip
  '';

  installPhase = ''
    mkdir -p $out

    install -Dm644 -t $out target/$NIX_ARCH/debian/*.deb
  '';
}
