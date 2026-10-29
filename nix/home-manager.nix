{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.vroca;
  isLinux = pkgs.stdenv.hostPlatform.isLinux;
  isDarwin = pkgs.stdenv.hostPlatform.isDarwin;
  logDirectory = "${config.home.homeDirectory}/Library/Logs";
in
{
  options.services.vroca = {
    enable = lib.mkEnableOption "the Vroca per-user speech daemon";
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.callPackage ./package.nix { };
      defaultText = lib.literalExpression "pkgs.callPackage ./nix/package.nix { }";
      description = "Vroca package providing bin/tts and bin/tts-daemon.";
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        assertions = [
          {
            assertion = isLinux || isDarwin;
            message = "The Vroca service supports Linux and macOS only.";
          }
        ];
        home.packages = [ cfg.package ];
      }
      (lib.mkIf isLinux {
        systemd.user.services.tts = {
          Unit = {
            Description = "Vroca speech synthesis daemon";
            PartOf = [ "graphical-session.target" ];
            After = [ "graphical-session.target" ];
          };
          Service = {
            ExecStart = "${cfg.package}/bin/tts-daemon";
            Restart = "on-failure";
            RestartSec = 3;
            # Keep credentials in the existing user-managed file, never in a
            # derivation or a module option containing secret values.
            EnvironmentFile = "-%E/tts/env";
            Environment = [ "XDG_CONFIG_HOME=${config.xdg.configHome}" ];
          };
          Install.WantedBy = [ "graphical-session.target" ];
        };
      })
      (lib.mkIf isDarwin {
        home.activation.vrocaLogDirectory =
          lib.hm.dag.entryBetween [ "setupLaunchAgents" ] [ "writeBoundary" ]
            ''
              run ${pkgs.coreutils}/bin/mkdir -p ${lib.escapeShellArg logDirectory}
            '';
        launchd.agents.vroca = {
          enable = true;
          # Home Manager installs enabled agents in gui/$UID.
          config = {
            ProgramArguments = [ "${cfg.package}/bin/tts-daemon" ];
            RunAtLoad = true;
            # Match systemd Restart=on-failure: `tts quit` must stay stopped.
            KeepAlive.SuccessfulExit = false;
            ThrottleInterval = 3;
            ProcessType = "Background";
            EnvironmentVariables = {
              HOME = config.home.homeDirectory;
              XDG_CONFIG_HOME = config.xdg.configHome;
            };
            StandardOutPath = "${logDirectory}/vroca.log";
            StandardErrorPath = "${logDirectory}/vroca.log";
          };
        };
      })
    ]
  );
}
