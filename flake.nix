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
      packagesPerSystem = flake-utils.lib.eachDefaultSystem (system: {
        packages.default =
          let
            toolchain = fenix.packages.${system}.minimal.toolchain;
            pkgs = nixpkgs.legacyPackages.${system};
          in
          pkgs.callPackage ./nix { inherit toolchain; };
      });
    in
    packagesPerSystem
    // {
      nixosModules.default = import ./nix/nixos-module.nix self;
      nixosConfigurations.test-vm = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          self.nixosModules.default
          {
            services.cardwire.enable = true;
            boot.loader.grub.device = "nodev";
            fileSystems."/" = {
              device = "/dev/null";
              fsType = "ext4";
            };
          }
        ];
      };
    };
}
