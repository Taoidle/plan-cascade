//! Unified Settings Export/Import Service (v6.0)
//!
//! Provides password-based encryption for API key export and
//! collection/restoration of all backend settings from various data sources.

use std::collections::HashMap;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use pbkdf2::pbkdf2_hmac;
use rand::rngs::OsRng;
use rand::RngCore;
use rusqlite::params;
use sha2::Sha256;

use crate::models::export::{
    BackendSettingsExport, GuardrailRuleExport, ProxyExport, RemoteExport,
    SettingsImportResult,
};
use crate::models::settings::AppConfig;
use crate::storage::{ConfigService, Database};
use crate::utils::error::{AppError, AppResult};
use crate::utils::paths::plan_cascade_dir;

const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_SIZE: usize = 16;
const NONCE_SIZE: usize = 12;
const KEY_SIZE: usize = 32;

// ============================================================================
// Password-based encryption
// ============================================================================

/// Encrypt plaintext using a user-provided password.
///
/// Returns base64-encoded `salt[16] || nonce[12] || ciphertext_with_tag`.
pub fn encrypt_with_password(plaintext: &str, password: &str) -> AppResult<String> {
    let mut salt = [0u8; SALT_SIZE];
    OsRng.fill_bytes(&mut salt);

    let mut derived_key = [0u8; KEY_SIZE];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, PBKDF2_ITERATIONS, &mut derived_key);

    let key = Key::<Aes256Gcm>::from_slice(&derived_key);
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| AppError::keyring(format!("Password encryption failed: {}", e)))?;

    // Combine: salt || nonce || ciphertext (includes GCM tag)
    let mut combined = Vec::with_capacity(SALT_SIZE + NONCE_SIZE + ciphertext.len());
    combined.extend_from_slice(&salt);
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(combined))
}

/// Decrypt a base64-encoded ciphertext using a user-provided password.
///
/// Returns the original plaintext or an error if the password is wrong.
pub fn decrypt_with_password(encrypted: &str, password: &str) -> AppResult<String> {
    let data = BASE64
        .decode(encrypted)
        .map_err(|e| AppError::keyring(format!("Base64 decode failed: {}", e)))?;

    let min_len = SALT_SIZE + NONCE_SIZE + 1; // At least 1 byte ciphertext
    if data.len() < min_len {
        return Err(AppError::keyring("Invalid encrypted data: too short"));
    }

    let salt = &data[..SALT_SIZE];
    let nonce_bytes = &data[SALT_SIZE..SALT_SIZE + NONCE_SIZE];
    let ciphertext = &data[SALT_SIZE + NONCE_SIZE..];

    let mut derived_key = [0u8; KEY_SIZE];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut derived_key);

    let key = Key::<Aes256Gcm>::from_slice(&derived_key);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| {
        AppError::keyring("Decryption failed: wrong password or corrupted data")
    })?;

    String::from_utf8(plaintext)
        .map_err(|e| AppError::keyring(format!("Decrypted data is not valid UTF-8: {}", e)))
}

// ============================================================================
// Backend settings collection
// ============================================================================

