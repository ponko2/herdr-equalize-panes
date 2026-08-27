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
            oxfmt = {
              type = "app";
              program = "${pkgs.oxfmt}/bin/oxfmt";
            };
            statix = {
              type = "app";
              program = "${pkgs.statix}/bin/statix";
            };
          };
          devShells.default = pkgs.mkShell {
            packages = with pkgs; [
              (pkgs.fenix.fromToolchainFile {
                file = ./rust-toolchain.toml;
                sha256 = "P30Tm3O7vQAE725YtDCDHGjNrSsfZO4us11UwJGZSJo=";
              })
              cargo-features-manager
              cargo-nextest
              cargo-shear
              commitlint
              deadnix
              editorconfig-checker
              hk
              nixd
              nixfmt
              oxfmt
              rust-analyzer
              statix
              yamllint
            ];
          };
          formatter = pkgs.nixfmt-tree;
          packages = {
            inherit (pkgs)
              direnv
              nix-direnv
              ;
          };
        };
    };
}
