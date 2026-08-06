{
  description = "Vroca — Text-to-Speech & Assistive Reading Subsystem";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          pyEnv = pkgs.python3.withPackages (ps: with ps; [
            sherpa-onnx pygobject3 pycairo numpy
          ]);
        in
        {
          default = pkgs.mkShell {
            packages = [
              pyEnv
              pkgs.uv
              pkgs.mpv
              pkgs.socat

              # Rust toolchain for the staged replacement in rust_impl/.
              # Taken from nixpkgs rather than a rust-overlay input: the
              # migration needs a stable toolchain, not a pinned nightly, and
              # avoiding the extra input keeps flake.lock to one entry.
              pkgs.rustc
              pkgs.cargo
              pkgs.rustfmt
              pkgs.clippy
              pkgs.rust-analyzer
            ]
            # systemd and the GTK layer-shell stack are Linux-only. Listing
            # them unconditionally made the advertised darwin shells fail to
            # evaluate at all, systemd first and gtk4-layer-shell behind it.
            ++ nixpkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
              pkgs.systemd
              pkgs.gtk4
              pkgs.gtk4-layer-shell
            ];
          };
        });
    };
}
