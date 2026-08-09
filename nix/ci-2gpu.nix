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
      t.assertIn("integrated", machine.succeed("cat /var/lib/cardwire/mode.json"), "mode.json didnt get saved")

    with subtest("Switchback to hybrid mode"):
      t.assertIn("Mode has been set to Hybrid", machine.succeed("cardwire set hybrid"), "Couldn't set to hybrid mode")
      machine.succeed(": < /dev/dri/renderD129")
      t.assertIn("hybrid", machine.succeed("cat /var/lib/cardwire/mode.json"), "mode.json didnt get saved")

    with subtest("Try to block default gpu"):
      t.assertIn("Per GPU block is only available on manual mode", machine.fail("cardwire gpu 0 --block 2>&1"), "Default gpu got blocked")

    with subtest("Smart Mode Base Test"):
      machine.succeed("cardwire set smart")
      t.assertIn("smart", machine.succeed("cat /var/lib/cardwire/mode.json"))
      # In Smart mode, dGPU is blocked by default
      machine.fail(": < /dev/dri/renderD129")

    with subtest("Test Dynamic Analysis ENV Flags"):
      # CARDWIRE_ALLOW
      machine.succeed("CARDWIRE_ALLOW=1 sh -c 'sleep 0.5 && exec 3< /dev/dri/renderD129'")
      machine.fail("CARDWIRE_ALLOW=0 sh -c 'sleep 0.5 && exec 3< /dev/dri/renderD129'")
      # CARDWIRE_FORCE_DGPU
      machine.succeed("CARDWIRE_FORCE_DGPU=1 sh -c 'sleep 0.5 && exec 3< /dev/dri/renderD129'")
      machine.fail("CARDWIRE_FORCE_DGPU=0 sh -c 'sleep 0.5 && exec 3< /dev/dri/renderD129'")

    with subtest("Test cardwire launch Environment Injection for GPU 0"):
      env_out = machine.succeed("cardwire launch --gpu 0 env")
      t.assertIn("CARDWIRE_ALLOW=0", env_out, "Missing CARDWIRE_ALLOW=0 for iGPU default")

    with subtest("Test cardwire launch Environment Injection for GPU 1"):
      env_out = machine.succeed("cardwire launch --gpu 1 env")
      t.assertIn("CARDWIRE_FORCE_DGPU=1", env_out, "Missing CARDWIRE_FORCE_DGPU=1 for dGPU")
      t.assertIn("DRI_PRIME=pci", env_out, "Missing DRI_PRIME for dGPU")

    with subtest("Test cardwire launch Default GPU"):
      # Without --gpu, it should default to the unblocked discrete GPU (GPU 1)
      env_out = machine.succeed("cardwire launch env")
      t.assertIn("CARDWIRE_FORCE_DGPU=1", env_out, "Default launch didn't target dGPU (GPU 1)")
      t.assertIn("DRI_PRIME=pci", env_out, "Default launch didn't set DRI_PRIME")

  '';
}
