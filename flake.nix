{
  description = "OBS controller using AKAI LPD8";
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { fenix, nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        f = fenix.packages.${system};
      in
      {
        devShells.default = pkgs.mkShell rec {
          name = "akai-lpd8-obs";
          packages = [
            f.stable.toolchain
          ];

          nativeBuildInputs = [
            pkgs.pkg-config
          ];
          buildInputs = [
            pkgs.alsa-lib-with-plugins
          ];
          LD_LIBRARY_PATH = nixpkgs.lib.makeLibraryPath buildInputs;
        };
      });
}
