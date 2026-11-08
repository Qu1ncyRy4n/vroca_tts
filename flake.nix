{
  description = "Vroca — Text-to-Speech & Assistive Reading Subsystem";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      mkPackages =
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          vroca = pkgs.callPackage ./nix/package.nix { };
        in
        {
          inherit vroca;
          default = vroca;
          inherit (vroca)
            vroca-cli
            vroca-daemon
            vroca-panel
            vroca-pitch-tables
            ;
        }
        // nixpkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
          inherit (vroca) vroca-overlay;
        };
    in
    {
      packages = forAllSystems mkPackages;

      overlays.default = final: _prev: {
        vroca = final.callPackage ./nix/package.nix { };
      };

      homeModules.default = import ./nix/home-manager.nix;

      checks = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        import ./nix/checks.nix {
          inherit (pkgs)
            runCommand
            python3
            lib
            stdenv
            lua
            ;
          inherit (self.packages.${system}) vroca;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          isLinux = pkgs.stdenv.hostPlatform.isLinux;
          pyEnv = pkgs.python3.withPackages (ps: [
            ps.sherpa-onnx
            ps.numpy
            ps.pygobject3
            ps.pycairo
          ]);
        in
        {
          default = pkgs.mkShell {
            GI_TYPELIB_PATH = self.packages.${system}.vroca.vroca-panel.typelibs;
            packages = [
              pyEnv
              pkgs.uv
              pkgs.mpv
              pkgs.socat
              pkgs.gtk4

              # Rust stays on nixpkgs' stable toolchain; no nightly or extra input
              # is needed for the staged replacement in rust_impl/.
              pkgs.rustc
              pkgs.cargo
              pkgs.rustfmt
              pkgs.clippy
              pkgs.rust-analyzer
            ]
            # Only the overlay and user-unit tools require Linux.
            ++ nixpkgs.lib.optionals isLinux [
              pkgs.systemd
              pkgs.gtk4-layer-shell
            ];
          };
        }
      );
    };
}
