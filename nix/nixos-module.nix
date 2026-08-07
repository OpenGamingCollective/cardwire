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
      settings = {
        auto_apply_gpu_state = mkOption {
          type = types.bool;
          default = true;
        };
        experimental_nvidia_block = mkOption {
          type = types.bool;
          default = false;
        };
        battery_auto_switch = mkOption {
          type = types.bool;
          default = false;
        };
        battery_auto_switch_mode = mkOption {
          type = types.str;
          default = "hybrid";
        };
        external_display_auto_switch = mkOption {
          type = types.bool;
          default = false;
          description = "Automatically make GPUs available for displays connected to dGPU-only ports";
        };
      };
    };
  };
  config = lib.mkIf cfg.enable {
    # /etc/cardwire/cardwire.toml
    environment.etc."cardwire/cardwire.toml" = {
      source = tomlFormat.generate "cardwire.toml" cfg.settings;
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
        Wants = [ "systemd-udev-settle.service" ];
        After = [
          "dbus.service"
          "systemd-udev-settle.service"
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
