{
  lib,
  pkgs,
  toolchain,
}:
let
  cargoToml = fromTOML (builtins.readFile ../Cargo.toml);
  version = cargoToml.workspace.package.version;
  rustPlatform = pkgs.makeRustPlatform {
    cargo = toolchain;
    rustc = toolchain;
  };
in
rustPlatform.buildRustPackage {
  inherit version;
  pname = "cardwire";
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
    pkgs.pkg-config
    pkgs.bpf-linker
    pkgs.makeBinaryWrapper
    pkgs.installShellFiles
  ];

  buildInputs = [
    pkgs.udev
  ];

  doCheck = false;
  doInstallCheck = true;

  meta = {
    description = "a GPU manager for laptop and workstation";
    homepage = "https://github.com/OpenGamingCollective/cardwire";
    license = lib.licenses.gpl3;
  };

  postPatch = ''
    # Point to the correct hwdata location
    substituteInPlace crates/cardwire-core/src/pci/pci_device.rs \
      --replace-fail "/usr/share/hwdata/pci.ids" "${pkgs.hwdata}/share/hwdata/pci.ids"
  '';

  env = {
    RUSTC_BOOTSTRAP = 1;
    RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
  };

  postInstall = ''
    install -Dm444 ./assets/org.opengamingcollective.cardwire.conf \
      $out/share/dbus-1/system.d/org.opengamingcollective.cardwire.conf

    install -Dm444 ./assets/org.opengamingcollective.cardwire.metainfo.xml \
      $out/share/metainfo/org.opengamingcollective.cardwire.metainfo.xml

    install -Dm444 ./assets/cardwire-gui.desktop \
      $out/share/applications/cardwire-gui.desktop

    for icon in ./assets/icons/*.svg; do
      install -Dm444 "$icon" "$out/share/icons/hicolor/scalable/apps/$(basename "$icon")"
    done

    wrapProgram $out/bin/cardwired \
      --prefix LD_LIBRARY_PATH : ${
        lib.makeLibraryPath [
          pkgs.vulkan-loader
          pkgs.libglvnd
          pkgs.libdrm
        ]
      } \
      --prefix LD_LIBRARY_PATH : ${pkgs.addDriverRunpath.driverLink}/lib

    wrapProgram $out/bin/cardwire-gui \
      --prefix LD_LIBRARY_PATH : ${
        lib.makeLibraryPath [
          pkgs.wayland
          pkgs.libxkbcommon
          pkgs.vulkan-loader
          pkgs.libGL
        ]
      }

  ''
  + lib.optionalString (pkgs.stdenv.buildPlatform.canExecute pkgs.stdenv.hostPlatform) ''
    installShellCompletion --cmd cardwire \
      --fish <($out/bin/cardwire completion fish) \
      --bash <($out/bin/cardwire completion bash) \
      --zsh <($out/bin/cardwire completion zsh)
  '';
}
