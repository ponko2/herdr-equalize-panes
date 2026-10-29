{
  description = "herdr-equalize-panes";

  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    inputs@{ fenix, flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      perSystem =
        { pkgs, system, ... }:
        let
          rust-toolchain = pkgs.fenix.fromToolchainFile {
            file = ./rust-toolchain.toml;
            sha256 = "+xUr6Al1wZSqX8aEsq7DpuFOK4Ve/i+qTl+K5uWQ9GM=";
          };
        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [
              fenix.overlays.default
            ];
          };
          apps = {
            commitlint = {
              type = "app";
              program = "${pkgs.commitlint}/bin/commitlint";
            };
            deadnix = {
              type = "app";
              program = "${pkgs.deadnix}/bin/deadnix";
            };
            editorconfig-checker = {
              type = "app";
              program = "${pkgs.editorconfig-checker}/bin/editorconfig-checker";
            };
            jactionlint = {
              type = "app";
              program = "${
                pkgs.writeShellApplication {
                  name = "jactionlint";
                  runtimeInputs = [
                    pkgs.jactionlint
                    pkgs.shellcheck
                  ];
                  text = ''exec jactionlint "$@"'';
                }
              }/bin/jactionlint";
            };
            oxfmt = {
              type = "app";
              program = "${pkgs.oxfmt}/bin/oxfmt";
            };
            statix = {
              type = "app";
              program = "${pkgs.statix}/bin/statix";
            };
            yamllint = {
              type = "app";
              program = "${pkgs.yamllint}/bin/yamllint";
            };
          };
          devShells.default = pkgs.mkShell {
            packages = with pkgs; [
              cargo-features-manager
              cargo-nextest
              cargo-shear
              commitlint
              deadnix
              editorconfig-checker
              hk
              jactionlint
              nixfmt
              oxfmt
              rust-analyzer
              rust-toolchain
              shellcheck
              statix
              yamllint
            ];
          };
          formatter = pkgs.nixfmt-tree;
        };
    };
}
