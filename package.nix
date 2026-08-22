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
  stdenv,
}:

let
  nativeTls = builtins.elem "native-tls" buildFeatures;
  # notify-rust is not behind a cargo feature here: a watch that cannot notify
  # is not the tool, so the system dbus is linked in every build.
  systemDbus = !stdenv.hostPlatform.isWindows;

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
  version = "0.1.0";
  cargoHash = "";

  src = fetchFromGitHub {
    owner = "pimalaya";
    repo = finalAttrs.pname;
    tag = "v${finalAttrs.version}";
    hash = "";
  };

  env = {
    # pkg-config hands the linker libdbus but no rpath, leaving a binary that
    # cannot find it: not in postInstall, which runs it, nor once installed.
    NIX_LDFLAGS = lib.optionalString systemDbus ("-rpath " + lib.getLib dbus' + "/lib");

    # openssl should not be provided by vendors, not even on windows
    OPENSSL_NO_VENDOR = 1;
  };

  nativeBuildInputs = [
    pkg-config
    installShellFiles
  ];

  buildInputs =
    lib.optional nativeTls openssl
    # dbus is provided by vendors on windows
    ++ lib.optional systemDbus dbus';

  buildFeatures =
    buildFeatures
    # dbus is provided by vendors on windows
    ++ lib.optional stdenv.hostPlatform.isWindows "vendored";

  postInstall =
    let
      exe =
        if stdenv.buildPlatform.canExecute stdenv.hostPlatform then
          "$out/bin/${finalAttrs.meta.mainProgram}"
        else
          lib.getExe buildPackages.${finalAttrs.pname};
    in
    ''
      mkdir -p $out/share/{completions,man,services}
      cp assets/${finalAttrs.pname}@.service "$out"/share/services/
      ${exe} manuals "$out"/share/man
      ${exe} completions -d "$out"/share/completions bash elvish fish powershell zsh
    ''
    + lib.optionalString installManPages ''
      installManPage "$out"/share/man/*
    ''
    + lib.optionalString installShellCompletions ''
      installShellCompletion --cmd ${finalAttrs.meta.mainProgram} \
        --bash "$out"/share/completions/${finalAttrs.meta.mainProgram}.bash \
        --fish "$out"/share/completions/${finalAttrs.meta.mainProgram}.fish \
        --zsh "$out"/share/completions/_${finalAttrs.meta.mainProgram}
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
