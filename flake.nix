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
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = fn: nixpkgs.lib.genAttrs supportedSystems (system: fn system);
      pkgs = system: nixpkgs.legacyPackages.${system};
      fenixpkgs = system: fenix.packages.${system};
      toolchainFor =
        system:
        (fenixpkgs system).combine [
          (fenixpkgs system).stable.cargo
          (fenixpkgs system).stable.rustc
          (fenixpkgs system).stable.rustfmt
          (fenixpkgs system).stable.clippy
          (fenixpkgs system).stable.rust-src
        ];
    in
    {
      packages = forAllSystems (system: {
        default = (pkgs system).callPackage ./nix { toolchain = toolchainFor system; };
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
          ]
          ++ self.checks.${system}.pre-commit-check.enabledPackages;
          RUST_SRC_PATH = "${(fenixpkgs system).stable.rust-src}/lib/rustlib/src/rust/library";
          RUST_BACKTRACE = "1";
          inherit (self.checks.${system}.pre-commit-check) shellHook;
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
        vm-test = import ./nix/integration-test.nix {
          inherit pkgs system self;
          lib = nixpkgs.lib;
        };
        pre-commit-check = git-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            nixfmt.enable = true;
            rustfmt.enable = true;
            clang-format.enable = true;
          };
        };
      });
    };
}
