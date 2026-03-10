#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=setup-dev-unix-common.sh
source "$SCRIPT_DIR/setup-dev-unix-common.sh"

ensure_xcode_clt() {
  if xcode-select -p >/dev/null 2>&1; then
    return
  fi

  warn "Xcode Command Line Tools are required. Triggering the installer."
  xcode-select --install || true
  die "Install the Xcode Command Line Tools, then rerun this script."
}

ensure_homebrew() {
  if command -v brew >/dev/null 2>&1; then
    return
  fi

  log "Installing Homebrew"
  NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
}

load_homebrew_env() {
  if [[ -x /opt/homebrew/bin/brew ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
  elif [[ -x /usr/local/bin/brew ]]; then
    eval "$(/usr/local/bin/brew shellenv)"
  else
    die "Homebrew was installed but brew is still not on PATH."
  fi
}

install_system_packages() {
  local missing=()

  command_exists git || missing+=("git")
  command_exists rsync || missing+=("rsync")
  command_exists pkg-config || missing+=("pkg-config")

  local node_major
  node_major="$(node_major_version || true)"
  if [[ -z "${node_major:-}" ]] || (( node_major < NODE_MAJOR_MIN )) || ! command_exists corepack; then
    missing+=("node@20")
  fi

  if ((${#missing[@]} == 0)); then
    log "macOS system dependencies already look good; skipping brew install"
    return
  fi

  log "Installing missing macOS dependencies: ${missing[*]}"
  brew update
  brew install "${missing[@]}"

  if [[ " ${missing[*]} " == *" node@20 "* ]]; then
    brew link --overwrite node@20 >/dev/null 2>&1 || true
  fi
}

main() {
  parse_unix_setup_args "$@"

  ensure_xcode_clt
  ensure_homebrew
  load_homebrew_env

  if (( ! SKIP_SYSTEM_PACKAGES )); then
    install_system_packages
  else
    log "Skipping macOS package installation"
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
