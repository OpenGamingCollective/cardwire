{
  description = "Cardwire, a GPU manager for laptop and workstation";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
    git-hooks.url = "github:cachix/git-hooks.nix";
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      git-hooks,
    }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      eachSystem =
        f:
        lib.genAttrs systems (
          system:
          f {
            inherit system;
            pkgs = nixpkgs.legacyPackages.${system};
            toolchain = fenix.packages.${system}.fromToolchainFile {
              file = ./rust-toolchain.toml;
              sha256 = "sha256-MFHoWVcPZKjFrLnQnodCXbUsfZcofGfo9tV9fTTvklg=";
            };
          }
        );
    in
    {
      packages = eachSystem (
        {
          pkgs,
          system,
          toolchain,
          ...
        }:
        {
          default = pkgs.callPackage ./nix { };
          archlinux = pkgs.callPackage ./nix/archlinux.nix {
            inherit toolchain;
            sourceDate = self.lastModified;
          };
          vm-test = self.checks.${system}.vm-ci-2gpu;
          vm-test-2gpu = self.checks.${system}.vm-ci-2gpu;
          vm-test-3gpu = self.checks.${system}.vm-ci-3gpu;
          vm-test-15gpu = self.checks.${system}.vm-ci-15gpu;
        }
      );

      formatter = eachSystem (
        { pkgs, system, ... }:
        let
          inherit (self.checks.${system}.pre-commit-check.config) package configFile;
        in
        pkgs.writeShellScriptBin "pre-commit-run" ''
          ${lib.getExe package} run --all-files --config ${configFile}
        ''
      );

      devShells = eachSystem (
        {
          pkgs,
          system,
          toolchain,
          ...
        }:
        let
          preCommit = self.checks.${system}.pre-commit-check;
          runtimeLibs = with pkgs; [
            wayland
            libxkbcommon
            vulkan-loader
            libGL
            udev
            libxcb
            egl-wayland
            egl-x11
            libglvnd
            libdrm
          ];
          bpf-linker = pkgs.bpf-linker.override {
            llvmPackagesForLinker = pkgs.llvmPackages_23;
          };
        in
        {
          default = pkgs.mkShell {
            packages = [
              toolchain
              bpf-linker
            ]
            ++ (with pkgs; [
              bpftools
              pkg-config
              mdbook
              mdbook-mermaid
              vulkan-headers
              nushell
              just
              pacman
            ])
            ++ runtimeLibs
            ++ preCommit.enabledPackages;

            LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
            RUSTC_BOOTSTRAP = "1";

            shellHook = ''
              ${preCommit.shellHook}
            '';
          };
        }
      );

      nixosModules.default = import ./nix/nixos-module.nix self;

      nixosConfigurations = lib.genAttrs systems (
        system: import ./nix/test-vm.nix { inherit nixpkgs self system; }
      );

      checks = eachSystem (
        {
          pkgs,
          system,
          toolchain,
          ...
        }:
        let
          vmTest =
            file:
            import file {
              inherit pkgs self;
            };
        in
        {
          vm-ci-2gpu = vmTest ./nix/ci-2gpu.nix;
          vm-ci-3gpu = vmTest ./nix/ci-3gpu.nix;
          vm-ci-15gpu = vmTest ./nix/ci-15gpu.nix;
          pre-commit-check = git-hooks.lib.${system}.run {
            src = ./.;
            hooks = {
              nixfmt.enable = true;
              rustfmt = {
                enable = true;
                package = toolchain;
              };
            };
          };
        }
      );
    };
}
