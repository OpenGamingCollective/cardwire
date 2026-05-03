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
          "-device virtio-gpu-pci,max_outputs=1"
          "-device virtio-gpu-pci,max_outputs=1"
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
      t.assertIn("renderD130", machine.succeed("ls -a /dev/dri"), "Missing DRM")
      t.assertIn("card2", machine.succeed("ls -a /dev/dri"), "Missing DRM")

    with subtest("Ensure cardwire is started and dbus works"):
      machine.wait_until_succeeds("su - john -c 'cardwire help'")

    with subtest("Try to switch to integrated and hybrid"):
      t.assertIn("Couldn't set mode to Integrated, the mode require exactly 2 GPUs", machine.succeed("cardwire set integrated 2>&1"), "Mode has been switched to integrated")
      t.assertIn("Couldn't set mode to Hybrid, the mode require exactly 2 GPUs", machine.succeed("cardwire set hybrid 2>&1"), "Mode has been switched to hybrid")

    with subtest("Set to manual, and block two gpus"):
      machine.succeed("cardwire set manual")
      machine.succeed("cardwire gpu 1 --block")
      machine.succeed("cardwire gpu 2 --block")
      t.assertIn("cannot be blocked", machine.succeed("cardwire gpu 0 --block 2>&1"), "Default gpu got blocked")


    with subtest("Check gpu_state.json to see if two gpus got blocked"):
      t.assertIn("2", machine.succeed("cat /var/lib/cardwire/gpu_state.json|grep true|wc -l"), "Only one or less GPU got blocked")

    with subtest("Check cardwire to see if two gpus got blocked"):
      t.assertIn("2", machine.succeed("cardwire list|grep 'true'|wc -l"), "Only one or less GPU got blocked")
      machine.fail(": < /dev/dri/renderD129")
      machine.fail(": < /dev/dri/renderD130")
      machine.fail(": < /dev/dri/card1")
      machine.fail(": < /dev/dri/card2")

    with subtest("Restart cardwire and check if gpu states got re-applied"):
      machine.wait_until_succeeds("systemctl stop cardwired.service")

      machine.succeed(": < /dev/dri/renderD129")
      machine.succeed(": < /dev/dri/renderD130")
      machine.succeed(": < /dev/dri/card1")
      machine.succeed(": < /dev/dri/card2")

      machine.wait_until_succeeds("systemctl start cardwired.service")

      t.assertIn("2", machine.succeed("cardwire list|grep 'true'|wc -l"), "Only one or less GPU got blocked")
      
      machine.fail(": < /dev/dri/renderD129")
      machine.fail(": < /dev/dri/renderD130")
      machine.fail(": < /dev/dri/card1")
      machine.fail(": < /dev/dri/card2")
  '';

}
