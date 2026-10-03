{
  pkgs,
  system,
  self,
  lib,
}:
let
  # Fills a sandbox's private tmpfs until a file lands on the inode number
  # given as $1, then checks that file and its neighbours survive
  sandboxCollision = (pkgs system).writeShellScript "sandbox-collision" ''
    set -eu
    export PATH=${(pkgs system).coreutils}/bin:$PATH

    # tmpfs hands out inode numbers in percpu blocks of 1024 and can skip
    # straight past the target, so overshoot by more than one block
    n=$(($1 + 1100))
    i=1
    while [ "$i" -le "$n" ]; do
      : > "/tmp/f$i"
      i=$((i + 1))
    done

    collider=""
    for f in /tmp/f*; do
      if [ "$(stat -c %i "$f")" = "$1" ]; then
        collider="$f"
        break
      fi
    done
    # No collision means the checks below prove nothing
    [ -n "$collider" ]

    # inode_getattr and file_open, the dirent path is covered by the ls
    stat "$collider" > /dev/null
    : < "$collider"

    [ "$(ls -1 /tmp | wc -l)" -eq "$n" ]
  '';
in
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
    import json

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

    with subtest("Runtime process policies accept fresh and repeated requests"):
      smart_api = (
        "busctl --system --json=short call org.opengamingcollective.cardwire "
        "/org/opengamingcollective/cardwire "
        "org.opengamingcollective.cardwire.SmartPolicy "
      )
      # Hybrid mode leaves these PIDs unclassified. Use a new process for each
      # initial policy so every branch is tested against two empty policy maps.
      for index, initial in enumerate(["Allow_dGPU", "Force_dGPU", "Force_GPU"]):
        unit = f"cardwire-process-policy-{index}.service"
        machine.succeed(f"systemd-run --unit={unit} --property=Type=exec sleep infinity")
        try:
          pid = int(machine.succeed(f"systemctl show --property=MainPID --value {unit}").strip())
          t.assertGreater(pid, 1)
          status = json.loads(machine.succeed(smart_api + f"GetProcessStatus u {pid}"))
          t.assertEqual(status["data"], ["", []])

          for policy, value in [
            (initial, 1),
            (initial, 1),
            ("Allow_dGPU", 1),
            ("Force_dGPU", 0),
            ("Force_GPU", 1),
            ("Allow_dGPU", 1),
          ]:
            machine.succeed(smart_api + f"RequestProcessAccess usu {pid} {policy} {value}")
            status = json.loads(machine.succeed(smart_api + f"GetProcessStatus u {pid}"))
            expected = ["Allowed", [0]] if policy == "Allow_dGPU" else ["Forced", [value]]
            t.assertEqual(status["data"], expected)

          # Default still does nothing; this fix must not change its semantics.
          machine.succeed(smart_api + f"RequestProcessAccess usu {pid} Default 0")
          status = json.loads(machine.succeed(smart_api + f"GetProcessStatus u {pid}"))
          t.assertEqual(status["data"], ["Allowed", [0]])
        finally:
          machine.succeed(f"systemctl stop {unit}")

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

    with subtest("Sandboxes keep files that share an inode number with a blocked GPU"):
      machine.succeed("bwrap --dev-bind / / --tmpfs /tmp true")

      # Read it while nothing is blocked, the stat would be denied otherwise
      machine.succeed("cardwire set hybrid")
      dgpu_ino = machine.succeed("stat -c %i /dev/dri/renderD129").strip()

      machine.succeed("cardwire set integrated")
      # if this passed the sandbox check below would prove nothing
      machine.fail(": < /dev/dri/renderD129")

      machine.succeed(
        "bwrap --dev-bind / / --tmpfs /tmp ${sandboxCollision} " + dgpu_ino
      )

  '';
}
