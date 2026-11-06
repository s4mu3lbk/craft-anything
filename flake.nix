{
  inputs = {
    nixpkgs.url = "github:cachix/devenv-nixpkgs/rolling";
    systems.url = "github:nix-systems/default";
    devenv.url = "github:cachix/devenv";
    devenv.inputs.nixpkgs.follows = "nixpkgs";
  };

  nixConfig = {
    extra-trusted-public-keys = "devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=";
    extra-substituters = "https://devenv.cachix.org";
  };

  outputs =
    {
      self,
      nixpkgs,
      devenv,
      systems,
      ...
    }@inputs:
    let
      forEachSystem = nixpkgs.lib.genAttrs (import systems);
      runtimeLibs =
        pkgs:
        with pkgs;
        [
          wayland
          libxkbcommon
          libGL
          vulkan-loader
          libx11
          libxcb
          libxcursor
          libxi
          libxrandr
          libxext
          libxfixes
          libxdamage
          libxcomposite
          libxrender
          libxtst
          libxinerama
          libxxf86vm
        ];
    in
    {
      packages = forEachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          libs = runtimeLibs pkgs;
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "craft-anything";
            version = "0.4.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [
              "-p"
              "craft-anything"
              "-p"
              "craft-anything-cli"
            ];
            nativeBuildInputs = [
              pkgs.pkg-config
              pkgs.makeWrapper
            ];
            buildInputs = libs;
            doCheck = false;
            postInstall = ''
              mkdir -p $out/bin
              cp target/${pkgs.stdenv.hostPlatform.rust.rustcTarget}/release/craft-anything $out/bin/
              cp target/${pkgs.stdenv.hostPlatform.rust.rustcTarget}/release/craft-anything-cli $out/bin/
              wrapProgram $out/bin/craft-anything \
                --prefix LD_LIBRARY_PATH : ${nixpkgs.lib.makeLibraryPath libs}
            '';
            meta = {
              description = "Evidence-backed software analysis and rebuilding";
              mainProgram = "craft-anything";
              license = nixpkgs.lib.licenses.mit;
              platforms = nixpkgs.lib.platforms.linux;
            };
          };
        }
      );

      devShells = forEachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          libs = runtimeLibs pkgs;
        in
        {
          default = devenv.lib.mkShell {
            inherit inputs pkgs;
            modules = [
              {
                packages = [ pkgs.pkg-config ] ++ libs;
                env.LD_LIBRARY_PATH = nixpkgs.lib.makeLibraryPath libs;
                languages.rust.enable = true;
              }
            ];
          };
        }
      );
    };
}
