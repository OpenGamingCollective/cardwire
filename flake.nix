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
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      packagesPerSystem = flake-utils.lib.eachSystem supportedSystems (system: {
        packages.default =
          let
            toolchain = fenix.packages.${system}.minimal.toolchain;
            pkgs = nixpkgs.legacyPackages.${system};
          in
           pkgs.callPackage ./nix { inherit toolchain; };
       });
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