/// Collect all backend settings from various data sources into a single export structure.
pub fn collect_backend_settings(
    db: &Database,
    config: &AppConfig,
) -> AppResult<BackendSettingsExport> {
    // 1. AppConfig
    let config_value = serde_json::to_value(config)?;

    // 2. Embedding config
    let embedding = db
        .get_setting("embedding_config")?
        .and_then(|s| serde_json::from_str(&s).ok());

    // 3. Proxy settings
    let proxy_global = db
        .get_setting("proxy_global")?
        .and_then(|s| serde_json::from_str(&s).ok());

    let proxy_strategies = db.get_settings_by_prefix("proxy_strategy_")?;
    let strategies_map: HashMap<String, serde_json::Value> = proxy_strategies
        .into_iter()
        .filter_map(|(k, v)| serde_json::from_str(&v).ok().map(|val| (k, val)))
        .collect();

    let proxy_customs = db.get_settings_by_prefix("proxy_custom_")?;
    let customs_map: HashMap<String, serde_json::Value> = proxy_customs
        .into_iter()
        .filter_map(|(k, v)| serde_json::from_str(&v).ok().map(|val| (k, val)))
        .collect();

    let proxy = ProxyExport {
        global: proxy_global,
        strategies: serde_json::to_value(strategies_map)?,
        custom_configs: serde_json::to_value(customs_map)?,
    };

    // 4. Webhooks
    let webhooks: Vec<serde_json::Value> = db
        .list_webhook_channels()?
        .iter()
        .filter_map(|ch| serde_json::to_value(ch).ok())
        .collect();

    // 5. Guardrail rules (direct SQL)
    let guardrails = collect_guardrail_rules(db)?;

    // 6. Remote settings
    let remote = RemoteExport {
        gateway: db
            .get_setting("remote_gateway_config")?
            .and_then(|s| serde_json::from_str(&s).ok()),
        telegram: db
            .get_setting("remote_telegram_config")?
            .and_then(|s| serde_json::from_str(&s).ok()),
    };

    // 7. A2A agents (direct SQL)
    let a2a_agents = collect_a2a_agents(db)?;

    // 8. MCP servers (reset status to "unknown")
    let mcp_servers: Vec<serde_json::Value> = db
        .list_mcp_servers()?
        .iter()
        .filter_map(|s| {
            serde_json::to_value(s).ok().map(|mut v| {
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("status".to_string(), serde_json::json!("unknown"));
                }
                v
            })
        })
        .collect();

    // 9. Plugin settings
    let plugin_settings = read_plugin_settings();

    Ok(BackendSettingsExport {
        config: config_value,
        embedding,
        proxy,
        webhooks,
        guardrails,
        remote,
        a2a_agents,
        mcp_servers,
        plugin_settings,
    })
}

