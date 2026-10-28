# Run against the Home Manager and nixpkgs sources used by a consuming config.
# This evaluates real Home Manager option types without activating a profile.
{ nixpkgs, homeManager }:
let
  evaluate =
    system: enabled:
    let
      pkgs = import nixpkgs { inherit system; };
    in
    (import (homeManager + "/modules") {
      inherit pkgs;
      configuration = {
        imports = [ ../nix/home-manager.nix ];
        home.username = "vroca-test";
        home.homeDirectory = "/tmp/vroca-home";
        home.stateVersion = "25.11";
        services.vroca.enable = enabled;
      };
    }).config;
  linux = evaluate "x86_64-linux" true;
  mac = evaluate "aarch64-darwin" true;
  disabledLinux = evaluate "x86_64-linux" false;
  disabledMac = evaluate "aarch64-darwin" false;
in
assert linux.systemd.user.services.tts.Service.Restart == "on-failure";
assert
  linux.systemd.user.services.tts.Service.ExecStart
  == [ "${linux.services.vroca.package}/bin/tts-daemon" ];
assert linux.systemd.user.services.tts.Service.EnvironmentFile == "-%E/tts/env";
assert linux.systemd.user.services.tts.Install.WantedBy == [ "graphical-session.target" ];
assert !(linux.launchd.agents ? vroca);
assert builtins.elem linux.services.vroca.package linux.home.packages;
assert mac.launchd.agents.vroca.enable;
assert mac.launchd.agents.vroca.domain == "gui";
assert
  mac.launchd.agents.vroca.config.ProgramArguments
  == [ "${mac.services.vroca.package}/bin/tts-daemon" ];
assert mac.launchd.agents.vroca.config.RunAtLoad;
assert !mac.launchd.agents.vroca.config.KeepAlive.SuccessfulExit;
assert
  mac.launchd.agents.vroca.config.StandardErrorPath == "/tmp/vroca-home/Library/Logs/vroca.log";
assert !(mac.launchd.agents.vroca.config.EnvironmentVariables ? XDG_RUNTIME_DIR);
assert !(mac.systemd.user.services ? tts);
assert builtins.elem "setupLaunchAgents" mac.home.activation.vrocaLogDirectory.before;
assert builtins.elem mac.services.vroca.package mac.home.packages;
assert !(disabledLinux.systemd.user.services ? tts);
assert !(disabledMac.launchd.agents ? vroca);
assert !(disabledMac.home.activation ? vrocaLogDirectory);
assert !(builtins.elem disabledLinux.services.vroca.package disabledLinux.home.packages);
assert !(builtins.elem disabledMac.services.vroca.package disabledMac.home.packages);
{
  linux = "systemd service and package verified";
  darwin = "LaunchAgent and package verified";
  disabled = "no Vroca package or service installed";
}
