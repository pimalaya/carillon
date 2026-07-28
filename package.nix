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

  pname = "carillon";
  version = "0.0.0";
  cargoHash = "";

  src = fetchFromGitHub {
    owner = "pimalaya";
    repo = "carillon";
    rev = "v0.0.0";
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
      mkdir -p $out/share/{completions,man}
      ${emulator} "$out"/bin/carillon${exe} manuals "$out"/share/man
      ${emulator} "$out"/bin/carillon${exe} completions -d "$out"/share/completions bash elvish fish powershell zsh
    ''
    + lib.optionalString installManPages ''
      installManPage "$out"/share/man/*
    ''
    + lib.optionalString installShellCompletions ''
      installShellCompletion --cmd carillon \
        --bash "$out"/share/completions/carillon.bash \
        --fish "$out"/share/completions/carillon.fish \
        --zsh "$out"/share/completions/_carillon
    '';

  meta = {
    description = "CLI daemon to watch PIM account changes";
    mainProgram = "carillon";
    homepage = "https://github.com/pimalaya/carillon";
    changelog = "https://github.com/pimalaya/carillon/blob/master/CHANGELOG.md";
    license = with lib.licenses; [ mit asl20 ];
    maintainers = with lib.maintainers; [ soywod ];
  };
}
