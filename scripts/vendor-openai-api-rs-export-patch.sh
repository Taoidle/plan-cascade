#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
META_FILE="$ROOT_DIR/desktop/src-tauri/vendor-patches/openai-api-rs/UPSTREAM.md"
VENDOR_DIR="$ROOT_DIR/desktop/src-tauri/vendor/openai-api-rs"
PATCH_DIR="$ROOT_DIR/desktop/src-tauri/vendor-patches/openai-api-rs"

usage() {
  cat <<'EOF'
Usage: vendor-openai-api-rs-export-patch.sh [options]

Export patch (vendor delta vs configured upstream ref) to a .patch file.

Options:
  --repo <url>       Override upstream repository URL.
  --ref <ref>        Override upstream ref (tag/branch/commit).
  --baseline-dir <path>  Use local baseline directory instead of cloning upstream.
  --output <path>    Output patch file path.
  -h, --help         Show this help.
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
CRATE_VERSION="$(read_meta crate_version)"
BASELINE_DIR=""
OUTPUT_PATH="$PATCH_DIR/0001-local.patch"

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
    --output)
      OUTPUT_PATH="${2:-}"
      shift 2
      ;;
    --baseline-dir)
      BASELINE_DIR="${2:-}"
      shift 2
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

if [[ "$OUTPUT_PATH" != /* ]]; then
  OUTPUT_PATH="$ROOT_DIR/$OUTPUT_PATH"
fi

mkdir -p "$(dirname "$OUTPUT_PATH")"

if ! command -v git >/dev/null 2>&1; then
  echo "Required command not found: git" >&2
  exit 1
fi
if ! command -v rsync >/dev/null 2>&1; then
  echo "Required command not found: rsync" >&2
  exit 1
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/openai-api-rs-export.XXXXXX")"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

prepare_baseline_repo() {
  local src_dir="$1"
  mkdir -p "$TMP_DIR/upstream"
  rsync -a --delete --exclude '.git' "$src_dir/" "$TMP_DIR/upstream/"
  git -C "$TMP_DIR/upstream" init -q
  git -C "$TMP_DIR/upstream" config user.name "vendor-export"
  git -C "$TMP_DIR/upstream" config user.email "vendor-export@example.invalid"
  git -C "$TMP_DIR/upstream" add -A
  git -C "$TMP_DIR/upstream" -c commit.gpgsign=false commit -q -m "baseline"
}

if [[ -n "$BASELINE_DIR" ]]; then
  if [[ "$BASELINE_DIR" != /* ]]; then
    BASELINE_DIR="$ROOT_DIR/$BASELINE_DIR"
  fi
  if [[ ! -d "$BASELINE_DIR" ]]; then
    echo "Baseline dir not found: $BASELINE_DIR" >&2
    exit 1
  fi
  echo "Using local baseline dir: $BASELINE_DIR"
  prepare_baseline_repo "$BASELINE_DIR"
else
  echo "Cloning upstream baseline: $UPSTREAM_REPO @ $UPSTREAM_REF"
  if git clone --filter=blob:none "$UPSTREAM_REPO" "$TMP_DIR/upstream" >/dev/null 2>&1 \
    && git -C "$TMP_DIR/upstream" checkout --quiet "$UPSTREAM_REF"; then
    :
  else
    CARGO_BASELINE=""
    if [[ -n "${CRATE_VERSION:-}" ]]; then
      CARGO_BASELINE="$(
        find "$HOME/.cargo/registry/src" -maxdepth 2 -type d -name "openai-api-rs-$CRATE_VERSION" 2>/dev/null | head -n1
      )"
    fi
    if [[ -n "$CARGO_BASELINE" && -d "$CARGO_BASELINE" ]]; then
      echo "Upstream clone failed; falling back to cargo cache baseline: $CARGO_BASELINE"
      rm -rf "$TMP_DIR/upstream"
      prepare_baseline_repo "$CARGO_BASELINE"
    else
      echo "Unable to get upstream baseline (clone failed and no cargo cache fallback)." >&2
      exit 2
    fi
  fi
fi

echo "Overlaying current vendor tree for diff: $VENDOR_DIR"
if [[ ! -d "$VENDOR_DIR" ]]; then
  echo "Vendor dir not found: $VENDOR_DIR" >&2
  exit 1
fi
rsync -a --delete --exclude '.git' "$VENDOR_DIR/" "$TMP_DIR/upstream/"

git -C "$TMP_DIR/upstream" add -A
git -C "$TMP_DIR/upstream" diff --binary --cached --no-prefix >"$OUTPUT_PATH"

if [[ ! -s "$OUTPUT_PATH" ]]; then
  rm -f "$OUTPUT_PATH"
  echo "No local vendor delta found; patch not written."
  exit 0
fi

echo "Patch exported: $OUTPUT_PATH"
