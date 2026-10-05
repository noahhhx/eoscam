{
  description = "eoscam - use a Canon EOS camera as a webcam via v4l2loopback";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

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
      packages = forAllSystems (pkgs: {
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.eoscam;
        eoscam = pkgs.rustPlatform.buildRustPackage {
          pname = "eoscam";
          version = "0.1.0";
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.rustPlatform.bindgenHook
          ];
          buildInputs = [ pkgs.libgphoto2 ];
          meta.mainProgram = "eoscam";
        };
      });

      nixosModules.default = import ./nix/module.nix self;
    };
}
