# eoscam

Use a Canon EOS camera as a webcam on Linux. eoscam is written for the EOS
550D. Other Canon models with liveview may need `--size`.

eoscam is a daemon that writes frames to a v4l2loopback device. Video apps see
that device as a webcam named "EOS Webcam".

- While no camera is connected, eoscam writes black frames. The webcam stays
  available to apps.
- When you plug in or switch on the camera, eoscam claims it within a few
  seconds and streams its liveview. Before eoscam claims the camera, it stops
  gvfs's gphoto2 processes, because they hold the camera.
- When you switch off or unplug the camera, eoscam goes back to black frames
  and waits for the camera. Apps that use the webcam keep their stream.

eoscam reads the camera through libgphoto2 and writes to the v4l2loopback
device itself. You do not need `gphoto2` or `ffmpeg`.

## Install on NixOS

1. Add the flake to your inputs:

   ```nix
   eoscam.url = "github:noahhhx/eoscam";
   ```

2. Import the module and enable the service:

   ```nix
   { inputs, ... }: {
     imports = [ inputs.eoscam.nixosModules.default ];
     services.eoscam.enable = true;
   }
   ```

3. Rebuild your system.

The module loads v4l2loopback with `card_label="EOS Webcam" exclusive_caps=1`.
It runs eoscam as a systemd user service.

## Install on other systemd distros

The installer supports Ubuntu, Debian, Arch, Fedora, and other distros that use
systemd. Before you start, check these requirements:

- Your machine is x86_64. The release has no other builds.
- Your distro has glibc 2.34 or later: Ubuntu 22.04+, Debian 12+, Arch, or
  Fedora 35+.
- If Secure Boot is on, DKMS asks you to enroll a signing key (MOK) the first
  time it builds v4l2loopback. Follow its prompt and reboot.

1. Install libgphoto2 and v4l2loopback:

   | Distro                  | Packages                                                      |
   | ----------------------- | ------------------------------------------------------------- |
   | Ubuntu 24.04+           | `libgphoto2-6t64 v4l2loopback-dkms`                           |
   | Ubuntu 22.04, Debian 12 | `libgphoto2-6 v4l2loopback-dkms`                              |
   | Arch                    | `libgphoto2 v4l2loopback-dkms`, and headers for your kernel   |
   | Fedora                  | `libgphoto2 v4l2loopback` from RPM Fusion                     |

   On Arch, the headers package for the default kernel is `linux-headers`.

2. Run the installer:

   ```sh
   curl -fsSL https://raw.githubusercontent.com/noahhhx/eoscam/main/install.sh | sh
   ```

The installer does these steps:

1. Downloads the binary from the latest GitHub release.
2. Checks for libgphoto2 and v4l2loopback. If either is missing, it prints the
   packages to install and stops. It does not change your system.
3. Installs the binary to `/usr/local/bin/eoscam`.
4. Configures v4l2loopback to load at boot, and loads it now.
5. Enables eoscam as a systemd user service for all users.

To update eoscam, run the installer again.

To uninstall eoscam, run:

```sh
curl -fsSL https://raw.githubusercontent.com/noahhhx/eoscam/main/install.sh | sh -s -- --uninstall
```

The uninstaller leaves the libgphoto2 and v4l2loopback packages installed.

## Check that it works

Turn on the camera and watch the log:

```sh
journalctl --user -u eoscam -f
```

When eoscam finds the camera, it logs `camera connected, streaming`. Open
"EOS Webcam" in a video app to see the liveview.

## Options

```
--name NAME     v4l2loopback card_label to write to [default: EOS Webcam]
--device PATH   write to this device instead of looking it up by name
--size WxH      output resolution [default: 1056x704, the 550D liveview size]
```

If the camera's liveview size differs from `--size`, eoscam logs the correct
`--size` value. Until you change it, eoscam crops or pads each frame to fit.

On NixOS, set options with `services.eoscam.extraArgs` and
`services.eoscam.cardLabel`.

## Development

The dev environment uses [devenv](https://devenv.sh). It provides stable Rust,
libgphoto2, libclang for bindgen, and a clippy pre-commit hook. If you use
direnv, run `direnv allow` once. Otherwise, run `devenv shell`.

```sh
cargo test
cargo run
```

To test `install.sh` with a local build instead of the latest release, set
`EOSCAM_BINARY`:

```sh
cargo build --release
EOSCAM_BINARY=target/release/eoscam sh install.sh
```

`flake.nix` only builds the package and the NixOS module. It does not define
the dev environment.

## Release

1. Push a version tag:

   ```sh
   git tag v0.1.0 && git push --tags
   ```

2. The release workflow builds the binary on Ubuntu 22.04. It attaches the
   binary to a GitHub release as `eoscam-x86_64-linux`. `install.sh` downloads
   this file.
