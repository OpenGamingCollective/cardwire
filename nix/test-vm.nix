{
  nixpkgs,
  system,
  self,
}:
nixpkgs.lib.nixosSystem {
  inherit system;
  modules = [
    self.nixosModules.default
    {
      imports = [ ./vm-configuration.nix ];
      boot.loader.grub.device = "nodev";
      fileSystems."/" = {
        device = "/dev/null";
        fsType = "ext4";
      };
    }
  ];
}
