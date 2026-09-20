{
  lib,
  pkgs,
  toolchain ? null,
}:
let
  cargoToml = builtins.fromTOML (builtins.readFile ../Cargo.toml);
  version = cargoToml.workspace.package.version;
in
pkgs.rustPlatform.buildRustPackage {
  inherit version;
  pname = "cardwire";
  src = ./..;
  cargoLock.lockFile = ../Cargo.lock;

  __structuredAttrs = true;

  nativeBuildInputs = [
    pkgs.pkg-config
    pkgs.bpf-linker
    pkgs.makeBinaryWrapper
    pkgs.installShellFiles
  ];

  buildInputs = [
    pkgs.udev
    pkgs.libxcb
  ];

  doCheck = false;
  doInstallCheck = true;

  meta = {
    description = "a GPU manager for laptop and workstation";
    homepage = "https://github.com/OpenGamingCollective/cardwire";
    license = lib.licenses.gpl3;
  };

  postPatch = ''

    # Fix from nixpkgs <https://github.com/NixOS/nixpkgs/blob/nixos-unstable/pkgs/development/python-modules/mitmproxy-linux/default.nix>

    sed -i 's/"+nightly"/"-v"/g' ../cargo-vendor-dir/aya-build-*/src/lib.rs
    sed -i 's/"-Z"/"-v"/g' ../cargo-vendor-dir/aya-build-*/src/lib.rs
    sed -i 's/"build-std=core"/"-v"/g' ../cargo-vendor-dir/aya-build-*/src/lib.rs

    find . -name config.toml -path "*/.cargo/config.toml" -exec sed -i 's/build-std = \["core"\]//g' {} +

    # Point to the correct hwdata location
    substituteInPlace crates/cardwire-daemon/src/core/pci/pci_device.rs \
      --replace-fail "/usr/share/hwdata/pci.ids" "${pkgs.hwdata}/share/hwdata/pci.ids"
  '';

  env = {
    RUSTFLAGS = "-C target-feature=";
    RUSTC_BOOTSTRAP = 1;
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
      }

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
