# TODO: move this to nixpkgs
# This file aims to be a replacement for the nixpkgs derivation.

{
  buildFeatures ? [ ],
  buildNoDefaultFeatures ? false,
  buildPackages,
  dbus,
  fetchFromGitHub,
  installManPages ? stdenv.buildPlatform.canExecute stdenv.hostPlatform,
  installShellCompletions ? stdenv.buildPlatform.canExecute stdenv.hostPlatform,
  installShellFiles,
  lib,
  openssl,
  pkg-config,
  rustPlatform,
  sqlite,
  stdenv,
}:

let
  nativeTls = builtins.elem "native-tls" buildFeatures;

  # A feature is on when asked for by name, or when the defaults are kept,
  # `notify` and `pimdir` being among them.
  enabled = feature: !buildNoDefaultFeatures || builtins.elem feature buildFeatures;

  # `vendored` is added on windows, where dbus and SQLite are built from
  # source; elsewhere it is asked for by name or left out.
  vendored = stdenv.hostPlatform.isWindows || builtins.elem "vendored" buildFeatures;

  # Desktop notifications go through the system dbus.
  systemDbus = enabled "notify" && !vendored;

  # The pimdir store links SQLite.
  systemSqlite = enabled "pimdir" && !vendored;

  # dbus calls libgcc outline atomics that the static aarch64 link cannot
  # resolve (__aarch64_ldset4_sync & co), so inline them instead.
  dbus' =
    if stdenv.hostPlatform.isLinux && stdenv.hostPlatform.isAarch64 then
      dbus.overrideAttrs (old: {
        env = (old.env or { }) // {
          NIX_CFLAGS_COMPILE = (old.env.NIX_CFLAGS_COMPILE or "") + " -mno-outline-atomics";
        };
      })
    else
      dbus;

in
rustPlatform.buildRustPackage (finalAttrs: {
  __structuredAttrs = true;

  inherit buildNoDefaultFeatures;

  pname = "carillon";
  version = "0.2.0";
  cargoHash = "";

  src = fetchFromGitHub {
    owner = "pimalaya";
    repo = finalAttrs.pname;
    tag = "v${finalAttrs.version}";
    hash = "";
  };

  env = {
    # pkg-config hands the linker libdbus and libsqlite3 but no rpath,
    # leaving a binary that cannot find them: not in postInstall, which runs
    # it, nor once installed.
    NIX_LDFLAGS = lib.concatStringsSep " " (
      lib.optional systemDbus ("-rpath " + lib.getLib dbus' + "/lib")
      ++ lib.optional systemSqlite ("-rpath " + lib.getLib sqlite + "/lib")
    );

    # openssl should not be provided by vendors, not even on windows
    OPENSSL_NO_VENDOR = 1;
  };

  nativeBuildInputs = [
    pkg-config
    installShellFiles
  ];

  buildInputs =
    lib.optional nativeTls openssl
    ++ lib.optional systemDbus dbus'
    ++ lib.optional systemSqlite sqlite;

  buildFeatures =
    buildFeatures
    # dbus and SQLite are provided by vendors on windows
    ++ lib.optional stdenv.hostPlatform.isWindows "vendored";

  postInstall =
    let
      exe =
        if stdenv.buildPlatform.canExecute stdenv.hostPlatform then
          "$out/bin/${finalAttrs.pname}"
        else
          lib.getExe buildPackages.${finalAttrs.pname};
    in
    ''
      mkdir -p $out/share/{completions,man,schemas,services}
      cp assets/${finalAttrs.pname}@.service "$out"/share/services/
      ${exe} completion -d "$out"/share/completions bash elvish fish powershell zsh
      ${exe} manual -d "$out"/share/man
      ${exe} json-schema -d "$out"/share/schemas
    ''
    + lib.optionalString installManPages ''
      installManPage "$out"/share/man/*
    ''
    + lib.optionalString installShellCompletions ''
      installShellCompletion --cmd ${finalAttrs.pname} \
        --bash "$out"/share/completions/${finalAttrs.pname}.bash \
        --fish "$out"/share/completions/${finalAttrs.pname}.fish \
        --zsh "$out"/share/completions/_${finalAttrs.pname}
    '';

  cargoTestFlags = [ "--bins" ];

  meta = {
    description = "CLI to watch PIM collection changes";
    mainProgram = finalAttrs.pname;
    homepage = "https://github.com/pimalaya/${finalAttrs.pname}";
    changelog = "${finalAttrs.meta.homepage}/releases/tag/${finalAttrs.src.tag}";
    license = with lib.licenses; [
      asl20
      mit
    ];
    maintainers = with lib.maintainers; [ soywod ];
  };
})
