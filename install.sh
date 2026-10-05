#!/bin/sh
set -eu

# modinfo lives in sbin on Debian-based distros, which may not be on a user's PATH.
PATH="$PATH:/usr/sbin:/sbin"

REPO=noahhhx/eoscam
BIN=/usr/local/bin/eoscam
UNIT=/etc/systemd/user/eoscam.service
MODPROBE_CONF=/etc/modprobe.d/eoscam.conf
MODULES_LOAD_CONF=/etc/modules-load.d/eoscam.conf
CARD_LABEL="EOS Webcam"

say() { printf 'eoscam: %s\n' "$*"; }
die() {
    say "$*" >&2
    exit 1
}

if [ "$(id -u)" -eq 0 ]; then SUDO=; else SUDO=sudo; fi

user_systemctl() {
    if [ "$(id -u)" -ne 0 ]; then
        systemctl --user "$@"
    elif [ -n "${SUDO_USER:-}" ]; then
        systemctl --user -M "$SUDO_USER@" "$@"
    else
        return 1
    fi
}

fetch_binary() {
    tmp=$(mktemp)
    trap 'rm -f "$tmp"' EXIT
    if [ -n "${EOSCAM_BINARY:-}" ]; then
        cp "$EOSCAM_BINARY" "$tmp"
    else
        [ "$(uname -m)" = x86_64 ] || die "only x86_64 builds are published (this is $(uname -m))"
        url="https://github.com/$REPO/releases/latest/download/eoscam-x86_64-linux"
        say "downloading $url"
        curl -fsSL -o "$tmp" "$url"
    fi
    chmod 755 "$tmp"
}

check_requirements() {
    missing=
    if ldd "$tmp" | grep -q "not found"; then
        missing="$missing
  - libgphoto2 ($(ldd "$tmp" | awk '/not found/ {print $1}' | tr '\n' ' ' | sed 's/ $//') not found)"
    fi
    if [ ! -d /sys/module/v4l2loopback ] && ! modinfo v4l2loopback >/dev/null 2>&1; then
        missing="$missing
  - the v4l2loopback kernel module"
    fi
    [ -z "$missing" ] && return 0

    cat >&2 <<EOF
eoscam: missing requirements:$missing

Install them with your package manager, then run this script again:
  Ubuntu/Debian: sudo apt install libgphoto2-6t64 v4l2loopback-dkms
                 (libgphoto2-6 instead of libgphoto2-6t64 on Ubuntu 22.04 / Debian 12)
  Arch:          sudo pacman -S libgphoto2 v4l2loopback-dkms linux-headers
                 (the headers for your kernel, e.g. linux-lts-headers)
  Fedora:        sudo dnf install libgphoto2 v4l2loopback
                 (v4l2loopback comes from RPM Fusion)
EOF
    exit 1
}

install_binary() {
    $SUDO install -Dm755 "$tmp" "$BIN"
}

loopback_has_card_label() {
    grep -qx "$CARD_LABEL" /sys/class/video4linux/*/name 2>/dev/null
}

setup_module() {
    printf '# exclusive_caps makes browsers (WebRTC) recognise the device as a webcam.\noptions v4l2loopback devices=1 exclusive_caps=1 card_label="%s"\n' "$CARD_LABEL" |
        $SUDO tee "$MODPROBE_CONF" >/dev/null
    echo v4l2loopback | $SUDO tee "$MODULES_LOAD_CONF" >/dev/null

    if ! loopback_has_card_label; then
        $SUDO modprobe -r v4l2loopback 2>/dev/null || true
        $SUDO modprobe v4l2loopback ||
            say "could not load v4l2loopback now. It loads at the next boot."
    fi
}

setup_service() {
    $SUDO tee "$UNIT" >/dev/null <<EOF
[Unit]
Description=Canon EOS camera as a webcam

[Service]
ExecStart=$BIN
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF
    $SUDO systemctl --global enable eoscam.service
    if user_systemctl daemon-reload && user_systemctl restart eoscam.service; then
        say "service running"
    else
        say "service enabled. It starts at your next login."
    fi
}

uninstall() {
    user_systemctl stop eoscam.service 2>/dev/null || true
    $SUDO systemctl --global disable eoscam.service 2>/dev/null || true
    $SUDO rm -f "$UNIT" "$BIN" "$MODPROBE_CONF" "$MODULES_LOAD_CONF"
    user_systemctl daemon-reload 2>/dev/null || true
    say "uninstalled (libgphoto2 and v4l2loopback packages were left installed)"
}

[ -e /etc/NIXOS ] && die "on NixOS use the flake's NixOS module instead (see README)"
command -v systemctl >/dev/null || die "eoscam needs systemd"

case "${1:-}" in
    --uninstall)
        uninstall
        exit 0
        ;;
    "") ;;
    *) die "unknown argument $1 (only --uninstall is supported)" ;;
esac

fetch_binary
check_requirements
install_binary
setup_module
setup_service
say "installed. Turn on the camera, then watch the log: journalctl --user -u eoscam -f"
