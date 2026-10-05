self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.eoscam;
in
{
  options.services.eoscam = {
    enable = lib.mkEnableOption "the eoscam Canon EOS webcam daemon";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "eoscam.packages.\${system}.default";
      description = "The eoscam package to use.";
    };

    cardLabel = lib.mkOption {
      type = lib.types.str;
      default = "EOS Webcam";
      description = "Name of the v4l2loopback device, as shown to video apps.";
    };

    extraArgs = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [
        "--size"
        "1024x680"
      ];
      description = "Extra command line arguments passed to eoscam.";
    };
  };

  config = lib.mkIf cfg.enable {
    boot.extraModulePackages = [ config.boot.kernelPackages.v4l2loopback ];
    boot.kernelModules = [ "v4l2loopback" ];
    # exclusive_caps makes browsers (WebRTC) recognise the device as a webcam.
    boot.extraModprobeConfig = ''
      options v4l2loopback devices=1 exclusive_caps=1 card_label="${cfg.cardLabel}"
    '';

    # A user service so it can claim the camera from the user's gvfs and gets
    # the seat's device access (uaccess) for the camera.
    systemd.user.services.eoscam = {
      description = "Canon EOS camera as a webcam";
      wantedBy = [ "default.target" ];
      serviceConfig = {
        ExecStart = lib.escapeShellArgs (
          [
            (lib.getExe cfg.package)
            "--name"
            cfg.cardLabel
          ]
          ++ cfg.extraArgs
        );
        Restart = "on-failure";
        RestartSec = 5;
      };
    };
  };
}
