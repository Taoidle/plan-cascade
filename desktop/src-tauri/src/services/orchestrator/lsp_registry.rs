//! LSP Server Registry — Language Server Detection and Management
//!
//! Detects installed language servers from PATH and known fallback locations.
//! Provides a trait-based adapter for language-specific server configuration.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use serde_json::Value;
use tracing::{debug, info};

/// Trait for language-specific server adapters.
///
/// Each adapter knows how to detect and spawn its corresponding language server.
pub trait LspServerAdapter: Send + Sync {
    /// The programming language this adapter handles (e.g., "rust", "python").
    fn language(&self) -> &str;

    /// Human-readable server name (e.g., "rust-analyzer").
    fn server_name(&self) -> &str;

    /// Detect if the server binary exists on the system.
    /// Returns the full path to the binary if found.
    fn detect(&self) -> Option<PathBuf>;

    /// Command and arguments to spawn the server.
    fn command(&self) -> (&str, Vec<String>);

    /// Initialization options specific to this server.
    fn init_options(&self) -> Option<Value>;
}

/// Registry that manages multiple language server adapters.
///
/// Detection results are cached per session to avoid redundant PATH lookups.
pub struct LspServerRegistry {
    adapters: Vec<Box<dyn LspServerAdapter>>,
    /// Cached detection results: language -> binary_path
    detected: RwLock<Option<HashMap<String, PathBuf>>>,
}

impl LspServerRegistry {
    /// Create a new registry with all supported language adapters.
    pub fn new() -> Self {
        let adapters: Vec<Box<dyn LspServerAdapter>> = vec![
            Box::new(RustAnalyzerAdapter),
            Box::new(PyrightAdapter),
            Box::new(GoplsAdapter),
            Box::new(VtslsAdapter),
            Box::new(JdtlsAdapter),
        ];

        Self {
            adapters,
            detected: RwLock::new(None),
        }
    }

    /// Run detection for all adapters. Returns detected language -> server name pairs.
    ///
    /// Results are cached: second call returns the cached map without re-detection.
    pub fn detect_all(&self) -> HashMap<String, String> {
        // Check cache first
        {
            let cache = self.detected.read().unwrap();
            if let Some(ref cached) = *cache {
                return cached
                    .iter()
                    .map(|(lang, _path)| {
                        let adapter = self.adapters.iter().find(|a| a.language() == lang);
                        let name = adapter
                            .map(|a| a.server_name().to_string())
                            .unwrap_or_default();
                        (lang.clone(), name)
                    })
                    .collect();
            }
        }

        // Run detection
        let mut results = HashMap::new();
        for adapter in &self.adapters {
            if let Some(path) = adapter.detect() {
                info!(
                    language = adapter.language(),
                    server = adapter.server_name(),
                    path = %path.display(),
                    "LSP server detected"
                );
                results.insert(adapter.language().to_string(), path);
            } else {
                debug!(
                    language = adapter.language(),
                    server = adapter.server_name(),
                    "LSP server not found"
                );
            }
        }

        // Cache the results
        {
            let mut cache = self.detected.write().unwrap();
            *cache = Some(results.clone());
        }

        results
            .iter()
            .map(|(lang, _path)| {
                let adapter = self.adapters.iter().find(|a| a.language() == lang);
                let name = adapter
                    .map(|a| a.server_name().to_string())
                    .unwrap_or_default();
                (lang.clone(), name)
            })
            .collect()
    }

    /// Get adapter for a language (regardless of detection status).
    pub fn get_adapter(&self, language: &str) -> Option<&dyn LspServerAdapter> {
        self.adapters
            .iter()
            .find(|a| a.language() == language)
            .map(|a| a.as_ref())
    }

    /// Check if a specific language server has been detected.
    pub fn is_detected(&self, language: &str) -> bool {
        let cache = self.detected.read().unwrap();
        cache
            .as_ref()
            .map(|m| m.contains_key(language))
            .unwrap_or(false)
    }

    /// Clear the detection cache, forcing re-detection on next call.
    pub fn clear_cache(&self) {
        let mut cache = self.detected.write().unwrap();
        *cache = None;
    }

    /// Get all supported languages.
    pub fn supported_languages(&self) -> Vec<&str> {
        self.adapters.iter().map(|a| a.language()).collect()
    }
}

// =============================================================================
// Helper: Find binary in PATH or fallback paths
// =============================================================================

