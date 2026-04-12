{
  pkgs,
  system,
  self,
  lib,
}:
(pkgs system).testers.runNixOSTest {
  name = "cardwire-test";
  nodes.machine =
    {
      config,
      lib,
      ...
    }:
    {
      imports = [
        self.nixosModules.default
        ./vm-configuration.nix
      ];

      virtualisation.qemu.options = [
        "-machine q35,kernel-irqchip=split"
        "-device intel-iommu,intremap=on,device-iotlb=on"
        "-vga none"
        "-device virtio-vga,id=gpu0"
        "-device virtio-vga,id=gpu1"
      ];
      virtualisation.memorySize = 1024;
      networking.useDHCP = false;
      networking.interfaces = lib.mkForce { };
    };

  testScript = ''
    machine.start()
    machine.wait_for_unit("default.target")
    with subtest("Wait for boot and services"):
        machine.wait_for_unit("multi-user.target")
        machine.wait_for_unit("dbus.service")
        machine.wait_for_unit("cardwired.service")

    with subtest("Check for DRM Devices"):
      # Check the DRM devices
      t.assertIn("renderD128", machine.succeed("ls -a /dev/dri"), "Missing DRM")
      t.assertIn("card0", machine.succeed("ls -a /dev/dri"), "Missing DRM")
      t.assertIn("renderD129", machine.succeed("ls -a /dev/dri"), "Missing DRM")
      t.assertIn("card1", machine.succeed("ls -a /dev/dri"), "Missing DRM")

    with subtest("Ensure cardwire is started"):
      machine.wait_until_succeeds("su - john -c 'cardwire help'")

    with subtest("Switch to Integrated mode"):
      # Check if cardwire detect both video card
      t.assertIn("renderD128", machine.succeed("su - john -c 'cardwire list'"), "Missing RenderD128 in cardwire")
      machine.succeed("test -e /dev/dri/renderD129")
      t.assertIn("Mode has been set to integrated", machine.succeed("su - john -c 'cardwire set integrated'"), "Couldn't set to integrated mode")
      machine.fail(": < /dev/dri/renderD129")
    with subtest("Switchback to hybrid mode"):
      t.assertIn("Mode has been set to hybrid", machine.succeed("su - john -c 'cardwire set hybrid'"), "Couldn't set to hybrid mode")
      machine.succeed(": < /dev/dri/renderD129")
  '';
}
