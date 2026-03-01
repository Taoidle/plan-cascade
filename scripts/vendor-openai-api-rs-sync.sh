#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
META_FILE="$ROOT_DIR/desktop/src-tauri/vendor-patches/openai-api-rs/UPSTREAM.md"
VENDOR_DIR="$ROOT_DIR/desktop/src-tauri/vendor/openai-api-rs"
PATCH_DIR="$ROOT_DIR/desktop/src-tauri/vendor-patches/openai-api-rs"

usage() {
  cat <<'EOF'
Usage: vendor-openai-api-rs-sync.sh [options]

Sync vendored openai-api-rs from upstream and optionally re-apply local patches.

Options:
  --repo <url>      Override upstream repository URL.
  --ref <ref>       Override upstream ref (tag/branch/commit).
  --no-patches      Do not apply local patch files after sync.
  --verify          Run llm/desktop compile checks after sync.
  -h, --help        Show this help.
EOF
}

read_meta() {
  local key="$1"
  awk -F': ' -v key="$key" '$1 == key {print $2; exit}' "$META_FILE"
}

if [[ ! -f "$META_FILE" ]]; then
  echo "Metadata file not found: $META_FILE" >&2
  exit 1
fi

UPSTREAM_REPO="$(read_meta upstream_repo)"
UPSTREAM_REF="$(read_meta upstream_ref)"
APPLY_PATCHES=1
RUN_VERIFY=0

while (($# > 0)); do
  case "$1" in
    --repo)
      UPSTREAM_REPO="${2:-}"
      shift 2
      ;;
    --ref)
      UPSTREAM_REF="${2:-}"
      shift 2
      ;;
    --no-patches)
      APPLY_PATCHES=0
      shift
      ;;
    --verify)
      RUN_VERIFY=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -z "$UPSTREAM_REPO" || -z "$UPSTREAM_REF" ]]; then
  echo "upstream_repo/upstream_ref must be set (metadata or args)." >&2
  exit 1
fi

for cmd in git rsync patch; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Required command not found: $cmd" >&2
    exit 1
  fi
done

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/openai-api-rs-sync.XXXXXX")"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

echo "Cloning upstream: $UPSTREAM_REPO"
git clone --filter=blob:none "$UPSTREAM_REPO" "$TMP_DIR/upstream" >/dev/null 2>&1
git -C "$TMP_DIR/upstream" checkout --quiet "$UPSTREAM_REF"
RESOLVED_REF="$(git -C "$TMP_DIR/upstream" rev-parse HEAD)"

echo "Syncing vendor directory: $VENDOR_DIR"
mkdir -p "$VENDOR_DIR"
rsync -a --delete --exclude '.git' "$TMP_DIR/upstream/" "$VENDOR_DIR/"

if ((APPLY_PATCHES)); then
  shopt -s nullglob
  patches=("$PATCH_DIR"/*.patch)
  shopt -u nullglob

  if ((${#patches[@]} == 0)); then
    echo "No patch files found in $PATCH_DIR"
  else
    for patch in "${patches[@]}"; do
      echo "Applying patch: $(basename "$patch")"
      patch -p0 -d "$VENDOR_DIR" <"$patch"
    done
  fi
fi

echo "Sync complete."
echo "Resolved upstream ref: $RESOLVED_REF"
echo "If needed, update metadata in: $META_FILE"

if ((RUN_VERIFY)); then
  echo "Running verification checks..."
  (
    cd "$ROOT_DIR/desktop/src-tauri"
    cargo test -p plan-cascade-llm --lib
    cargo check -p plan-cascade-desktop --lib
  )
fi
