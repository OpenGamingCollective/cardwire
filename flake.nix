{
  description = "Cardwire, a GPU manager for laptop and workstation";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs =
    {
      self,
      nixpkgs,
      fenix,
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
      devShells = forAllSystems (system: {
        default = (pkgs system).mkShell {
          packages = [
            (toolchainFor system)
            (pkgs system).clang
            (pkgs system).libbpf
          ];
          RUST_SRC_PATH = "${(fenixpkgs system).stable.rust-src}/lib/rustlib/src/rust/library";
          RUST_BACKTRACE = "1";
        };
      });
      nixosModules.default = import ./nix/nixos-module.nix self;
      nixosConfigurations = nixpkgs.lib.genAttrs supportedSystems (
        system:
        import ./nix/test-vm.nix {
          inherit nixpkgs self system;
        }
      );
    };
}
