self: {
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.services.clipboard-sync;

  tomlFormat = pkgs.formats.toml {};
in {
  options = {
    services.clipboard-sync = {
      enable = lib.mkEnableOption "clipboard-sync-client";
      package = lib.mkPackageOption self.packages.${pkgs.system} "clipboard-sync-client" {
        default = "default";
        pkgsText = "clipboard-sync-client.packages.\${pkgs.system}";
      };

      autostart = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Start clipboard-sync-client after login.";
      };

      config = let
        conf.options = {
          relay = lib.mkOption {
            type = lib.types.str;
            default = "130.61.88.218:5200";
            description = "Address of the clipboard-sync-relay to connect to.";
          };

          client_key = lib.mkOption {
            type = lib.types.either lib.types.str lib.types.path;
            default = "";
            description = "Client key path to decrypt the communication with.";
          };
          peers_keys = lib.mkOption {
            type = lib.types.listOf (lib.types.either lib.types.str lib.types.path);
            default = [];
            description = "Paths to public keys of the peers to encrypt the communication with.";
          };
        };
      in
        lib.mkOption {
          type = lib.types.submodule conf;
          default = {};
          description = ''
            Configuration written to `$XDG_CONFIG_HOME/clipboard-sync/config.toml`.
          '';
        };
    };
  };

  config = lib.mkIf cfg.enable (lib.mkMerge [
    {
      home.packages = [cfg.package];

      xdg.configFile."clipboard-sync/config.toml".source = lib.mkIf (cfg.config != {}) (tomlFormat.generate "clipboard-sync-client-config.toml" cfg.config);

      systemd.user.services.clipboard-sync-client = lib.mkIf cfg.autostart {
        Unit = {
          Description = "clipboard-sync-client";
          Wants = ["network-online.target" "graphical.target"];
          After = ["network-online.target" "graphical.target"];
        };
        Install.WantedBy = ["default.target"];
        Service = {
          ExecStart = lib.getExe (pkgs.writeShellScriptBin "clipboard-sync-client" ''
            ${lib.getExe cfg.package} --config ${config.xdg.configFile."clipboard-sync/config.toml".source}
          '');
          Restart = "on-failure";
        };
      };
    }
  ]);
}
