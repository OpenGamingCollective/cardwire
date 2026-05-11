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
        BusName = "com.github.opengamingcollective.cardwire";
        ExecStart = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/cardwired";
        Restart = "on-failure";
        RestartSec = "5s";
      };
      wantedBy = [ "multi-user.target" ];
    };
  };
}
