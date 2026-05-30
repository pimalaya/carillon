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
  inherit (stdenv.hostPlatform)
    isLinux
    isWindows
    isAarch64
    ;

  emulator = stdenv.hostPlatform.emulator buildPackages;
  exe = stdenv.hostPlatform.extensions.executable;

  dbus' = dbus.overrideAttrs (old: {
    env = (old.env or { }) // {
      NIX_CFLAGS_COMPILE =
        (old.env.NIX_CFLAGS_COMPILE or "")
        # required for D-Bus on Linux AArch64
        + lib.optionalString (isLinux && isAarch64) " -mno-outline-atomics";
    };
  });

in
rustPlatform.buildRustPackage {
  inherit buildNoDefaultFeatures;

  pname = "mirador";
  version = "0.1.0";
  cargoHash = "";

  src = fetchFromGitHub {
    owner = "pimalaya";
    repo = "mirador";
    rev = "v0.1.0";
    hash = "";
  };

  env.OPENSSL_NO_VENDOR = true;

  nativeBuildInputs = [
    pkg-config
    installShellFiles
  ];

  buildInputs =
    # D-Bus is provided by vendors on Windows
    lib.optional (!isWindows) dbus' ++ lib.optional (builtins.elem "native-tls" buildFeatures) openssl;

  buildFeatures = buildFeatures ++ lib.optional isWindows "vendored";

  # most of the tests are lib side
  doCheck = false;

  postInstall =
    lib.optionalString (lib.hasInfix "wine" emulator) ''
      export WINEPREFIX="''${WINEPREFIX:-$(mktemp -d)}"
      mkdir -p $WINEPREFIX
    ''
    + ''
      mkdir -p $out/share/{completions,man,services}
      cp assets/mirador@.service "$out"/share/services/
      ${emulator} "$out"/bin/mirador${exe} manuals "$out"/share/man
      ${emulator} "$out"/bin/mirador${exe} completions -d "$out"/share/completions bash elvish fish powershell zsh
    ''
    + lib.optionalString installManPages ''
      installManPage "$out"/share/man/*
    ''
    + lib.optionalString installShellCompletions ''
      installShellCompletion --cmd mirador \
        --bash "$out"/share/completions/mirador.bash \
        --fish "$out"/share/completions/mirador.fish \
        --zsh "$out"/share/completions/_mirador
    '';

  meta = {
    description = "CLI to watch mailbox changes";
    mainProgram = "mirador";
    homepage = "https://github.com/pimalaya/mirador";
    changelog = "https://github.com/pimalaya/mirador/blob/master/CHANGELOG.md";
    license = lib.licenses.agpl3Only;
    maintainers = with lib.maintainers; [ soywod ];
  };
}
