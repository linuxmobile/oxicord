{
  lib,
  rustPlatform,
  pkg-config,
  clang,
  makeBinaryWrapper,
  mold,
  dbus,
  chafa,
  glib,
}:
rustPlatform.buildRustPackage (final: let
  inherit (lib.fileset) toSource unions;
  inherit (lib) licenses platforms;
in {
  pname = "oxicord";
  version = "0.1.7";

  src = toSource {
    root = ../.;
    fileset = unions [
      ../src
      ../Cargo.lock
      ../Cargo.toml
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [
    pkg-config
    clang
    mold
    makeBinaryWrapper
  ];

  buildInputs = [
    dbus
    chafa
    glib
  ];

  meta = {
    description = "A lightweight, secure Discord terminal client";
    homepage = "https://github.com/linuxmobile/oxicord";
    license = licenses.mit;
    maintainers = [];
    mainProgram = "oxicord";
    platforms = platforms.unix;
  };
})
