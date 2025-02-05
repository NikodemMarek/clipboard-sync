{
  inputs = {
    cargo2nix.url = "github:cargo2nix/cargo2nix/release-0.11.0";
    flake-utils.follows = "cargo2nix/flake-utils";
    nixpkgs.follows = "cargo2nix/nixpkgs";
  };

  outputs = inputs:
    with inputs;
      flake-utils.lib.eachDefaultSystem (
        system: let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [cargo2nix.overlays.default];
          };

          rustPkgs = pkgs.rustBuilder.makePackageSet {
            rustChannel = "nightly";
            extraRustComponents = ["rustfmt" "clippy"];
            packageFun = import ./Cargo.nix;
          };

          workspaceShell = let
            alias-run-client = pkgs.writeShellScriptBin "rc" ''cargo run --bin clipboard-sync-client'';
            alias-run-relay = pkgs.writeShellScriptBin "rr" ''cargo run --bin clipboard-sync-relay'';
            alias-generate-client-key = pkgs.writeShellScriptBin "gck" ''
              openssl genpkey -algorithm RSA -out clipboard-sync-client.key
              openssl rsa -pubout -in clipboard-sync-client.key -out clipboard-sync-client.pub
            '';
          in
            rustPkgs.workspaceShell
            {
              packages = [cargo2nix.packages."${system}".cargo2nix];
              buildInputs = [alias-run-client alias-run-relay alias-generate-client-key];
              shellHook = ''
                printf "\e[33m
                  \e[1mr[rc]\e[0m\e[33m  -> run [r]elay [c]lient
                  \e[1mgck\e[0m\e[33m  -> generate [g]enerate [c]lient [k]ey
                \e[0m"
              '';
            };
        in rec {
          devShells = {
            default = workspaceShell;
          };

          packages = {
            clipboard-sync-client = rustPkgs.workspace.clipboard-sync-client {};
            clipboard-sync-relay = rustPkgs.workspace.clipboard-sync-relay {};
            default = packages.clipboard-sync-client;
          };
        }
      )
      // {
        homeManagerModules = {
          default = self.homeManagerModules.clipboard-sync-client;
          clipboard-sync-client = import ./nix/hm-module.nix self;
        };
      };
}
