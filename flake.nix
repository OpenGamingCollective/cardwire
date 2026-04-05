{
  description = "Cardwire, a GPU manager for laptop and workstation";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
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
      flake-utils,
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      packagesPerSystem = flake-utils.lib.eachSystem supportedSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          fenixPkgs = fenix.packages.${system};

          toolchain = fenixPkgs.combine [
            fenixPkgs.stable.cargo
            fenixPkgs.stable.rustc
            fenixPkgs.stable.rustfmt
            fenixPkgs.stable.clippy
            fenixPkgs.stable.rust-src
          ];
        in
        {
          packages.default = pkgs.callPackage ./nix { inherit toolchain; };
          devShells.default = pkgs.mkShell {
            packages = [ toolchain pkgs.clang pkgs.libbpf ];
            RUST_SRC_PATH = "${fenixPkgs.stable.rust-src}/lib/rustlib/src/rust/library";
            RUST_BACKTRACE = "1";
          };
        }
      );
      nixosConfigurationsPerSystem = nixpkgs.lib.genAttrs supportedSystems (
        system:
        import ./nix/test-vm.nix {
          inherit nixpkgs self system;
        }
      );
    in
    packagesPerSystem
    // {
      nixosModules.default = import ./nix/nixos-module.nix self;
      nixosConfigurations = nixosConfigurationsPerSystem;
    };
}
