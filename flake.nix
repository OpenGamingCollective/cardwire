{
  description = "Cardwire - GPU manager for laptop and workstation";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "cardwire";
          version = "0.10.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.clang
            pkgs.llvm
            pkgs.bpf-linker
            pkgs.bpftools
          ];
          buildInputs = [
            pkgs.hwdata
            pkgs.libbpf
          ];
          meta = with pkgs.lib; {
            description = "A program to block dgpu on Linux";
            homepage = "https://github.com/luytan/cardwire";
            license = licenses.mit;
            mainProgram = "cardwire";
            platforms = platforms.linux;
          };
          # Patch the source code to point to the correct hwdata location in the Nix store
          postPatch = ''
            substituteInPlace crates/cardwire-core/src/iommu/pci.rs \
              --replace "/usr/share/hwdata/pci.ids" "${pkgs.hwdata}/share/hwdata/pci.ids"
          '';

          # Make D-Bus configuration available
          postInstall = ''
            mkdir -p $out/share/dbus-1/system.d
            install -m 644 ${./assets/com.cardwire.daemon.conf} \
              $out/share/dbus-1/system.d/com.cardwire.daemon.conf
          '';

        };

        nixosModule =
          {
            config,
            lib,
            pkgs,
            ...
          }:
          with lib;
          let
            cfg = config.services.cardwire;
          in
          {
            options.services.cardwire = {
              enable = mkEnableOption "cardwire daemon";
              package = mkOption {
                type = types.package;
                default = self.packages.${pkgs.system}.default;
                description = "cardwire daemon package";
              };
            };

            config = mkIf cfg.enable {
              environment.systemPackages = [ cfg.package ];
              services.dbus.packages = [ cfg.package ];
              services.dbus.enable = true;

              systemd.services.cardwired = {
                description = "Cardwire Daemon";
                after = [
                  "dbus.service"
                  "network.target"
                ];
                requires = [ "dbus.service" ];

                before = [
                  "graphical.target"
                  "multi-user.target"
                  "display-manager.service"
                ];

                serviceConfig = {
                  Type = "dbus";
                  BusName = "com.cardwire.daemon";
                  ExecStart = "${cfg.package}/bin/cardwired";
                  Restart = "on-failure";
                  RestartSec = "5s";
                  User = "root";

                  Environment = [
                    "PATH=${
                      lib.makeBinPath [
                        pkgs.hwdata
                        pkgs.pciutils
                        pkgs.usbutils
                      ]
                    }:/run/current-system/sw/bin"
                  ];
                };

                wantedBy = [ "multi-user.target" ];
              };
            };
          };
        devShells.default = pkgs.mkShell {
          inputsFrom = [ self.packages.${system}.default ];
          buildInputs = with pkgs; [
            rust-analyzer
            rustfmt
            clippy
          ];
          shellHook = ''
            DBUS_FILE="/tmp/cardwire_dbus"

            start_bus() {
                sudo dbus-daemon --session --print-address --fork > "$DBUS_FILE"
                echo "Dev D-Bus started"
            }

            run_daemon() {
                export DBUS_SYSTEM_BUS_ADDRESS=$(cat "$DBUS_FILE")
                sudo -E result/bin/cardwired "$@"
            }

            run_cli() {
                export DBUS_SYSTEM_BUS_ADDRESS=$(cat "$DBUS_FILE")
                sudo -E result/bin/cardwire "$@"
            }

            echo " Cardwire dev environment"
            echo " Available commands:"
            echo "  1. start_bus        (run once to start the dev D-Bus)"
            echo "  2. run_daemon       (starts the background service)"
            echo "  3. run_cli <args>   (ex: run_cli get)"
          '';
        };
      }
    );
}
