#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
META_FILE="$ROOT_DIR/desktop/src-tauri/vendor-patches/openai-api-rs/UPSTREAM.md"

if [[ ! -f "$META_FILE" ]]; then
  echo "Metadata file not found: $META_FILE" >&2
  exit 1
fi

read_meta() {
  local key="$1"
  awk -F': ' -v key="$key" '$1 == key {print $2; exit}' "$META_FILE"
}

UPSTREAM_REPO="$(read_meta upstream_repo)"
UPSTREAM_REF="$(read_meta upstream_ref)"
UPSTREAM_TAG="$(read_meta upstream_tag)"
CRATE_VERSION="$(read_meta crate_version)"

if [[ -z "$UPSTREAM_REPO" ]]; then
  echo "upstream_repo is missing in $META_FILE" >&2
  exit 1
fi

echo "Configured upstream repo: $UPSTREAM_REPO"
echo "Configured upstream ref : ${UPSTREAM_REF:-<unset>}"
echo "Configured upstream tag : ${UPSTREAM_TAG:-<unset>}"
echo "Configured crate version: ${CRATE_VERSION:-<unset>}"
echo

echo "Querying latest upstream tag..."
set +e
LATEST_LINE="$(
  git ls-remote --tags --refs --sort='-v:refname' "$UPSTREAM_REPO" 'v*' 2>/dev/null \
    | head -n1
)"
STATUS=$?
set -e

if [[ $STATUS -ne 0 || -z "$LATEST_LINE" ]]; then
  echo "Unable to query upstream tags (network/proxy/repo access issue)." >&2
  exit 2
fi

LATEST_TAG="$(awk '{print $2}' <<<"$LATEST_LINE" | awk -F'/' '{print $3}')"

if [[ -z "$LATEST_TAG" ]]; then
  echo "Could not resolve latest upstream tag."
  exit 1
fi

echo "Latest upstream tag: $LATEST_TAG"

if [[ -n "$UPSTREAM_TAG" && "$UPSTREAM_TAG" == "$LATEST_TAG" ]]; then
  echo "Status: up-to-date by tag."
else
  echo "Status: update available (by tag)."
fi
