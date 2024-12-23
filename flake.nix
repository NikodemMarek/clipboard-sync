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
            alias-create-clipboard-pipe = pkgs.writeShellScriptBin "clp" ''
              mkfifo /tmp/clipboard.pipe
              wl-paste --watch wl-paste --no-newline > /tmp/clipboard.pipe
            '';
          in
            rustPkgs.workspaceShell
            {
              packages = [cargo2nix.packages."${system}".cargo2nix];
              buildInputs = [alias-run-client alias-run-relay alias-create-clipboard-pipe];
              shellHook = ''
                printf "\e[33m
                  \e[1mr[rc]\e[0m\e[33m  -> run [r]elay [c]lient
                  \e[1mclp\e[0m\e[33m  -> create clipboard pipe
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
