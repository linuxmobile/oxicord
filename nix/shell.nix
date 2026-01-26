{
  mkShell,
  stdenvAdapters,
  cargo-watch,
  oxicord,
}:
mkShell.override (old: {
  stdenv = stdenvAdapters.useMoldLinker old.stdenv;
}) {
  inputsFrom = [
    oxicord
  ];

  packages = [
    cargo-watch
  ];
}
