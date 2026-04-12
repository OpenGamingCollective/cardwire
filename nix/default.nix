{
  lib,
  pkgs,
  toolchain,
}:
let
  cargoToml = fromTOML (builtins.readFile ../Cargo.toml);
  version = cargoToml.workspace.package.version;
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
      pkgs.installShellFiles
    ];
    buildInputs = [
      pkgs.hwdata
      pkgs.libbpf
    ];
    runtimeDeps = [
      pkgs.hwdata
    ];
    doCheck = false;
    doInstallCheck = true;
    meta = {
      description = "a GPU manager for laptop and workstation";
      homepage = "https://github.com/luytan/cardwire";
      license = lib.licenses.gpl3;
    };
    # Point to the correct hwdata location
    postPatch = ''
      substituteInPlace crates/cardwire-core/src/pci/pci_device.rs \
      --replace "/usr/share/hwdata/pci.ids" "${pkgs.hwdata}/share/hwdata/pci.ids"
    '';
    # Copy dbus conf, systemd service and make shell completion
    postInstall = ''
         install -Dm444 ./assets/com.github.luytan.cardwire.conf \
         $out/share/dbus-1/system.d/com.github.luytan.cardwire.conf
      installShellCompletion --cmd cardwire \
         --fish <($out/bin/cardwire completion fish)
    '';
  }
