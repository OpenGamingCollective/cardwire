{
  lib,
  stdenv,
  pkgs,
  toolchain,
}:
let
  version = "0.1.0";
  runtimeDeps = [
    pkgs.hwdata
  ];
in
(pkgs.makeRustPlatform {
  cargo = toolchain;
  rustc = toolchain;
}).buildRustPackage
  {
    inherit version;
    pname = "cardwire";
    src = ./..;
    cargoLock.lockFile = ../Cargo.lock;
    nativeBuildInputs = [
      pkgs.clang
      toolchain
    ];
    buildInputs = [
      pkgs.hwdata
      pkgs.libbpf
    ];

    meta = {
      description = "a GPU manager for laptop and workstation";
      homepage = "https://github.com/luytan/cardwire";
      license = lib.licenses.gpl3;
    };
    # Point to the correct hwdata location
    postPatch = ''
      substituteInPlace crates/cardwire-core/src/iommu/pci.rs \
      --replace "/usr/share/hwdata/pci.ids" "${pkgs.hwdata}/share/hwdata/pci.ids"
    '';
    # Copy dbus conf
    postInstall = ''
      install -Dm444 ./assets/com.cardwire.daemon.conf \
      $out/share/dbus-1/system.d/com.cardwire.daemon.conf
    '';
  }