/// Collect guardrail rules from the database via direct SQL.
fn collect_guardrail_rules(db: &Database) -> AppResult<Vec<GuardrailRuleExport>> {
    let conn = db.get_connection()?;
    let mut stmt =
        conn.prepare("SELECT id, name, pattern, action, enabled FROM guardrail_rules")?;
    let rules = stmt
        .query_map([], |row| {
            Ok(GuardrailRuleExport {
                id: row.get(0)?,
                name: row.get(1)?,
                pattern: row.get(2)?,
                action: row.get(3)?,
                enabled: row.get::<_, i32>(4)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rules)
}

/// Collect A2A remote agents from the database via direct SQL.
fn collect_a2a_agents(db: &Database) -> AppResult<Vec<serde_json::Value>> {
    let conn = db.get_connection()?;
    let mut stmt = conn.prepare(
        "SELECT id, base_url, name, description, capabilities, endpoint, version, auth_required, supported_inputs, created_at, updated_at
         FROM remote_agents",
    )?;
    let agents = stmt
        .query_map([], |row| {
            let capabilities_str: String = row.get(4)?;
            let supported_inputs_str: String = row.get(8)?;
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "base_url": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?,
                "description": row.get::<_, String>(3)?,
                "capabilities": serde_json::from_str::<serde_json::Value>(&capabilities_str).unwrap_or_default(),
                "endpoint": row.get::<_, String>(5)?,
                "version": row.get::<_, String>(6)?,
                "auth_required": row.get::<_, i32>(7)? != 0,
                "supported_inputs": serde_json::from_str::<serde_json::Value>(&supported_inputs_str).unwrap_or_default(),
                "created_at": row.get::<_, Option<String>>(9)?,
                "updated_at": row.get::<_, Option<String>>(10)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(agents)
}

/// Write plugin settings to ~/.plan-cascade/plugin-settings.json.
pub fn import_plugin_settings(plugin_settings: &serde_json::Value) -> AppResult<()> {
    let path = plan_cascade_dir()?.join("plugin-settings.json");
    let content = serde_json::to_string_pretty(plugin_settings)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Read plugin settings from ~/.plan-cascade/plugin-settings.json.
fn read_plugin_settings() -> Option<serde_json::Value> {
    let path = plan_cascade_dir().ok()?.join("plugin-settings.json");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

// ============================================================================
// Backend settings import
// ============================================================================

/// Import backend settings from an export structure, writing to all data sources.
///
/// Each section is imported independently — errors in one section don't block others.
pub fn import_backend_settings(
    db: &Database,
    config_service: &mut ConfigService,
    backend: &BackendSettingsExport,
) -> SettingsImportResult {
    let mut result = SettingsImportResult {
        success: true,
        frontend: None,
        imported_sections: Vec::new(),
        skipped_sections: Vec::new(),
        warnings: Vec::new(),
        errors: Vec::new(),
    };

    // 1. Config
    import_section(&mut result, "config", || {
        let new_config: AppConfig = serde_json::from_value(backend.config.clone())?;
        let update = crate::models::settings::SettingsUpdate {
            theme: Some(new_config.theme),
            language: Some(new_config.language),
            default_provider: Some(new_config.default_provider),
            default_model: Some(new_config.default_model),
            analytics_enabled: Some(new_config.analytics_enabled),
            auto_save_interval: Some(new_config.auto_save_interval),
            max_recent_projects: Some(new_config.max_recent_projects),
            debug_mode: Some(new_config.debug_mode),
            search_provider: Some(new_config.search_provider),
        };
        config_service.update_config(update)?;
        Ok(())
    });

    // 2. Embedding
    if let Some(ref embedding) = backend.embedding {
        import_section(&mut result, "embedding", || {
            let json_str = serde_json::to_string(embedding)?;
            db.set_setting("embedding_config", &json_str)?;
            Ok(())
        });
    } else {
        result.skipped_sections.push("embedding".to_string());
    }

    // 3. Proxy
    import_section(&mut result, "proxy", || {
        if let Some(ref global) = backend.proxy.global {
            let json_str = serde_json::to_string(global)?;
            db.set_setting("proxy_global", &json_str)?;
        }
        // Strategies
        if let Some(obj) = backend.proxy.strategies.as_object() {
            for (key, value) in obj {
                let json_str = serde_json::to_string(value)?;
                db.set_setting(key, &json_str)?;
            }
        }
        // Custom configs
        if let Some(obj) = backend.proxy.custom_configs.as_object() {
            for (key, value) in obj {
                let json_str = serde_json::to_string(value)?;
                db.set_setting(key, &json_str)?;
            }
        }
        Ok(())
    });

    // 4. Webhooks — clear old + insert new
    import_section(&mut result, "webhooks", || {
        let conn = db.get_connection()?;
        conn.execute("DELETE FROM webhook_channels", [])?;
        drop(conn);
        for wh_value in &backend.webhooks {
            let ch: crate::services::webhook::types::WebhookChannelConfig =
                serde_json::from_value(wh_value.clone())?;
            db.insert_webhook_channel(&ch)?;
        }
        Ok(())
    });

    // 5. Guardrails — clear old custom rules + insert new
    import_section(&mut result, "guardrails", || {
        let conn = db.get_connection()?;
        conn.execute("DELETE FROM guardrail_rules", [])?;
        for rule in &backend.guardrails {
            conn.execute(
                "INSERT INTO guardrail_rules (id, name, pattern, action, enabled) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![rule.id, rule.name, rule.pattern, rule.action, rule.enabled as i32],
            )?;
        }
        Ok(())
    });

    // 6. Remote
    import_section(&mut result, "remote", || {
        if let Some(ref gateway) = backend.remote.gateway {
            let json_str = serde_json::to_string(gateway)?;
            db.set_setting("remote_gateway_config", &json_str)?;
        }
        if let Some(ref telegram) = backend.remote.telegram {
            let json_str = serde_json::to_string(telegram)?;
            db.set_setting("remote_telegram_config", &json_str)?;
        }
        Ok(())
    });

    // 7. A2A agents — clear old + insert new
    import_section(&mut result, "a2a_agents", || {
        let conn = db.get_connection()?;
        conn.execute("DELETE FROM remote_agents", [])?;
        for agent_value in &backend.a2a_agents {
            let id = agent_value["id"].as_str().unwrap_or_default();
            let base_url = agent_value["base_url"].as_str().unwrap_or_default();
            let name = agent_value["name"].as_str().unwrap_or_default();
            let description = agent_value["description"].as_str().unwrap_or_default();
            let capabilities = serde_json::to_string(&agent_value["capabilities"])?;
            let endpoint = agent_value["endpoint"].as_str().unwrap_or_default();
            let version = agent_value["version"].as_str().unwrap_or_default();
            let auth_required = agent_value["auth_required"].as_bool().unwrap_or(false) as i32;
            let supported_inputs = serde_json::to_string(&agent_value["supported_inputs"])?;

            conn.execute(
                "INSERT INTO remote_agents (id, base_url, name, description, capabilities, endpoint, version, auth_required, supported_inputs)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![id, base_url, name, description, capabilities, endpoint, version, auth_required, supported_inputs],
            )?;
        }
        Ok(())
    });

    // 8. MCP servers — upsert by id
    import_section(&mut result, "mcp_servers", || {
        for server_value in &backend.mcp_servers {
            let server: crate::models::McpServer = serde_json::from_value(server_value.clone())?;
            // Try update first, if no rows affected then insert
            match db.get_mcp_server(&server.id)? {
                Some(_) => db.update_mcp_server(&server)?,
                None => db.insert_mcp_server(&server)?,
            }
        }
        Ok(())
    });

    // 9. Plugin settings
    if let Some(ref plugin_settings) = backend.plugin_settings {
        import_section(&mut result, "plugin_settings", || {
            let path = plan_cascade_dir()?.join("plugin-settings.json");
            let content = serde_json::to_string_pretty(plugin_settings)?;
            std::fs::write(path, content)?;
            Ok(())
        });
    } else {
        result.skipped_sections.push("plugin_settings".to_string());
    }

    result.success = result.errors.is_empty();
    result
}

/// Helper: run an import operation for a named section, recording results.
fn import_section<F>(result: &mut SettingsImportResult, section_name: &str, f: F)
where
    F: FnOnce() -> AppResult<()>,
{
    match f() {
        Ok(()) => {
            result.imported_sections.push(section_name.to_string());
        }
        Err(e) => {
            result.errors.push(format!("{}: {}", section_name, e));
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = r#"{"anthropic":"sk-ant-123","openai":"sk-456"}"#;
        let password = "test-password-123";

        let encrypted = encrypt_with_password(plaintext, password).unwrap();
        let decrypted = decrypt_with_password(&encrypted, password).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_different_passwords_produce_different_output() {
        let plaintext = "secret-data";
        let enc1 = encrypt_with_password(plaintext, "password1").unwrap();
        let enc2 = encrypt_with_password(plaintext, "password2").unwrap();
        // Different salt + different key → different output
        assert_ne!(enc1, enc2);
    }

    #[test]
    fn test_wrong_password_fails() {
        let plaintext = "secret-data";
        let password = "correct-password";
        let encrypted = encrypt_with_password(plaintext, password).unwrap();

        let result = decrypt_with_password(&encrypted, "wrong-password");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_plaintext() {
        // Even empty string should encrypt/decrypt successfully
        let encrypted = encrypt_with_password("", "password").unwrap();
        let decrypted = decrypt_with_password(&encrypted, "password").unwrap();
        assert_eq!("", decrypted);
    }

    #[test]
    fn test_invalid_encrypted_data() {
        assert!(decrypt_with_password("not-valid-base64!!!", "password").is_err());
        assert!(decrypt_with_password("AAAA", "password").is_err()); // Too short
    }

    #[test]
    fn test_collect_backend_settings_with_empty_db() {
        let db = Database::new_in_memory().unwrap();
        let config = AppConfig::default();
        let result = collect_backend_settings(&db, &config);
        assert!(result.is_ok());
        let export = result.unwrap();
        assert!(export.webhooks.is_empty());
        assert!(export.guardrails.is_empty());
        assert!(export.a2a_agents.is_empty());
        assert!(export.mcp_servers.is_empty());
    }
}
