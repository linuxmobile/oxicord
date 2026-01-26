{
  description = "Oxicord - Discord TUI client";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    systems.url = "github:nix-systems/default";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    systems,
    rust-overlay,
    ...
  }: let
    withOverlay = pkgs: pkgs.extend (import rust-overlay);
    eachSystem = fn:
      nixpkgs.lib.genAttrs
      (import systems)
      (system: fn (withOverlay nixpkgs.legacyPackages.${system}));
  in {
    packages = eachSystem (pkgs: let
      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [
          "rust-src"
          "rust-analyzer"
          "clippy"
          "rustfmt"
        ];
      };
    in {
      default = pkgs.callPackage ./nix/package.nix {
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };
      };
    });

    devShells = eachSystem (pkgs: {
      default = pkgs.callPackage ./nix/shell.nix {
        oxicord = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      };
    });
  };
}
