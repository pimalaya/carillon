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
  emulator = stdenv.hostPlatform.emulator buildPackages;
  exe = stdenv.hostPlatform.extensions.executable;

in
rustPlatform.buildRustPackage {
  inherit buildNoDefaultFeatures buildFeatures;

  pname = "mirador";
  version = "2.0.0-rc";
  cargoHash = "";

  src = fetchFromGitHub {
    owner = "pimalaya";
    repo = "mirador";
    rev = "v2.0.0-rc";
    hash = "";
  };

  env.OPENSSL_NO_VENDOR = true;

  nativeBuildInputs = [
    pkg-config
    installShellFiles
  ];

  buildInputs = [
    dbus
  ]
  ++ lib.optional (builtins.elem "native-tls" buildFeatures) openssl;

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
    changelog = "https://github.com/pimalaya/mirador/blob/v2.0.0-rc/CHANGELOG.md";
    license = lib.licenses.agpl3Only;
    maintainers = with lib.maintainers; [ soywod ];
  };
}
