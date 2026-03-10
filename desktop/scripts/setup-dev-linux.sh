#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=setup-dev-unix-common.sh
source "$SCRIPT_DIR/setup-dev-unix-common.sh"

detect_linux_pkg_manager() {
  local manager
  for manager in apt-get dnf yum pacman zypper; do
    if command -v "$manager" >/dev/null 2>&1; then
      printf '%s\n' "$manager"
      return 0
    fi
  done

  return 1
}

install_system_packages() {
  local manager
  manager="$(detect_linux_pkg_manager)" || die "Unsupported Linux distribution. Install system packages manually, then rerun with --skip-system-packages."
  require_sudo

  log "Installing Linux system dependencies with $manager"

  case "$manager" in
    apt-get)
      $SUDO apt-get update
      $SUDO apt-get install -y \
        build-essential curl file git libayatana-appindicator3-dev libgtk-3-dev \
        libjavascriptcoregtk-4.1-dev librsvg2-dev libssl-dev libwebkit2gtk-4.1-dev \
        patch patchelf pkg-config rsync
      ;;
    dnf)
      if ! $SUDO dnf install -y \
        gcc gcc-c++ make curl file git libayatana-appindicator-gtk3-devel \
        gtk3-devel openssl-devel patch patchelf pkgconf-pkg-config rsync \
        librsvg2-devel webkit2gtk4.1-devel; then
        $SUDO dnf install -y \
          gcc gcc-c++ make curl file git libappindicator-gtk3-devel \
          gtk3-devel openssl-devel patch patchelf pkgconf-pkg-config rsync \
          librsvg2-devel webkit2gtk4.1-devel
      fi
      ;;
    yum)
      if ! $SUDO yum install -y \
        gcc gcc-c++ make curl file git libayatana-appindicator-gtk3-devel \
        gtk3-devel openssl-devel patch patchelf pkgconfig rsync \
        librsvg2-devel webkit2gtk4.1-devel; then
        $SUDO yum install -y \
          gcc gcc-c++ make curl file git libappindicator-gtk3-devel \
          gtk3-devel openssl-devel patch patchelf pkgconfig rsync \
          librsvg2-devel webkit2gtk4.1-devel
      fi
      ;;
    pacman)
      $SUDO pacman -Sy --needed --noconfirm \
        base-devel curl file git gtk3 libayatana-appindicator openssl patch \
        patchelf pkgconf rsync librsvg webkit2gtk-4.1
      ;;
    zypper)
      $SUDO zypper --non-interactive install \
        gcc gcc-c++ make curl file git gtk3-devel libayatana-appindicator3-devel \
        libopenssl-devel librsvg-devel patch patchelf pkg-config rsync \
        webkit2gtk3-devel
      ;;
  esac
}

main() {
  parse_unix_setup_args "$@"

  if (( ! SKIP_SYSTEM_PACKAGES )); then
    install_system_packages
  else
    log "Skipping Linux package installation"
  fi

  ensure_node_via_nvm
  ensure_rust
  ensure_corepack_pnpm
  sync_vendor_patch_queue
  install_frontend_deps
  verify_desktop_env
  print_success_message
}

main "$@"
