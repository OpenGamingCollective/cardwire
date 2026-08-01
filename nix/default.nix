{
  lib,
  pkgs,
  toolchain ? null, # Kept so your flake.nix callPackage doesn't break, but ignored here
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

  nativeBuildInputs = [
    pkgs.clang
    pkgs.installShellFiles
    pkgs.makeWrapper
    pkgs.pkg-config
    pkgs.bpf-linker
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
    pkgs.wayland
    pkgs.libxkbcommon
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
      --replace "/usr/share/hwdata/pci.ids" "${pkgs.hwdata}/share/hwdata/pci.ids"

    substituteInPlace crates/cardwire-daemon/src/core/gpu/discover.rs \
      --replace "/usr/share/libdrm/amdgpu.ids" "${pkgs.libdrm}/share/libdrm/amdgpu.ids"
  '';

  env = {
    RUSTFLAGS = "-C target-feature=";
    RUSTC_BOOTSTRAP = 1;
  };

  postInstall = ''
    install -Dm444 ./assets/com.github.opengamingcollective.cardwire.conf \
       $out/share/dbus-1/system.d/com.github.opengamingcollective.cardwire.conf

    install -Dm444 ./assets/cardwire-gui.desktop \
       $out/share/applications/cardwire-gui.desktop

    install -Dm444 ./assets/com.github.opengamingcollective.cardwire.metainfo.xml \
       $out/share/metainfo/com.github.opengamingcollective.cardwire.metainfo.xml

    for icon in ./assets/icons/*.svg; do
      install -Dm444 "$icon" "$out/share/icons/hicolor/scalable/apps/$(basename "$icon")"
    done

    installShellCompletion --cmd cardwire \
       --fish <($out/bin/cardwire completion fish)

    wrapProgram $out/bin/cardwired \
    --prefix LD_LIBRARY_PATH : ${
      lib.makeLibraryPath [
        pkgs.udev
        pkgs.upower
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
  '';
}
