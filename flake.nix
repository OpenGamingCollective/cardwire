{
  description = "Cardwire, a GPU manager for laptop and workstation";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix/monthly";
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
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = fn: nixpkgs.lib.genAttrs supportedSystems (system: fn system);
      pkgs = system: nixpkgs.legacyPackages.${system};
      fenixpkgs = system: fenix.packages.${system};
      toolchainFor =
        system:
        let
          tc = (fenixpkgs system).toolchainOf {
            channel = "nightly";
            date = "2026-08-12";
            sha256 = "sha256-LQDrWx1txtq4YH8MaJENr7uH1a8W6TwCN464Xjda3Ss=";
          };
        in
        (fenixpkgs system).combine [
          tc.cargo
          tc.rustc
          tc.rustfmt
          tc.clippy
          tc.rust-src
          tc.llvm-tools-preview
        ];
    in
    {
      packages = forAllSystems (system: {
        default = (pkgs system).callPackage ./nix { toolchain = toolchainFor system; };
        vm-test = self.checks.${system}.vm-ci-2gpu;
        vm-test-2gpu = self.checks.${system}.vm-ci-2gpu;
        vm-test-3gpu = self.checks.${system}.vm-ci-3gpu;
        vm-test-15gpu = self.checks.${system}.vm-ci-15gpu;
      });
      formatter = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          config = self.checks.${system}.pre-commit-check.config;
          inherit (config) package configFile;
          script = ''
            ${pkgs.lib.getExe package} run --all-files --config ${configFile}
          '';
        in
        pkgs.writeShellScriptBin "pre-commit-run" script
      );
      devShells = forAllSystems (system: {
        default = (pkgs system).mkShell {
          packages = [
            (toolchainFor system)
            (pkgs system).clang
            (pkgs system).libbpf
            (pkgs system).bpftools
            (pkgs system).udev
            (pkgs system).pkg-config
            (pkgs system).mdbook
            (pkgs system).mdbook-mermaid
            (pkgs system).wayland
            (pkgs system).libxkbcommon
            (pkgs system).vulkan-headers
            (pkgs system).libxcb
            (pkgs system).egl-wayland
            (pkgs system).egl-x11
            (pkgs system).libglvnd
          ]
          ++ self.checks.${system}.pre-commit-check.enabledPackages;
          LD_LIBRARY_PATH = (pkgs system).lib.makeLibraryPath [
            (pkgs system).wayland
            (pkgs system).libxkbcommon
            (pkgs system).vulkan-loader
            (pkgs system).libGL
            (pkgs system).udev
            (pkgs system).vulkan-headers
            (pkgs system).libxcb
            (pkgs system).egl-wayland
            (pkgs system).egl-x11
            (pkgs system).libglvnd
          ];
          LIBCLANG_PATH = "${(pkgs system).llvmPackages.libclang.lib}/lib";
          RUST_SRC_PATH = "${toolchainFor system}/lib/rustlib/src/rust/library";
          RUST_BACKTRACE = "1";
          shellHook = ''
            export PATH="$HOME/.cargo/bin:$PATH"
            ${self.checks.${system}.pre-commit-check.shellHook}
          '';
        };
      });
      nixosModules.default = import ./nix/nixos-module.nix self;
      nixosConfigurations = nixpkgs.lib.genAttrs supportedSystems (
        system:
        import ./nix/test-vm.nix {
          inherit nixpkgs self system;
        }
      );
      checks = forAllSystems (system: {
        vm-ci-2gpu = import ./nix/ci-2gpu.nix {
          inherit pkgs system self;
          lib = nixpkgs.lib;
        };
        vm-ci-3gpu = import ./nix/ci-3gpu.nix {
          inherit pkgs system self;
          lib = nixpkgs.lib;
        };
        vm-ci-15gpu = import ./nix/ci-15gpu.nix {
          inherit pkgs system self;
          lib = nixpkgs.lib;
        };
        pre-commit-check = git-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            nixfmt.enable = true;
            rustfmt = {
              enable = true;
              package = toolchainFor system;
            };
          };
        };
      });
    };
}
