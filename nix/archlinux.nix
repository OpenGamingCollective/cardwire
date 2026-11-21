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
    pkgs.zstd
    pkgs.pacman
    pkgs.fakeroot
    pkgs.libarchive
    pkgs.bpf-linker
    pkgs.pkg-config
    pkgs.rustPlatform.cargoSetupHook
    pkgs.patchelf
    toolchain
  ];

  buildInputs = [
    pkgs.udev
    pkgs.libxcb
    pkgs.sqlite
  ];

  env = {
    MAKEPKG_CONF = "${pkgs.pacman}/etc/makepkg.conf";
    RUSTC_BOOTSTRAP = 1;
    RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
    SOURCE_DATE_EPOCH = toString sourceDate;
    PACKAGER = "luytan <luytan@khora.me>";
    NIX_FOREIGN_INTERP =
      if pkgs.stdenv.hostPlatform.isAarch64 then
        "/lib/ld-linux-aarch64.so.1"
      else
        "/lib64/ld-linux-x86-64.so.2";
  };

  buildPhase = ''


    cp $src/packages/arch-linux/cardwire-nix-PKGBUILD ./PKGBUILD

    mkdir -p src

    cp -r --no-preserve=mode $src src/cardwire-${version}

    sed -i -E 's|^source=.*|source=("cardwire-'"${version}"'.tar.gz")|' PKGBUILD

    mkdir -p result

    # Fix bsdtar permissions
    mkdir -p bsdtar-wrapper
    cat > bsdtar-wrapper/bsdtar <<EOF
    #!${pkgs.runtimeShell}
    exec ${pkgs.libarchive}/bin/bsdtar --no-acls "\$@"
    EOF
    chmod +x bsdtar-wrapper/bsdtar
    export PATH="$PWD/bsdtar-wrapper:$PATH"

    PKGDEST=result PKGEXT=".pkg.tar.zst" makepkg --noextract --nodeps --noconfirm
  '';

  installPhase = ''
    mkdir -p $out

    install -Dm644 -t $out result/*.pkg.tar.zst
  '';

}
