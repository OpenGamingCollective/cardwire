{
  pkgs,
  ...
}:
{
  # Cardwire stuffs
  services.cardwire.enable = true;
  services.dbus.enable = true;
  # VM stuffs
  boot.loader.systemd-boot.enable = true;
  boot.loader.efi.canTouchEfiVariables = true;
  # Why john? idk i was reading some manwha and the dude was named john doe
  users.users.john = {
    isNormalUser = true;
    extraGroups = [ "wheel" ];
    initialPassword = "doe";
  };
  boot.kernelParams = [
    "intel_iommu=on"
    "iommu=pt"
  ];
  security.sudo.wheelNeedsPassword = false;
  environment.systemPackages = with pkgs; [
    pciutils
    fish
  ];
  services.getty.autologinUser = "john";
  virtualisation.vmVariant = {
    virtualisation = {
      memorySize = 512;
      cores = 2;
      graphics = false;
      diskImage = null;
      qemu.options = [
        "-machine q35,accel=kvm,kernel-irqchip=split"
        "-device intel-iommu,intremap=on,device-iotlb=on"
        "-vga none"
        "-device virtio-vga,id=gpu0"
        "-device virtio-vga,id=gpu1"
      ];
    };
  };
  programs.bash = {
    enable = true;
    shellAliases = {
      shut = "sudo shutdown -h now";
    };
  };
  system.stateVersion = "26.06";
}
