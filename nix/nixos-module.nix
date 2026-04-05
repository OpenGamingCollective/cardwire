self:
{
  lib,
  config,
  pkgs,
  ...
}:
let
  cfg = config.services.cardwire;
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
    };
  };
  config = lib.mkIf cfg.enable {
    # DBUS
    services.dbus.enable = true;
    services.dbus.packages = [ cfg.package ];
    # Cardwire package
    environment.systemPackages = [ cfg.package ];
    # systemd
    systemd.services.cardwired = {
      unitConfig = {
        Description = "Cardwire Daemon";
        Before = [
          "graphical.target"
        ];
      };
      serviceConfig = {
        Type = "dbus";
        BusName = "com.github.luytan.cardwire";
        ExecStart = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/cardwired";
      };
      wantedBy = [ "multi-user.target" ];
    };
  };
}
