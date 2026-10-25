{
  lib,
  rustPlatform,
  pkg-config,
  makeWrapper,
  wayland,
  libxkbcommon,
  fontconfig,
  freetype,
  pam,
  libGL,
  vulkan-loader,
}:

let
  manifest = (lib.importTOML ../Cargo.toml).package;
in
rustPlatform.buildRustPackage {
  pname = manifest.name;
  inherit (manifest) version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../build.rs
      ../res
      ../src
      ../examples
      ../tests
    ];
  };
  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];
  buildInputs = [
    wayland
    libxkbcommon
    fontconfig
    freetype
    pam
  ];

  # Tests touching the machine (/proc, /sys, PAM) work in the sandbox; the CLI tests need a
  # writable data directory.
  preCheck = ''
    export HOME=$TMPDIR
  '';

  # libEGL and libvulkan are opened at runtime; make the loaders findable.
  postFixup = ''
    wrapProgram $out/bin/slintlu \
      --prefix LD_LIBRARY_PATH : ${
        lib.makeLibraryPath [
          libGL
          vulkan-loader
          wayland
          libxkbcommon
        ]
      }
  '';

  meta = {
    description = "Wayland desktop shells written in the Slint language";
    homepage = "https://github.com/LunaDash-DE/SlintLu";
    license = lib.licenses.gpl3Only;
    mainProgram = "slintlu";
    platforms = lib.platforms.linux;
  };
}
