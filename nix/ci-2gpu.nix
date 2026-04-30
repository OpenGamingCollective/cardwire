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

      virtualisation = {
        memorySize = 1024;
        graphics = false;
        diskImage = null;
        qemu.options = [
          "-machine q35,accel=kvm,kernel-irqchip=split"
          "-device intel-iommu,intremap=on,device-iotlb=on"
          "-vga none"
          "-device virtio-gpu-pci,id=igpu,max_outputs=2"
          "-device virtio-gpu-pci,id=dgpu,max_outputs=1"
        ];
      };
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

    with subtest("Ensure cardwire is started and dbus works"):
      machine.wait_until_succeeds("su - john -c 'cardwire help'")

    with subtest("Ensure files are present"):
      machine.succeed("cat /etc/cardwire/cardwire.toml")
      machine.succeed("cat /var/lib/cardwire/gpu_state.json")
      machine.succeed("cat /var/lib/cardwire/mode.json")

    with subtest("Switch to Integrated mode"):
      # Check if cardwire detect both video card
      t.assertIn("renderD128", machine.succeed("cardwire list"), "Missing RenderD128 in cardwire")
      machine.succeed("test -e /dev/dri/renderD129")
      t.assertIn("Mode has been set to Integrated", machine.succeed("cardwire set integrated"), "Couldn't set to integrated mode")
      machine.fail(": < /dev/dri/renderD129")
      t.assertIn("Integrated", machine.succeed("cat /var/lib/cardwire/mode.json"), "mode.json didnt get saved")

    with subtest("Switchback to hybrid mode"):
      t.assertIn("Mode has been set to Hybrid", machine.succeed("cardwire set hybrid"), "Couldn't set to hybrid mode")
      machine.succeed(": < /dev/dri/renderD129")
      t.assertIn("Hybrid", machine.succeed("cat /var/lib/cardwire/mode.json"), "mode.json didnt get saved")

    with subtest("Try to block default gpu"):
      t.assertIn("cannot be blocked", machine.succeed("cardwire gpu 0 --block 2>&1"), "Default gpu got blocked")
  '';
}
