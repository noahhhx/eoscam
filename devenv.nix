{ pkgs, ... }:

{
  languages.rust = {
    enable = true;
    channel = "stable";
  };

  packages = [
    pkgs.pkg-config
    pkgs.libgphoto2
    # libgphoto2_sys generates its bindings with bindgen, which needs libclang
    # and the C headers' include paths.
    pkgs.rustPlatform.bindgenHook
  ];

  git-hooks.hooks = {
    clippy.enable = true;
  };
}
