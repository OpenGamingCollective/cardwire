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
          "-device virtio-gpu-pci,max_outputs=1"
          "-device virtio-gpu-pci,max_outputs=1"
          "-device virtio-gpu-pci,max_outputs=1"
          "-device virtio-gpu-pci,max_outputs=1"
          "-device virtio-gpu-pci,max_outputs=1"
          "-device virtio-gpu-pci,max_outputs=1"
          "-device virtio-gpu-pci,max_outputs=1"
          "-device virtio-gpu-pci,max_outputs=1"
          "-device virtio-gpu-pci,max_outputs=1"
          "-device virtio-gpu-pci,max_outputs=1"
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
      t.assertIn("33", machine.succeed("ls -a /dev/dri | wc -l"), "Missing DRMs, must be 31")

    with subtest("Ensure cardwire is started and dbus works"):
      machine.wait_until_succeeds("su - john -c 'cardwire help'")

    with subtest("Ensure files are present"):
      machine.succeed("cat /etc/cardwire/cardwire.toml")
      machine.succeed("cat /var/lib/cardwire/gpu_state.json")
      machine.succeed("cat /var/lib/cardwire/mode.json")

    with subtest("Check if cardwire found all gpus"):
      t.assertIn("17", machine.succeed("cardwire list | wc -l"), "Must be 17 (15 GPUs + 2 headers)")

    with subtest("Try to switch to integrated and hybrid"):
      t.assertIn("Couldn't set mode to Integrated, the mode require exactly 2 GPUs", machine.succeed("cardwire set integrated 2>&1"), "Mode has been switched to integrated")
      t.assertIn("Couldn't set mode to Hybrid, the mode require exactly 2 GPUs", machine.succeed("cardwire set hybrid 2>&1"), "Mode has been switched to hybrid")

    with subtest("Set to manual, and block 14 gpus"):
      machine.succeed("cardwire set manual")
      t.assertIn("cannot be blocked", machine.succeed("cardwire gpu 0 --block 2>&1"), "Default gpu got blocked")

      for x in range(1, 15):
        machine.succeed(f'cardwire gpu {x} --block')

    with subtest("Check cardwire to see if 14 gpus got blocked"):
      t.assertIn("14", machine.succeed("cardwire list|grep 'on'|wc -l"), "Only 13 or less got blocked")

      for x in range(1, 15):
        cardid = 0 + x
        renderid = 128 + x
        machine.succeed(f'cardwire gpu {x} --block')
        machine.fail(f": < /dev/dri/renderD{renderid}")
        machine.fail(f": < /dev/dri/card{cardid}")
      machine.succeed(": < /dev/dri/renderD128")
      machine.succeed(": < /dev/dri/card0")

    with subtest("Check gpu_state.json to see if two gpus got blocked"):
      t.assertIn("14", machine.succeed("cat /var/lib/cardwire/gpu_state.json|grep true|wc -l"), "Only 13 or less got blocked")


    with subtest("Restart cardwire and check if gpu states got re-applied"):
      machine.wait_until_succeeds("systemctl stop cardwired.service")

      for x in range(0, 15):
        cardid = 0 + x
        renderid = 128 + x
        machine.succeed(f": < /dev/dri/renderD{renderid}")
        machine.succeed(f": < /dev/dri/card{cardid}")

      machine.wait_until_succeeds("systemctl start cardwired.service")
      
      t.assertIn("14", machine.succeed("cardwire list|grep 'on'|wc -l"), "Only 13 or less got blocked")

      for x in range(1, 15):
        cardid = 0 + x
        renderid = 128 + x
        machine.succeed(f'cardwire gpu {x} --block')
        machine.fail(f": < /dev/dri/renderD{renderid}")
        machine.fail(f": < /dev/dri/card{cardid}")
      machine.succeed(": < /dev/dri/renderD128")
      machine.succeed(": < /dev/dri/card0")
  '';

}
