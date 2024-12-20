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
            alias-run = pkgs.writeShellScriptBin "r" ''cargo run'';
          in
            rustPkgs.workspaceShell
            {
              packages = [cargo2nix.packages."${system}".cargo2nix];
              buildInputs = [alias-run];
              shellHook = ''
                printf "\e[33m
                  \e[1mr\e[0m\e[33m  -> run
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
      );
}