/// Search for a binary in PATH, then in a list of fallback directories.
fn find_binary(names: &[&str], fallback_dirs: &[PathBuf]) -> Option<PathBuf> {
    // Check PATH first
    for name in names {
        if let Some(path) = which_binary(name) {
            return Some(path);
        }
    }

    // Check fallback directories
    for dir in fallback_dirs {
        for name in names {
            let candidate = dir.join(name);
            if candidate.exists() && candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Simple which-like lookup that checks PATH only (no fallback).
/// Uses std::process::Command to check if a binary exists.
fn which_binary(name: &str) -> Option<PathBuf> {
    // Try to find in PATH by checking common locations
    let path_var = std::env::var("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.exists() && candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

// =============================================================================
// Language Adapters
// =============================================================================

/// Rust Analyzer adapter: rust-analyzer
pub struct RustAnalyzerAdapter;

impl LspServerAdapter for RustAnalyzerAdapter {
    fn language(&self) -> &str {
        "rust"
    }

    fn server_name(&self) -> &str {
        "rust-analyzer"
    }

    fn detect(&self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        let fallbacks = vec![
            home.join(".cargo").join("bin"),
            // rustup toolchains
            home.join(".rustup")
                .join("toolchains")
                .join("stable-x86_64-unknown-linux-gnu")
                .join("bin"),
            home.join(".rustup")
                .join("toolchains")
                .join("stable-x86_64-apple-darwin")
                .join("bin"),
            home.join(".rustup")
                .join("toolchains")
                .join("stable-aarch64-apple-darwin")
                .join("bin"),
        ];

        find_binary(&["rust-analyzer"], &fallbacks)
    }

    fn command(&self) -> (&str, Vec<String>) {
        ("rust-analyzer", vec![])
    }

    fn init_options(&self) -> Option<Value> {
        None
    }
}

/// Pyright adapter: pyright-langserver, basedpyright-langserver, pylsp
pub struct PyrightAdapter;

impl LspServerAdapter for PyrightAdapter {
    fn language(&self) -> &str {
        "python"
    }

    fn server_name(&self) -> &str {
        "pyright"
    }

    fn detect(&self) -> Option<PathBuf> {
        let fallbacks = npm_global_bin_dirs();

        find_binary(
            &["pyright-langserver", "basedpyright-langserver", "pylsp"],
            &fallbacks,
        )
    }

    fn command(&self) -> (&str, Vec<String>) {
        ("pyright-langserver", vec!["--stdio".to_string()])
    }

    fn init_options(&self) -> Option<Value> {
        None
    }
}

/// Gopls adapter: gopls
pub struct GoplsAdapter;

impl LspServerAdapter for GoplsAdapter {
    fn language(&self) -> &str {
        "go"
    }

    fn server_name(&self) -> &str {
        "gopls"
    }

    fn detect(&self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        let fallbacks = vec![home.join("go").join("bin")];

        find_binary(&["gopls"], &fallbacks)
    }

    fn command(&self) -> (&str, Vec<String>) {
        ("gopls", vec!["serve".to_string()])
    }

    fn init_options(&self) -> Option<Value> {
        None
    }
}

/// Vtsls adapter: vtsls, typescript-language-server
pub struct VtslsAdapter;

impl LspServerAdapter for VtslsAdapter {
    fn language(&self) -> &str {
        "typescript"
    }

    fn server_name(&self) -> &str {
        "vtsls"
    }

    fn detect(&self) -> Option<PathBuf> {
        let fallbacks = npm_global_bin_dirs();

        find_binary(&["vtsls", "typescript-language-server"], &fallbacks)
    }

    fn command(&self) -> (&str, Vec<String>) {
        ("vtsls", vec!["--stdio".to_string()])
    }

    fn init_options(&self) -> Option<Value> {
        None
    }
}

/// JDT.LS adapter: jdtls
pub struct JdtlsAdapter;

impl LspServerAdapter for JdtlsAdapter {
    fn language(&self) -> &str {
        "java"
    }

    fn server_name(&self) -> &str {
        "jdtls"
    }

    fn detect(&self) -> Option<PathBuf> {
        let mut fallbacks = vec![];

        // Homebrew prefix (macOS)
        if let Some(brew_prefix) = homebrew_prefix() {
            fallbacks.push(brew_prefix.join("bin"));
        }

        find_binary(&["jdtls"], &fallbacks)
    }

    fn command(&self) -> (&str, Vec<String>) {
        ("jdtls", vec![])
    }

    fn init_options(&self) -> Option<Value> {
        None
    }
}

// =============================================================================
// Platform helpers
// =============================================================================

/// Get npm global bin directories for the current platform.
fn npm_global_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![];

    if let Some(home) = dirs::home_dir() {
        // npm global (default prefix)
        dirs.push(home.join(".npm-global").join("bin"));
        dirs.push(home.join(".npm").join("bin"));

        // pnpm global
        if let Some(data) = dirs::data_local_dir() {
            dirs.push(data.join("pnpm"));
        }

        // yarn global
        dirs.push(home.join(".yarn").join("bin"));

        // Common Linux/macOS paths
        dirs.push(PathBuf::from("/usr/local/bin"));
    }

    dirs
}

/// Get Homebrew prefix on macOS.
fn homebrew_prefix() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        // Apple Silicon
        let arm_prefix = PathBuf::from("/opt/homebrew");
        if arm_prefix.exists() {
            return Some(arm_prefix);
        }
        // Intel
        let intel_prefix = PathBuf::from("/usr/local");
        if intel_prefix.join("Cellar").exists() {
            return Some(intel_prefix);
        }
    }
    None
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Test: Adapter trait interface completeness
    // =========================================================================

    #[test]
    fn test_all_adapters_have_language() {
        let registry = LspServerRegistry::new();
        let languages: Vec<&str> = registry.supported_languages();
        assert!(languages.contains(&"rust"));
        assert!(languages.contains(&"python"));
        assert!(languages.contains(&"go"));
        assert!(languages.contains(&"typescript"));
        assert!(languages.contains(&"java"));
        assert_eq!(languages.len(), 5);
    }

    #[test]
    fn test_adapter_server_names() {
        let registry = LspServerRegistry::new();

        assert_eq!(
            registry.get_adapter("rust").unwrap().server_name(),
            "rust-analyzer"
        );
        assert_eq!(
            registry.get_adapter("python").unwrap().server_name(),
            "pyright"
        );
        assert_eq!(registry.get_adapter("go").unwrap().server_name(), "gopls");
        assert_eq!(
            registry.get_adapter("typescript").unwrap().server_name(),
            "vtsls"
        );
        assert_eq!(registry.get_adapter("java").unwrap().server_name(), "jdtls");
    }

    #[test]
    fn test_get_adapter_returns_none_for_unknown() {
        let registry = LspServerRegistry::new();
        assert!(registry.get_adapter("cobol").is_none());
        assert!(registry.get_adapter("").is_none());
    }

    // =========================================================================
    // Test: Command and args
    // =========================================================================

    #[test]
    fn test_rust_analyzer_command() {
        let adapter = RustAnalyzerAdapter;
        let (cmd, args) = adapter.command();
        assert_eq!(cmd, "rust-analyzer");
        assert!(args.is_empty());
    }

    #[test]
    fn test_pyright_command() {
        let adapter = PyrightAdapter;
        let (cmd, args) = adapter.command();
        assert_eq!(cmd, "pyright-langserver");
        assert_eq!(args, vec!["--stdio"]);
    }

    #[test]
    fn test_gopls_command() {
        let adapter = GoplsAdapter;
        let (cmd, args) = adapter.command();
        assert_eq!(cmd, "gopls");
        assert_eq!(args, vec!["serve"]);
    }

    #[test]
    fn test_vtsls_command() {
        let adapter = VtslsAdapter;
        let (cmd, args) = adapter.command();
        assert_eq!(cmd, "vtsls");
        assert_eq!(args, vec!["--stdio"]);
    }

    // =========================================================================
    // Test: Detection caching
    // =========================================================================

    #[test]
    fn test_detection_caching() {
        let registry = LspServerRegistry::new();

        // First call runs detection
        let result1 = registry.detect_all();

        // Second call should return cached results
        let result2 = registry.detect_all();

        // Results should be the same
        assert_eq!(result1, result2);

        // Verify cache is populated
        let cache = registry.detected.read().unwrap();
        assert!(cache.is_some());
    }

    #[test]
    fn test_clear_cache() {
        let registry = LspServerRegistry::new();

        // Populate cache
        let _ = registry.detect_all();
        assert!(registry.detected.read().unwrap().is_some());

        // Clear cache
        registry.clear_cache();
        assert!(registry.detected.read().unwrap().is_none());
    }

    // =========================================================================
    // Test: is_detected returns false for undetected language
    // =========================================================================

    #[test]
    fn test_is_detected_false_before_detection() {
        let registry = LspServerRegistry::new();
        // Before detect_all, nothing is detected
        assert!(!registry.is_detected("rust"));
        assert!(!registry.is_detected("python"));
    }

    // =========================================================================
    // Test: Init options
    // =========================================================================

    #[test]
    fn test_init_options_default_none() {
        let registry = LspServerRegistry::new();
        for lang in registry.supported_languages() {
            let adapter = registry.get_adapter(lang).unwrap();
            // Currently all adapters return None for init_options
            assert!(
                adapter.init_options().is_none(),
                "Adapter for {} should have no init_options by default",
                lang
            );
        }
    }

    // =========================================================================
    // Test: which_binary for known binary
    // =========================================================================

    #[test]
    fn test_which_binary_finds_ls() {
        // `ls` should be available on any UNIX system
        if cfg!(unix) {
            let result = which_binary("ls");
            assert!(result.is_some(), "ls should be found in PATH on UNIX");
        }
    }

    #[test]
    fn test_which_binary_not_found() {
        let result = which_binary("definitely_nonexistent_binary_xyz_123");
        assert!(result.is_none());
    }
}
