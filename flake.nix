{
  description = "Oxicord - Discord TUI client";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = ["rust-src" "rust-analyzer" "clippy" "rustfmt"];
        };
      in {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "oxicord";
          version = "0.1.5";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [pkgs.pkg-config pkgs.clang pkgs.mold];

          buildInputs =
            [
              pkgs.dbus
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.Security
              pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            ];

          meta = with pkgs.lib; {
            description = "A lightweight, secure Discord terminal client";
            homepage = "https://github.com/linuxmobile/oxicord";
            license = licenses.mit;
            maintainers = [];
            mainProgram = "oxicord";
            platforms = platforms.unix;
          };
        };

        devShells.default = pkgs.mkShell {
          RUSTFLAGS = "-C link-arg=-fuse-ld=mold";
          packages = [
            rustToolchain
            pkgs.cargo-watch
            pkgs.pkg-config
            pkgs.clang
            pkgs.mold
          ];

          buildInputs =
            [
              pkgs.dbus
              pkgs.chafa
              pkgs.glib
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.Security
              pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            ];

          PKG_CONFIG_PATH = "${pkgs.chafa}/lib/pkgconfig:${pkgs.glib.dev}/lib/pkgconfig:${pkgs.dbus.dev}/lib/pkgconfig";
          LD_LIBRARY_PATH = "${pkgs.chafa}/lib:${pkgs.glib.out}/lib:${pkgs.dbus.lib}/lib";
        };
      }
    );
}
