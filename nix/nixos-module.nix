self:
{
  lib,
  system,
  config,
  pkgs,
  ...
}:
let
  inherit system;
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
    # systemd
    systemd.services.cardwired = {
      unitConfig = {
        description = "Cardwire Daemon";
        before = [
          "graphical.target"
        ];
      };
      serviceConfig = {
        Type = "dbus";
        BusName = "com.github.luytan.cardwire";
        ExecStart = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/cardwired";
      };
    };    
  };
}
