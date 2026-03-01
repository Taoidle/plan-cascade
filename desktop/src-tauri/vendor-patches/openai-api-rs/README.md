# openai-api-rs Patch Queue

This directory stores local patch files that are applied on top of the vendored
`desktop/src-tauri/vendor/openai-api-rs` baseline.

Metadata for upstream tracking lives in:
- `desktop/src-tauri/vendor-patches/openai-api-rs/UPSTREAM.md`

## Conventions

- Name patches in apply order, e.g.:
  - `0001-*.patch`
  - `0002-*.patch`
- Keep each patch focused on one topic (stream parsing, request headers, etc.).
- Patch paths are relative to the `openai-api-rs` root and applied with `patch -p0`.

## Common Commands

- Sync vendor and apply all patches:
  - `./scripts/vendor-openai-api-rs-sync.sh --verify`
- Export current vendor delta vs configured upstream into a patch:
  - `./scripts/vendor-openai-api-rs-export-patch.sh --output desktop/src-tauri/vendor-patches/openai-api-rs/0001-local.patch`
- Check latest upstream tag quickly:
  - `./scripts/vendor-openai-api-rs-check-upstream.sh`
