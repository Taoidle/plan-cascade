#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DESKTOP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$DESKTOP_DIR/.." && pwd)"

NODE_MAJOR_MIN=20
PNPM_VERSION=10

SKIP_SYSTEM_PACKAGES=0
SKIP_VENDOR_SYNC=0
SKIP_PNPM_INSTALL=0
SKIP_VERIFY=0

log() {
  printf '\n[setup] %s\n' "$*"
}

warn() {
  printf '\n[setup][warn] %s\n' "$*" >&2
}

die() {
  printf '\n[setup][error] %s\n' "$*" >&2
  exit 1
}

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

parse_unix_setup_args() {
  while (($# > 0)); do
    case "$1" in
      --skip-system-packages)
        SKIP_SYSTEM_PACKAGES=1
        ;;
      --skip-vendor-sync)
        SKIP_VENDOR_SYNC=1
        ;;
      --skip-pnpm-install)
        SKIP_PNPM_INSTALL=1
        ;;
      --skip-verify)
        SKIP_VERIFY=1
        ;;
      -h|--help)
        cat <<'EOF'
Options:
  --skip-system-packages  Skip OS package installation.
  --skip-vendor-sync      Skip vendoring/patching openai-api-rs.
  --skip-pnpm-install     Skip pnpm install.
  --skip-verify           Skip final cargo/ts verification.
  -h, --help              Show this help.
EOF
        exit 0
        ;;
      *)
        die "Unknown option: $1"
        ;;
    esac
    shift
  done
}

require_sudo() {
  if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
    SUDO=""
    return
  fi

  if command -v sudo >/dev/null 2>&1; then
    SUDO="sudo"
    return
  fi

  die "This script needs root privileges to install system packages, but sudo is not available."
}

pkg_config_has() {
  local package="$1"
  command_exists pkg-config && pkg-config --exists "$package"
}

ensure_rust() {
  if [[ ! -x "$HOME/.cargo/bin/rustup" ]] && ! command -v rustup >/dev/null 2>&1; then
    log "Installing rustup"
    curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
  fi

  export PATH="$HOME/.cargo/bin:$PATH"

  command -v rustup >/dev/null 2>&1 || die "rustup is not available after installation."

  log "Configuring Rust stable toolchain"
  rustup toolchain install stable --profile minimal >/dev/null
  rustup default stable >/dev/null
  rustup component add rustfmt clippy >/dev/null 2>&1 || true
}

node_major_version() {
  if ! command -v node >/dev/null 2>&1; then
    return 1
  fi

  node -p "process.versions.node.split('.')[0]" 2>/dev/null
}

ensure_nvm() {
  export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"

  if [[ ! -s "$NVM_DIR/nvm.sh" ]]; then
    log "Installing nvm"
    curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
  fi

  # shellcheck disable=SC1090
  source "$NVM_DIR/nvm.sh"
}

ensure_node_via_nvm() {
  local major
  major="$(node_major_version || true)"
  if [[ -n "${major:-}" ]] && (( major >= NODE_MAJOR_MIN )); then
    log "Node.js $major already satisfies the requirement"
    return
  fi

  ensure_nvm
  log "Installing Node.js ${NODE_MAJOR_MIN} via nvm"
  nvm install "$NODE_MAJOR_MIN"
  nvm alias default "$NODE_MAJOR_MIN" >/dev/null
  nvm use "$NODE_MAJOR_MIN" >/dev/null
}

ensure_corepack_pnpm() {
  command -v node >/dev/null 2>&1 || die "Node.js is required before enabling pnpm."
  command -v corepack >/dev/null 2>&1 || die "corepack is not available. Install Node.js 20+ first."

  log "Enabling corepack and pnpm@$PNPM_VERSION"
  corepack enable
  corepack prepare "pnpm@${PNPM_VERSION}" --activate
  command -v pnpm >/dev/null 2>&1 || die "pnpm is not available after corepack activation."
}

sync_vendor_patch_queue() {
  if ((SKIP_VENDOR_SYNC)); then
    log "Skipping vendored openai-api-rs sync"
    return
  fi

  log "Syncing vendored openai-api-rs and applying local patches"
  bash "$REPO_ROOT/scripts/vendor-openai-api-rs-sync.sh"
}

install_frontend_deps() {
  if ((SKIP_PNPM_INSTALL)); then
    log "Skipping pnpm install"
    return
  fi

  log "Installing desktop frontend dependencies"
  (cd "$DESKTOP_DIR" && pnpm install --frozen-lockfile)
}

verify_desktop_env() {
  if ((SKIP_VERIFY)); then
    log "Skipping verification"
    return
  fi

  log "Running cargo check"
  cargo check --manifest-path "$DESKTOP_DIR/src-tauri/Cargo.toml" --lib

  log "Running TypeScript check"
  pnpm -C "$DESKTOP_DIR" exec tsc --noEmit
}

print_success_message() {
  cat <<EOF

[setup] Desktop development environment is ready.

Next steps:
  cd "$DESKTOP_DIR"
  pnpm tauri:dev

Optional checks:
  pnpm -C "$DESKTOP_DIR" test
  cargo test --manifest-path "$DESKTOP_DIR/src-tauri/Cargo.toml" --lib
EOF
}
