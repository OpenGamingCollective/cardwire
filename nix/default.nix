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
      pkgs.makeWrapper
      pkgs.pkg-config
    ];
    buildInputs = [
      pkgs.hwdata
      pkgs.libbpf
      pkgs.udev
    ];
    runtimeDeps = [
      pkgs.hwdata
      pkgs.upower
      pkgs.udev
    ];
    doCheck = false;
    doInstallCheck = true;
    meta = {
      description = "a GPU manager for laptop and workstation";
      homepage = "https://github.com/OpenGamingCollective/cardwire";
      license = lib.licenses.gpl3;
    };
    # Point to the correct hwdata location
    postPatch = ''
      substituteInPlace crates/cardwire-daemon/src/core/pci/pci_device.rs \
      --replace "/usr/share/hwdata/pci.ids" "${pkgs.hwdata}/share/hwdata/pci.ids"
    '';
    # Copy dbus conf, systemd service and make shell completion
    postInstall = ''
      install -Dm444 ./assets/com.github.opengamingcollective.cardwire.conf \
         $out/share/dbus-1/system.d/com.github.opengamingcollective.cardwire.conf

      installShellCompletion --cmd cardwire \
         --fish <($out/bin/cardwire completion fish)



      wrapProgram $out/bin/cardwired \
      --prefix LD_LIBRARY_PATH : ${
        lib.makeLibraryPath [
          pkgs.udev
          pkgs.upower
        ]
      }
    '';
  }
