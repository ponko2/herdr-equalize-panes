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
            sha256 = "zm3dyIY2T414ZRR3EhLOvptzG6gta4WZUcawzMUWtqI=";
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
            actionlint = {
              type = "app";
              program = "${pkgs.actionlint}/bin/actionlint";
            };
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
            ghalint = {
              type = "app";
              program = "${pkgs.ghalint}/bin/ghalint";
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
            zizmor = {
              type = "app";
              program = "${pkgs.zizmor}/bin/zizmor";
            };
          };
          devShells.default = pkgs.mkShell {
            packages = with pkgs; [
              actionlint
              cargo-features-manager
              cargo-nextest
              cargo-shear
              commitlint
              deadnix
              editorconfig-checker
              ghalint
              hk
              nixfmt
              oxfmt
              rust-analyzer
              rust-toolchain
              statix
              yamllint
              zizmor
            ];
          };
          formatter = pkgs.nixfmt-tree;
        };
    };
}
