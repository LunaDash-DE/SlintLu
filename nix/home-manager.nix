# Home Manager module:
#   imports = [ slintlu.homeManagerModules.default ];
#   programs.slintlu = { enable = true; configDir = ./my-shell; theme.dark = "nord"; };
self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.slintlu;
  json = pkgs.formats.json { };
in
{
  options.programs.slintlu = {
    enable = lib.mkEnableOption "the SlintLu desktop shell";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "slintlu.packages.\${system}.default";
      description = "The SlintLu package.";
    };

    configDir = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      example = lib.literalExpression "./shell";
      description = "Directory with `shell.slint` (and its other files), linked to ~/.config/slintlu.";
    };

    theme = lib.mkOption {
      inherit (json) type;
      default = { };
      example = {
        dark = "tokyo-night";
        colors.accent = "#ff79c6";
      };
      description = "Contents of ~/.config/slintlu/theme.json (see docs/THEMING.md).";
    };

    systemd.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Start SlintLu with graphical-session.target.";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile = lib.mkMerge [
      (lib.mkIf (cfg.configDir != null) {
        "slintlu" = {
          source = cfg.configDir;
          recursive = true;
        };
      })
      (lib.mkIf (cfg.theme != { }) { "slintlu/theme.json".source = json.generate "theme.json" cfg.theme; })
    ];

    systemd.user.services.slintlu = lib.mkIf cfg.systemd.enable {
      Unit = {
        Description = "SlintLu desktop shell";
        PartOf = [ "graphical-session.target" ];
        After = [ "graphical-session.target" ];
        Requisite = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = lib.getExe cfg.package;
        ExecReload = "${pkgs.coreutils}/bin/kill -HUP $MAINPID";
        Restart = "on-failure";
        RestartSec = 1;
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };
  };
}
