self:
{
  lib,
  config,
  pkgs,
  ...
}:
let
  cfg = config.services.cardwire;
  tomlFormat = pkgs.formats.toml { };
in
{
  options = with lib; {
    services.cardwire = {
      enable = mkEnableOption "Enable cardwire";
      package = mkOption {
        type = types.package;
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
        description = "Cardwire package";
      };
      settings = mkOption {
        type = types.submodule {
          imports = [
            (lib.mkRenamedOptionModule [ "auto_apply_gpu_state" ] [ "global_settings" "restore_gpu_states" ])

            (lib.mkRenamedOptionModule
              [ "experimental_nvidia_block" ]
              [ "experimental_features" "advanced_nvidia_blocking" ]
            )

            (lib.mkRenamedOptionModule
              [ "battery_auto_switch" ]
              [ "global_settings" "battery_switch" "enabled" ]
            )

            (lib.mkRenamedOptionModule
              [ "battery_auto_switch_mode" ]
              [ "global_settings" "battery_switch" "ac_mode" ]
            )

            (lib.mkRenamedOptionModule
              [ "external_display_auto_switch" ]
              [ "global_settings" "switch_on_display" ]
            )
          ];
          options = {
            global_settings = mkOption {
              type = types.submodule {
                options = {
                  restore_gpu_states = mkOption {
                    type = types.bool;
                    default = true;
                    description = ''
                      Automatically restore GPU states on manual mode.
                    '';
                  };

                  switch_on_display = mkOption {
                    type = types.bool;
                    default = false;
                    description = ''
                      Automatically change the mode on display connection.
                    '';
                  };

                  battery_switch = mkOption {
                    type = types.submodule {
                      options = {
                        enabled = mkOption {
                          type = types.bool;
                          default = false;
                          description = ''
                            Automatically switch mode when the power source changes.
                          '';
                        };

                        ac_mode = mkOption {
                          type = types.enum [
                            "integrated"
                            "hybrid"
                            "manual"
                            "smart"
                          ];
                          default = "hybrid";
                          description = ''
                            The mode Cardwire switches to on AC power.
                          '';
                        };

                        bat_mode = mkOption {
                          type = types.enum [
                            "integrated"
                            "hybrid"
                            "manual"
                            "smart"
                          ];
                          default = "integrated";
                          description = ''
                            The mode Cardwire switches to on battery power.
                          '';
                        };
                      };
                    };
                    default = { };
                    description = ''
                      Settings for switching modes when the power source changes.
                    '';
                  };
                };
              };
              default = { };
              description = ''
                Global Cardwire settings.
              '';
            };

            experimental_features = mkOption {
              type = types.submodule {
                options = {
                  advanced_nvidia_blocking = mkOption {
                    type = types.bool;
                    default = true;
                    description = ''
                      Enable advanced blocking of Nvidia files. This feature is
                      experimental because these files can be shared across multiple
                      Nvidia GPUs.
                    '';
                  };

                  fake_drm_uevent = mkOption {
                    type = types.bool;
                    default = false;
                    description = ''
                      Enable fake DRM uevents.
                      Send fake DRM uevents on GPU blocking.
                      ('remove' on blocking, 'add' on unblocking)
                    '';
                  };
                };
              };
              default = { };
              description = ''
                Experimental Cardwire features.
              '';
            };

            switcheroo_settings = mkOption {
              type = types.submodule {
                options = {
                  enabled = mkOption {
                    type = types.bool;
                    default = true;
                    description = ''
                      Enable switcheroo integration.
                    '';
                  };

                  cardwire_envs = mkOption {
                    type = types.bool;
                    default = true;
                    description = ''
                      Include Cardwire environment variables in switcheroo integration.
                    '';
                  };

                  switcheroo_envs = mkOption {
                    type = types.bool;
                    default = true;
                    description = ''
                      Include switcheroo environment variables in switcheroo integration.
                    '';
                  };
                };
              };
              default = { };
              description = ''
                Switcheroo integration settings.
              '';
            };

            internal_whitelist = mkOption {
              type = types.submodule {
                options = {
                  packages_managers = mkOption {
                    type = types.bool;
                    default = true;
                    description = ''
                      Enable the internal whitelist for package managers.
                    '';
                  };

                  vfio = mkOption {
                    type = types.bool;
                    default = true;
                    description = ''
                      Enable the internal whitelist for VFIO.
                    '';
                  };

                  systemd = mkOption {
                    type = types.bool;
                    default = true;
                    description = ''
                      Enable the internal whitelist for systemd.
                    '';
                  };

                  nvidia_powerd = mkOption {
                    type = types.bool;
                    default = true;
                    description = ''
                      Enable the internal whitelist for nvidia-powerd.
                    '';
                  };
                };
              };
              default = { };
              description = ''
                Internal whitelist settings.
              '';
            };
          };
        };
        default = { };
        description = ''
          Configuration for {file}`/etc/cardwire.toml`
          See <https://opengamingcollective.github.io/cardwire/getting-started/usage>
        '';
      };
    };
  };
  config = lib.mkIf cfg.enable {
    # /etc/cardwire/cardwire.toml
    environment.etc."cardwire/cardwire.toml" = {
      source = tomlFormat.generate "cardwire.toml" (
        builtins.removeAttrs cfg.settings [
          "auto_apply_gpu_state"
          "battery_auto_switch"
          "battery_auto_switch_mode"
          "experimental_nvidia_block"
          "external_display_auto_switch"
        ]
      );
    };
    # DBUS
    services.dbus.enable = true;
    services.dbus.packages = [ cfg.package ];
    # Cardwire package
    environment.systemPackages = [ cfg.package ];
    # Shell completions
    environment.pathsToLink = [
      "/share/bash-completion"
      "/share/fish"
      "/share/zsh"
    ];
    # systemd
    systemd.services.cardwired = {
      unitConfig = {
        Description = "Cardwire Daemon";
        Before = [
          "graphical.target"
          "display-manager.service"
        ];
      };
      serviceConfig = {
        Type = "dbus";
        BusName = "org.opengamingcollective.cardwire";
        ExecStart = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/cardwired";
        Restart = "on-failure";
        RestartSec = "5s";
        # Hardening
        User = "root";
        PrivateNetwork = true;
        PrivateTmp = true;
        ProtectHostname = true;
        NoNewPrivileges = true;
        ProtectClock = true;
        ProtectSystem = "strict";
        StateDirectory = "cardwire";
        StateDirectoryMode = "0700";
        ConfigurationDirectory = "cardwire";
        ConfigurationDirectoryMode = "0700";
        ProtectHome = "read-only";
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectKernelModules = true;
        RestrictAddressFamilies = [
          "AF_UNIX"
          "AF_NETLINK"
        ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        UMask = "0077";
        IPAddressDeny = "any";
        CapabilityBoundingSet = [
          "CAP_SYS_ADMIN"
          "CAP_BPF"
          "CAP_SYS_PTRACE"
          "CAP_DAC_OVERRIDE"
        ];
        SystemCallFilter = [
          "~`@cpu-emulation` `@module` `@obsolete` `@raw-io` `@reboot` `@swap`"
        ];
      };
      wantedBy = [ "multi-user.target" ];
    };
  };
}
