{
  description = "SlintLu: Wayland desktop shells written in the Slint language";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: rec {
        slintlu = pkgs.callPackage ./nix/package.nix { };
        default = slintlu;
      });

      overlays.default = final: _prev: { slintlu = final.callPackage ./nix/package.nix { }; };

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.slintlu ];
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            rust-analyzer
            just
            cargo-deny
            typos
            python3
            # Headless integration tests (`just headless`).
            sway
            grim
            wtype
            wl-clipboard
            libnotify
          ];
          # EGL and Vulkan are loaded at runtime.
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (
            with pkgs;
            [
              libGL
              vulkan-loader
              wayland
              libxkbcommon
            ]
          );
        };
      });

      homeManagerModules.default = import ./nix/home-manager.nix self;

      checks = forAllSystems (pkgs: {
        package = self.packages.${pkgs.stdenv.hostPlatform.system}.slintlu;
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt-rfc-style);
    };
}
