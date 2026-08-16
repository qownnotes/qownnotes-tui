{
  description = "qownnotes-tui: a QOwnNotes terminal browser and editor";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    devenv.url = "github:cachix/devenv";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      flake-utils,
      devenv,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        package = pkgs.rustPlatform.buildRustPackage {
          pname = "qownnotes-tui";
          version = "0.6.0";
          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./src
            ];
          };
          cargoLock.lockFile = ./Cargo.lock;
          strictDeps = true;
          nativeBuildInputs = [ pkgs.installShellFiles ];
          postInstall = ''
            installShellCompletion --cmd qownnotes-tui \
              --bash <($out/bin/qownnotes-tui --generate-completion bash) \
              --fish <($out/bin/qownnotes-tui --generate-completion fish) \
              --zsh <($out/bin/qownnotes-tui --generate-completion zsh)
          '';
          meta = {
            description = "Keyboard-first terminal browser for QOwnNotes-compatible note folders";
            homepage = "https://github.com/qownnotes/qownnotes-tui";
            license = pkgs.lib.licenses.gpl3Only;
            mainProgram = "qownnotes-tui";
            platforms = pkgs.lib.platforms.unix;
          };
        };
      in
      {
        packages = {
          default = package;
          qownnotes-tui = package;
        };

        apps.default = flake-utils.lib.mkApp { drv = package; };

        devShells.default = devenv.lib.mkShell {
          inherit inputs pkgs;
          modules = [
            (_: {
              devenv.root =
                let
                  root = builtins.getEnv "PWD";
                in
                if root != "" then root else self.outPath;
            })
            ./devenv.nix
          ];
        };

        formatter = pkgs.nixfmt;

        checks = {
          inherit package;
          formatting =
            pkgs.runCommand "qownnotes-tui-formatting"
              {
                nativeBuildInputs = [
                  pkgs.cargo
                  pkgs.rustfmt
                ];
              }
              ''
                cp -r ${self} source
                chmod -R u+w source
                cd source
                cargo fmt --check
                touch $out
              '';
        };
      }
    )
    // {
      homeModules.default = nixpkgs.lib.modules.importApply ./nix/home-manager.nix {
        qownnotes-tui = self;
      };
    };
}
