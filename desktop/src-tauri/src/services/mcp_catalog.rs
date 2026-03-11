//! MCP Catalog Service
//!
//! Provides a signed local catalog for MCP server recommendations with cache support.

use base64::Engine;
use chrono::{Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reqwest::header::{ETAG, IF_NONE_MATCH};
use sha2::{Digest, Sha256};
use std::time::Duration as StdDuration;

use crate::models::{
    McpCatalogFilter, McpCatalogItem, McpCatalogListResponse, McpCatalogRefreshResult,
    McpCatalogTrustLevel, McpInstallStrategy, McpInstallStrategyKind, McpInstallVerification,
    McpRuntimeKind, McpSecretSchemaField, RuntimeRequirement,
};
use crate::storage::database::Database;
use crate::utils::error::{AppError, AppResult};

const BUILTIN_CATALOG_SOURCE_ID: &str = "builtin:mcp-catalog-v1";
const BUILTIN_CATALOG_SOURCE_LABEL: &str = "builtin";
const REMOTE_CATALOG_SOURCE_ID: &str = "remote:mcp-catalog-v1";
const REMOTE_CATALOG_SOURCE_LABEL: &str = "remote";
const CATALOG_REMOTE_URL_ENV: &str = "PLAN_CASCADE_MCP_CATALOG_URL";
const CATALOG_REMOTE_PUBKEY_ENV: &str = "PLAN_CASCADE_MCP_CATALOG_PUBKEY_HEX";
const CATALOG_SIGNATURE_HEADER: &str = "x-plan-cascade-signature";
const CATALOG_SIGNATURE_HEADER_FALLBACK: &str = "x-signature";
const CACHE_TTL_HOURS: i64 = 24;
const HTTP_TIMEOUT_SECS: u64 = 12;
/// Trusted public key for remote catalog signature verification.
/// This key can be rotated in future catalog versions.
const CATALOG_SIGNING_PUBLIC_KEY_HEX: &str =
    "f307de0f7b5cc5f35d1bbf2ca3f7176316eb95fab2f693db39f8be9aa5c9f971";

/// MCP recommendation catalog service.
#[derive(Clone)]
pub struct McpCatalogService {
    db: Database,
}

impl McpCatalogService {
    /// Create service with default database.
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            db: Database::new()?,
        })
    }

    /// Create service with injected database.
    pub fn with_database(db: Database) -> Self {
        Self { db }
    }

    /// List catalog entries with optional filter.
    pub fn list_catalog(
        &self,
        filter: Option<McpCatalogFilter>,
    ) -> AppResult<McpCatalogListResponse> {
        let (items, source, fetched_at, signature_valid) = self.load_catalog_items()?;
        let filtered = Self::apply_filter(items, filter.as_ref());
        Ok(McpCatalogListResponse {
            items: filtered,
            source,
            fetched_at,
            signature_valid,
        })
    }

    /// Refresh catalog cache.
    pub fn refresh_catalog(&self, force: bool) -> AppResult<McpCatalogRefreshResult> {
        if let Some(remote_url) = remote_catalog_url() {
            match self.refresh_remote_catalog(&remote_url, force) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!(
                        event = "catalog_refresh_remote_failed",
                        error = %e,
                        remote_url = %remote_url,
                        "Failed to refresh MCP catalog from remote source; falling back to builtin seed"
                    );
                    return self
                        .refresh_builtin_catalog(Some(format!("remote_refresh_failed: {}", e)));
                }
            }
        }
        self.refresh_builtin_catalog(None)
    }

    fn refresh_remote_catalog(
        &self,
        remote_url: &str,
        force: bool,
    ) -> AppResult<McpCatalogRefreshResult> {
        let now = Utc::now();
        let existing = self.db.get_mcp_catalog_cache(REMOTE_CATALOG_SOURCE_ID)?;
        let client = reqwest::blocking::Client::builder()
            .timeout(StdDuration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .map_err(|e| AppError::internal(format!("Failed to build HTTP client: {}", e)))?;

        let mut request = client.get(remote_url);
        if !force {
            if let Some(etag) = existing.as_ref().and_then(|row| row.etag.as_deref()) {
                if !etag.trim().is_empty() {
                    request = request.header(IF_NONE_MATCH, etag);
                }
            }
        }

        let response = request
            .send()
            .map_err(|e| AppError::internal(format!("Failed to fetch remote catalog: {}", e)))?;

        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            let Some(row) = existing else {
                return Err(AppError::internal(
                    "Remote catalog returned 304 but no cache exists".to_string(),
                ));
            };
            let items = parse_catalog_items(&row.payload_json)?;
            let valid = row
                .signature
                .as_deref()
                .map(|sig| verify_catalog_signature(&row.payload_json, sig, false))
                .unwrap_or(false);
            if !valid {
                return Err(AppError::validation(
                    "Remote catalog cache signature is invalid".to_string(),
                ));
            }
            tracing::info!(
                event = "catalog_refresh",
                source = REMOTE_CATALOG_SOURCE_LABEL,
                updated = false,
                item_count = items.len(),
                unpinned_count = count_unpinned_catalog_items(&items),
                signature_valid = true,
                "MCP catalog cache is up to date (304)"
            );
            return Ok(McpCatalogRefreshResult {
                source: REMOTE_CATALOG_SOURCE_LABEL.to_string(),
                fetched_at: row.fetched_at.unwrap_or_else(|| now.to_rfc3339()),
                item_count: items.len() as u32,
                updated: false,
                signature_valid: true,
                error: None,
            });
        }

        if !response.status().is_success() {
            return Err(AppError::internal(format!(
                "Remote catalog HTTP error: {}",
                response.status()
            )));
        }

        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());
        let signature = response
            .headers()
            .get(CATALOG_SIGNATURE_HEADER)
            .or_else(|| response.headers().get(CATALOG_SIGNATURE_HEADER_FALLBACK))
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::validation("Remote catalog missing signature header"))?
            .trim()
            .to_string();
        let payload_json = response
            .text()
            .map_err(|e| AppError::internal(format!("Failed to read remote catalog: {}", e)))?;
        let items = parse_catalog_items(&payload_json)?;
        let signature_valid = verify_catalog_signature(&payload_json, &signature, false);
        if !signature_valid {
            return Err(AppError::validation(
                "Remote catalog signature verification failed".to_string(),
            ));
        }

        let expires_at = (now + Duration::hours(CACHE_TTL_HOURS)).to_rfc3339();
        self.db.upsert_mcp_catalog_cache(
            REMOTE_CATALOG_SOURCE_ID,
            &payload_json,
            Some(&signature),
            etag.as_deref(),
            Some(&expires_at),
        )?;
        tracing::info!(
            event = "catalog_refresh",
            source = REMOTE_CATALOG_SOURCE_LABEL,
            updated = true,
            item_count = items.len(),
            unpinned_count = count_unpinned_catalog_items(&items),
            signature_valid = true,
            "MCP catalog refreshed from remote source"
        );

        Ok(McpCatalogRefreshResult {
            source: REMOTE_CATALOG_SOURCE_LABEL.to_string(),
            fetched_at: now.to_rfc3339(),
            item_count: items.len() as u32,
            updated: true,
            signature_valid: true,
            error: None,
        })
    }

    fn refresh_builtin_catalog(&self, error: Option<String>) -> AppResult<McpCatalogRefreshResult> {
        let now = Utc::now();
        let payload = built_in_catalog_items();
        let payload_json = serde_json::to_string(&payload)?;
        // Built-in seed currently uses sha256 integrity signature.
        // Remote catalogs should provide `ed25519:<base64_signature>`.
        let signature = format!("sha256:{}", sha256_hex(payload_json.as_bytes()));
        let expires_at = (now + Duration::hours(CACHE_TTL_HOURS)).to_rfc3339();
        let signature_valid = verify_catalog_signature(&payload_json, &signature, true);

        self.db.upsert_mcp_catalog_cache(
            BUILTIN_CATALOG_SOURCE_ID,
            &payload_json,
            Some(&signature),
            None,
            Some(&expires_at),
        )?;
        tracing::info!(
            event = "catalog_refresh",
            source = BUILTIN_CATALOG_SOURCE_LABEL,
            updated = true,
            item_count = payload.len(),
            unpinned_count = count_unpinned_catalog_items(&payload),
            signature_valid = signature_valid,
            fallback = error.is_some(),
            "MCP catalog refreshed from builtin seed"
        );

        Ok(McpCatalogRefreshResult {
            source: BUILTIN_CATALOG_SOURCE_LABEL.to_string(),
            fetched_at: now.to_rfc3339(),
            item_count: payload.len() as u32,
            updated: true,
            signature_valid,
            error,
        })
    }

    /// Get a single catalog item.
    pub fn get_item(&self, item_id: &str) -> AppResult<McpCatalogItem> {
        let response = self.list_catalog(None)?;
        response
            .items
            .into_iter()
            .find(|item| item.id == item_id)
            .ok_or_else(|| AppError::not_found(format!("Catalog item not found: {}", item_id)))
    }

    fn load_catalog_items(&self) -> AppResult<(Vec<McpCatalogItem>, String, Option<String>, bool)> {
        if let Some(cached) =
            self.load_cached_catalog(REMOTE_CATALOG_SOURCE_ID, REMOTE_CATALOG_SOURCE_LABEL, false)?
        {
            return Ok(cached);
        }
        if let Some(cached) = self.load_cached_catalog(
            BUILTIN_CATALOG_SOURCE_ID,
            BUILTIN_CATALOG_SOURCE_LABEL,
            true,
        )? {
            return Ok(cached);
        }

        let refresh = self.refresh_catalog(false)?;
        if let Some(cached) =
            self.load_cached_catalog(REMOTE_CATALOG_SOURCE_ID, REMOTE_CATALOG_SOURCE_LABEL, false)?
        {
            return Ok(cached);
        }
        if let Some(cached) = self.load_cached_catalog(
            BUILTIN_CATALOG_SOURCE_ID,
            BUILTIN_CATALOG_SOURCE_LABEL,
            true,
        )? {
            return Ok(cached);
        }
        Err(AppError::internal(format!(
            "Catalog refresh from '{}' returned no readable cache",
            refresh.source
        )))
    }

    fn load_cached_catalog(
        &self,
        source_id: &str,
        source_label: &str,
        allow_sha256: bool,
    ) -> AppResult<Option<(Vec<McpCatalogItem>, String, Option<String>, bool)>> {
        let Some(row) = self.db.get_mcp_catalog_cache(source_id)? else {
            return Ok(None);
        };
        if Self::is_expired(row.expires_at.as_deref()) {
            return Ok(None);
        }

        let items = match parse_catalog_items(&row.payload_json) {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };
        let valid = row
            .signature
            .as_deref()
            .map(|sig| verify_catalog_signature(&row.payload_json, sig, allow_sha256))
            .unwrap_or(false);
        if !valid {
            return Ok(None);
        }
        Ok(Some((
            items,
            source_label.to_string(),
            row.fetched_at,
            true,
        )))
    }

    fn apply_filter(
        items: Vec<McpCatalogItem>,
        filter: Option<&McpCatalogFilter>,
    ) -> Vec<McpCatalogItem> {
        let mut result = items;
        if let Some(filter) = filter {
            if !filter.trust_levels.is_empty() {
                result.retain(|item| filter.trust_levels.contains(&item.trust_level));
            }
            if !filter.tags.is_empty() {
                let expected: std::collections::HashSet<_> =
                    filter.tags.iter().map(|s| s.to_lowercase()).collect();
                result.retain(|item| {
                    let tags: std::collections::HashSet<_> =
                        item.tags.iter().map(|s| s.to_lowercase()).collect();
                    expected.is_subset(&tags)
                });
            }
            if let Some(query) = filter.query.as_ref().map(|q| q.trim().to_lowercase()) {
                if !query.is_empty() {
                    result.retain(|item| {
                        item.name.to_lowercase().contains(&query)
                            || item.vendor.to_lowercase().contains(&query)
                            || item.id.to_lowercase().contains(&query)
                            || item
                                .tags
                                .iter()
                                .any(|tag| tag.to_lowercase().contains(&query))
                    });
                }
            }
        }
        result.sort_by(|a, b| {
            a.trust_level
                .cmp(&b.trust_level)
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        result
    }

    fn is_expired(expires_at: Option<&str>) -> bool {
        let Some(expires_at) = expires_at else {
            return true;
        };
        chrono::DateTime::parse_from_rfc3339(expires_at)
            .map(|dt| dt.with_timezone(&Utc) <= Utc::now())
            .unwrap_or(true)
    }
}

fn remote_catalog_url() -> Option<String> {
    std::env::var(CATALOG_REMOTE_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn catalog_public_key_hex() -> Option<String> {
    if let Ok(value) = std::env::var(CATALOG_REMOTE_PUBKEY_ENV) {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    if CATALOG_SIGNING_PUBLIC_KEY_HEX
        .chars()
        .all(|char| char == '0')
    {
        return None;
    }
    Some(CATALOG_SIGNING_PUBLIC_KEY_HEX.to_string())
}

fn parse_catalog_items(payload_json: &str) -> AppResult<Vec<McpCatalogItem>> {
    if let Ok(items) = serde_json::from_str::<Vec<McpCatalogItem>>(payload_json) {
        return Ok(items);
    }
    let value: serde_json::Value = serde_json::from_str(payload_json)?;
    let items = value
        .get("items")
        .cloned()
        .ok_or_else(|| AppError::validation("Catalog payload missing 'items'"))?;
    let parsed: Vec<McpCatalogItem> = serde_json::from_value(items)?;
    Ok(parsed)
}

fn verify_catalog_signature(payload: &str, signature: &str, allow_sha256: bool) -> bool {
    if allow_sha256 {
        if let Some(expected) = signature.strip_prefix("sha256:") {
            return sha256_hex(payload.as_bytes()) == expected;
        }
    }
    if let Some(expected) = signature.strip_prefix("sha256:") {
        // Remote catalogs must not use digest-only integrity signatures.
        let _ = expected;
        return false;
    }
    if let Some(raw_sig) = signature.strip_prefix("ed25519:") {
        return verify_ed25519_signature(payload.as_bytes(), raw_sig);
    }
    false
}

fn verify_ed25519_signature(payload: &[u8], signature_b64: &str) -> bool {
    let Some(pubkey_hex) = catalog_public_key_hex() else {
        return false;
    };
    verify_ed25519_signature_with_key(payload, signature_b64, &pubkey_hex)
}

fn verify_ed25519_signature_with_key(
    payload: &[u8],
    signature_b64: &str,
    pubkey_hex: &str,
) -> bool {
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(signature_b64))
        .ok();
    let Some(sig_bytes) = sig_bytes else {
        return false;
    };
    if sig_bytes.len() != 64 {
        return false;
    }

    let pubkey_bytes = match hex_decode(pubkey_hex) {
        Some(bytes) => bytes,
        None => return false,
    };
    if pubkey_bytes.len() != 32 {
        return false;
    }

    let pubkey_array: [u8; 32] = match pubkey_bytes.as_slice().try_into() {
        Ok(value) => value,
        Err(_) => return false,
    };
    let verifying_key = match VerifyingKey::from_bytes(&pubkey_array) {
        Ok(key) => key,
        Err(_) => return false,
    };
    let signature = match Signature::from_slice(&sig_bytes) {
        Ok(value) => value,
        Err(_) => return false,
    };
    verifying_key.verify(payload, &signature).is_ok()
}

fn hex_decode(input: &str) -> Option<Vec<u8>> {
    if input.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    let bytes = input.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        let hi = hex_nibble(bytes[idx])?;
        let lo = hex_nibble(bytes[idx + 1])?;
        out.push((hi << 4) | lo);
        idx += 2;
    }
    Some(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    digest
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<String>()
}

fn count_unpinned_catalog_items(items: &[McpCatalogItem]) -> usize {
    items
        .iter()
        .filter(|item| item.strategies.iter().any(strategy_has_unpinned_artifact))
        .count()
}

fn strategy_has_unpinned_artifact(strategy: &McpInstallStrategy) -> bool {
    if let Some(image) = strategy.recipe.get("image").and_then(|v| v.as_str()) {
        if docker_image_unpinned(image) {
            return true;
        }
    }
    if let Some(package) = strategy.recipe.get("package").and_then(|v| v.as_str()) {
        if package_spec_unpinned(package) {
            return true;
        }
    }
    if let Some(package) = strategy
        .recipe
        .get("bridge_package")
        .and_then(|v| v.as_str())
    {
        if package_spec_unpinned(package) {
            return true;
        }
    }
    false
}

fn docker_image_unpinned(image: &str) -> bool {
    let trimmed = image.trim();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.contains("@sha256:") {
        return false;
    }

    let last_segment = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if let Some((_, tag)) = last_segment.split_once(':') {
        let normalized = tag.trim();
        return normalized.is_empty() || normalized.eq_ignore_ascii_case("latest");
    }
    true
}

fn package_spec_unpinned(spec: &str) -> bool {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.starts_with('@') {
        let scoped_part = &trimmed[1..];
        if let Some(relative_idx) = scoped_part.rfind('@') {
            let version = &scoped_part[(relative_idx + 1)..];
            return version.trim().is_empty() || version.eq_ignore_ascii_case("latest");
        }
        return true;
    }
    if let Some((_, version)) = trimmed.split_once('@') {
        return version.trim().is_empty() || version.eq_ignore_ascii_case("latest");
    }
    true
}

fn requirement(runtime: McpRuntimeKind, min_version: &str, optional: bool) -> RuntimeRequirement {
    RuntimeRequirement {
        runtime,
        min_version: Some(min_version.to_string()),
        optional,
    }
}

fn strategy(
    id: &str,
    kind: McpInstallStrategyKind,
    priority: u32,
    requirements: Vec<RuntimeRequirement>,
    recipe: serde_json::Value,
) -> McpInstallStrategy {
    McpInstallStrategy {
        id: id.to_string(),
        kind,
        priority,
        requirements,
        recipe,
        verification: McpInstallVerification {
            require_initialize: true,
            require_tools_list: true,
        },
    }
}

fn secret(key: &str, label: &str, required: bool, secret_type: &str) -> McpSecretSchemaField {
    McpSecretSchemaField {
        key: key.to_string(),
        label: label.to_string(),
        required,
        secret_type: Some(secret_type.to_string()),
    }
}

fn common_os() -> Vec<String> {
    vec![
        "macos".to_string(),
        "windows".to_string(),
        "linux".to_string(),
    ]
}

/// Built-in v1 catalog list (13+ entries).
pub fn built_in_catalog_items() -> Vec<McpCatalogItem> {
    vec![
        McpCatalogItem {
            id: "minimax-coding-plan-mcp".to_string(),
            name: "MiniMax Coding Plan MCP".to_string(),
            vendor: "MiniMax".to_string(),
            trust_level: McpCatalogTrustLevel::Verified,
            tags: vec![
                "coding".to_string(),
                "planning".to_string(),
                "python".to_string(),
            ],
            docs_url: Some(
                "https://platform.minimaxi.com/docs/guides/coding-plan-mcp-guide".to_string(),
            ),
            maintained_by: Some("minimax".to_string()),
            os_support: common_os(),
            strategies: vec![
                strategy(
                    "uv_tool",
                    McpInstallStrategyKind::UvTool,
                    1,
                    vec![requirement(McpRuntimeKind::Uv, "0.4", false)],
                    serde_json::json!({
                        "server_type": "stdio",
                        "launcher": "uvx",
                        "package": "minimax-coding-plan-mcp",
                        "args": []
                    }),
                ),
                strategy(
                    "python_venv",
                    McpInstallStrategyKind::PythonVenv,
                    2,
                    vec![requirement(McpRuntimeKind::Python, "3.10", false)],
                    serde_json::json!({
                        "server_type": "stdio",
                        "launcher": "python_venv",
                        "package": "minimax-coding-plan-mcp",
                        "args": []
                    }),
                ),
            ],
            secrets_schema: vec![secret(
                "MINIMAX_API_KEY",
                "MiniMax API Key",
                true,
                "api_key",
            )],
        },
        McpCatalogItem {
            id: "postgres-mcp".to_string(),
            name: "postgres-mcp".to_string(),
            vendor: "Community".to_string(),
            trust_level: McpCatalogTrustLevel::Community,
            tags: vec!["database".to_string(), "postgres".to_string()],
            docs_url: Some("https://github.com/crystaldba/postgres-mcp".to_string()),
            maintained_by: Some("crystaldba".to_string()),
            os_support: common_os(),
            strategies: vec![
                strategy(
                    "docker",
                    McpInstallStrategyKind::Docker,
                    1,
                    vec![requirement(McpRuntimeKind::Docker, "24", false)],
                    serde_json::json!({
                        "server_type": "stdio",
                        "launcher": "docker",
                        "image": "ghcr.io/crystaldba/postgres-mcp:latest",
                        "args": []
                    }),
                ),
                strategy(
                    "python_venv",
                    McpInstallStrategyKind::PythonVenv,
                    2,
                    vec![requirement(McpRuntimeKind::Python, "3.10", false)],
                    serde_json::json!({
                        "server_type": "stdio",
                        "launcher": "python_venv",
                        "package": "postgres-mcp",
                        "args": []
                    }),
                ),
            ],
            secrets_schema: vec![secret("DATABASE_URL", "PostgreSQL URL", true, "connection")],
        },
        McpCatalogItem {
            id: "zhipu-search-mcp".to_string(),
            name: "Zhipu Search MCP".to_string(),
            vendor: "Zhipu".to_string(),
            trust_level: McpCatalogTrustLevel::Official,
            tags: vec!["search".to_string(), "remote".to_string()],
            docs_url: Some(
                "https://docs.bigmodel.cn/cn/coding-plan/mcp/search-mcp-server".to_string(),
            ),
            maintained_by: Some("zhipu".to_string()),
            os_support: common_os(),
            strategies: vec![strategy(
                "stream_http_api_key",
                McpInstallStrategyKind::StreamHttpApiKey,
                1,
                vec![],
                serde_json::json!({
                    "server_type": "stream_http",
                    "url": "https://open.bigmodel.cn/api/mcp/search",
                    "headers": { "Authorization": "Bearer {{ZHIPU_API_KEY}}" }
                }),
            )],
            secrets_schema: vec![secret("ZHIPU_API_KEY", "Zhipu API Key", true, "api_key")],
        },
        McpCatalogItem {
            id: "zhipu-reader-mcp".to_string(),
            name: "Zhipu Reader MCP".to_string(),
            vendor: "Zhipu".to_string(),
            trust_level: McpCatalogTrustLevel::Official,
            tags: vec!["reader".to_string(), "remote".to_string()],
            docs_url: Some(
                "https://docs.bigmodel.cn/cn/coding-plan/mcp/reader-mcp-server".to_string(),
            ),
            maintained_by: Some("zhipu".to_string()),
            os_support: common_os(),
            strategies: vec![strategy(
                "stream_http_api_key",
                McpInstallStrategyKind::StreamHttpApiKey,
                1,
                vec![],
                serde_json::json!({
                    "server_type": "stream_http",
                    "url": "https://open.bigmodel.cn/api/mcp/reader",
                    "headers": { "Authorization": "Bearer {{ZHIPU_API_KEY}}" }
                }),
            )],
            secrets_schema: vec![secret("ZHIPU_API_KEY", "Zhipu API Key", true, "api_key")],
        },
        McpCatalogItem {
            id: "zhipu-vision-mcp".to_string(),
            name: "Zhipu Vision MCP".to_string(),
            vendor: "Zhipu".to_string(),
            trust_level: McpCatalogTrustLevel::Official,
            tags: vec!["vision".to_string(), "node".to_string()],
            docs_url: Some(
                "https://docs.bigmodel.cn/cn/coding-plan/mcp/vision-mcp-server".to_string(),
            ),
            maintained_by: Some("zhipu".to_string()),
            os_support: common_os(),
            strategies: vec![strategy(
                "node_managed_pkg",
                McpInstallStrategyKind::NodeManagedPkg,
                1,
                vec![requirement(McpRuntimeKind::Node, "20", false)],
                serde_json::json!({
                    "server_type": "stdio",
                    "launcher": "node_managed_pkg",
                    "package": "@zhipu-ai/mcp-vision-server",
                    "args": []
                }),
            )],
            secrets_schema: vec![secret("ZHIPU_API_KEY", "Zhipu API Key", true, "api_key")],
        },
        McpCatalogItem {
            id: "github-mcp".to_string(),
            name: "GitHub MCP".to_string(),
            vendor: "GitHub".to_string(),
            trust_level: McpCatalogTrustLevel::Official,
            tags: vec!["github".to_string(), "code".to_string()],
            docs_url: Some("https://github.com/github/github-mcp-server".to_string()),
            maintained_by: Some("github".to_string()),
            os_support: common_os(),
            strategies: vec![
                strategy(
                    "stream_http_api_key",
                    McpInstallStrategyKind::StreamHttpApiKey,
                    0,
                    vec![],
                    serde_json::json!({
                        "server_type": "stream_http",
                        "url": "https://api.githubcopilot.com/mcp/",
                        "headers": { "Authorization": "Bearer {{GITHUB_TOKEN}}" }
                    }),
                ),
                strategy(
                    "docker",
                    McpInstallStrategyKind::Docker,
                    1,
                    vec![requirement(McpRuntimeKind::Docker, "24", false)],
                    serde_json::json!({
                        "server_type": "stdio",
                        "launcher": "docker",
                        "image": "ghcr.io/github/github-mcp-server:latest",
                        "args": []
                    }),
                ),
            ],
            secrets_schema: vec![secret("GITHUB_TOKEN", "GitHub Token", true, "api_key")],
        },
        McpCatalogItem {
            id: "chrome-devtools-mcp".to_string(),
            name: "Chrome DevTools MCP".to_string(),
            vendor: "Chrome DevTools".to_string(),
            trust_level: McpCatalogTrustLevel::Official,
            tags: vec![
                "browser".to_string(),
                "devtools".to_string(),
                "frontend".to_string(),
                "debugging".to_string(),
                "performance".to_string(),
            ],
            docs_url: Some("https://github.com/ChromeDevTools/chrome-devtools-mcp".to_string()),
            maintained_by: Some("ChromeDevTools".to_string()),
            os_support: common_os(),
            strategies: vec![
                strategy(
                    "node_managed_pkg",
                    McpInstallStrategyKind::NodeManagedPkg,
                    1,
                    vec![requirement(McpRuntimeKind::Node, "20.19", false)],
                    serde_json::json!({
                        "server_type": "stdio",
                        "launcher": "node_managed_pkg",
                        "package": "chrome-devtools-mcp@latest",
                        "args": ["--isolated=true", "--no-usage-statistics"]
                    }),
                ),
                strategy(
                    "node_managed_pkg_slim_headless",
                    McpInstallStrategyKind::NodeManagedPkg,
                    2,
                    vec![requirement(McpRuntimeKind::Node, "20.19", false)],
                    serde_json::json!({
                        "server_type": "stdio",
                        "launcher": "node_managed_pkg",
                        "package": "chrome-devtools-mcp@latest",
                        "args": ["--slim", "--headless=true", "--isolated=true", "--no-usage-statistics"]
                    }),
                ),
            ],
            secrets_schema: vec![],
        },
        McpCatalogItem {
            id: "redis-mcp".to_string(),
            name: "redis/mcp-redis".to_string(),
            vendor: "Redis".to_string(),
            trust_level: McpCatalogTrustLevel::Verified,
            tags: vec![
                "database".to_string(),
                "redis".to_string(),
                "python".to_string(),
            ],
            docs_url: Some("https://github.com/redis/mcp-redis".to_string()),
            maintained_by: Some("redis".to_string()),
            os_support: common_os(),
            strategies: vec![
                strategy(
                    "uv_tool",
                    McpInstallStrategyKind::UvTool,
                    1,
                    vec![requirement(McpRuntimeKind::Uv, "0.4", false)],
                    serde_json::json!({
                        "server_type": "stdio",
                        "launcher": "uvx",
                        "package": "mcp-redis",
                        "args": []
                    }),
                ),
                strategy(
                    "python_venv",
                    McpInstallStrategyKind::PythonVenv,
                    2,
                    vec![requirement(McpRuntimeKind::Python, "3.10", false)],
                    serde_json::json!({
                        "server_type": "stdio",
                        "launcher": "python_venv",
                        "package": "mcp-redis",
                        "args": []
                    }),
                ),
            ],
            secrets_schema: vec![secret("REDIS_URL", "Redis URL", true, "connection")],
        },
        McpCatalogItem {
            id: "mysql-mcp".to_string(),
            name: "mcp-server-mysql".to_string(),
            vendor: "Community".to_string(),
            trust_level: McpCatalogTrustLevel::Community,
            tags: vec![
                "database".to_string(),
                "mysql".to_string(),
                "node".to_string(),
            ],
            docs_url: Some("https://github.com/benborla/mcp-server-mysql".to_string()),
            maintained_by: Some("benborla".to_string()),
            os_support: common_os(),
            strategies: vec![strategy(
                "node_managed_pkg",
                McpInstallStrategyKind::NodeManagedPkg,
                1,
                vec![requirement(McpRuntimeKind::Node, "20", false)],
                serde_json::json!({
                    "server_type": "stdio",
                    "launcher": "node_managed_pkg",
                    "package": "mcp-server-mysql",
                    "args": []
                }),
            )],
            secrets_schema: vec![secret("MYSQL_URL", "MySQL URL", true, "connection")],
        },
        McpCatalogItem {
            id: "notion-mcp".to_string(),
            name: "Notion MCP".to_string(),
            vendor: "Notion".to_string(),
            trust_level: McpCatalogTrustLevel::Official,
            tags: vec![
                "notion".to_string(),
                "oauth".to_string(),
                "remote".to_string(),
            ],
            docs_url: Some(
                "https://developers.notion.com/guides/mcp/get-started-with-mcp".to_string(),
            ),
            maintained_by: Some("notion".to_string()),
            os_support: common_os(),
            strategies: vec![strategy(
                "oauth_bridge_mcp_remote",
                McpInstallStrategyKind::OauthBridgeMcpRemote,
                1,
                vec![requirement(McpRuntimeKind::Node, "20", false)],
                serde_json::json!({
                    "server_type": "stdio",
                    "launcher": "oauth_bridge_mcp_remote",
                    "target_url": "https://mcp.notion.com/mcp",
                    "bridge_package": "mcp-remote"
                }),
            )],
            secrets_schema: vec![],
        },
        McpCatalogItem {
            id: "linear-mcp".to_string(),
            name: "Linear MCP".to_string(),
            vendor: "Linear".to_string(),
            trust_level: McpCatalogTrustLevel::Official,
            tags: vec![
                "linear".to_string(),
                "oauth".to_string(),
                "remote".to_string(),
            ],
            docs_url: Some("https://linear.app/docs/mcp".to_string()),
            maintained_by: Some("linear".to_string()),
            os_support: common_os(),
            strategies: vec![strategy(
                "oauth_bridge_mcp_remote",
                McpInstallStrategyKind::OauthBridgeMcpRemote,
                1,
                vec![requirement(McpRuntimeKind::Node, "20", false)],
                serde_json::json!({
                    "server_type": "stdio",
                    "launcher": "oauth_bridge_mcp_remote",
                    "target_url": "https://mcp.linear.app/sse",
                    "bridge_package": "mcp-remote"
                }),
            )],
            secrets_schema: vec![],
        },
        McpCatalogItem {
            id: "figma-mcp".to_string(),
            name: "Figma MCP".to_string(),
            vendor: "Figma".to_string(),
            trust_level: McpCatalogTrustLevel::Official,
            tags: vec![
                "figma".to_string(),
                "oauth".to_string(),
                "remote".to_string(),
            ],
            docs_url: Some(
                "https://developers.figma.com/docs/figma-mcp-server/remote-server-installation/"
                    .to_string(),
            ),
            maintained_by: Some("figma".to_string()),
            os_support: common_os(),
            strategies: vec![strategy(
                "oauth_bridge_mcp_remote",
                McpInstallStrategyKind::OauthBridgeMcpRemote,
                1,
                vec![requirement(McpRuntimeKind::Node, "20", false)],
                serde_json::json!({
                    "server_type": "stdio",
                    "launcher": "oauth_bridge_mcp_remote",
                    "target_url": "https://mcp.figma.com/mcp",
                    "bridge_package": "mcp-remote"
                }),
            )],
            secrets_schema: vec![],
        },
        McpCatalogItem {
            id: "upstash-context7".to_string(),
            name: "Upstash Context7".to_string(),
            vendor: "Upstash".to_string(),
            trust_level: McpCatalogTrustLevel::Verified,
            tags: vec![
                "context".to_string(),
                "remote".to_string(),
                "search".to_string(),
            ],
            docs_url: Some("https://github.com/upstash/context7".to_string()),
            maintained_by: Some("upstash".to_string()),
            os_support: common_os(),
            strategies: vec![
                strategy(
                    "stream_http_api_key_optional",
                    McpInstallStrategyKind::StreamHttpApiKeyOptional,
                    1,
                    vec![],
                    serde_json::json!({
                        "server_type": "stream_http",
                        "url": "https://mcp.context7.com/mcp",
                        "headers": { "Authorization": "Bearer {{CONTEXT7_API_KEY}}" }
                    }),
                ),
                strategy(
                    "node_managed_pkg",
                    McpInstallStrategyKind::NodeManagedPkg,
                    2,
                    vec![requirement(McpRuntimeKind::Node, "20", false)],
                    serde_json::json!({
                        "server_type": "stdio",
                        "launcher": "node_managed_pkg",
                        "package": "@upstash/context7-mcp",
                        "args": []
                    }),
                ),
            ],
            secrets_schema: vec![secret(
                "CONTEXT7_API_KEY",
                "Context7 API Key",
                false,
                "api_key",
            )],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn test_verify_sha256_signature_policy() {
        let payload = "[]";
        let signature = format!("sha256:{}", sha256_hex(payload.as_bytes()));
        assert!(verify_catalog_signature(payload, &signature, true));
        assert!(!verify_catalog_signature(payload, &signature, false));
    }

    #[test]
    fn test_verify_ed25519_signature_with_explicit_key() {
        let payload = br#"{"items":[]}"#;
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let signature =
            base64::engine::general_purpose::STANDARD.encode(signing_key.sign(payload).to_bytes());
        let pubkey_hex = signing_key
            .verifying_key()
            .to_bytes()
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<String>();

        assert!(verify_ed25519_signature_with_key(
            payload,
            &signature,
            &pubkey_hex
        ));
        assert!(!verify_ed25519_signature_with_key(
            payload,
            "invalid",
            &pubkey_hex
        ));
    }

    #[test]
    fn test_parse_catalog_items_supports_array_and_wrapped_payload() {
        let item = built_in_catalog_items()
            .into_iter()
            .next()
            .expect("seed item");
        let array_payload = serde_json::json!([item.clone()]).to_string();
        let wrapped_payload = serde_json::json!({ "items": [item] }).to_string();

        let from_array = parse_catalog_items(&array_payload).expect("array payload");
        let from_wrapped = parse_catalog_items(&wrapped_payload).expect("wrapped payload");

        assert_eq!(from_array.len(), 1);
        assert_eq!(from_wrapped.len(), 1);
    }
}
