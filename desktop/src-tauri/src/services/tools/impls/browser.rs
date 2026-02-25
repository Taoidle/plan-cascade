//! Browser Automation Tool
//!
//! Provides headless browser automation capabilities for agents.
//! Types and trait are defined unconditionally; the actual browser
//! implementation is gated behind `#[cfg(feature = "browser")]` to
//! avoid pulling in heavy chromiumoxide dependencies by default.
//!
//! ## Tools
//! - `navigate(url)` - Navigate to a URL
//! - `click(selector)` - Click an element matching a CSS selector
//! - `type_text(selector, text)` - Type text into an input element
//! - `screenshot()` - Take a screenshot of the current page
//! - `extract_text(selector)` - Extract text content from an element
//! - `wait_for(selector, timeout)` - Wait for an element to appear
//!
//! ## Architecture
//! - BrowserAction/BrowserActionResult: unconditional types
//! - BrowserBackend: feature-gated (#[cfg(feature = "browser")]) backend
//!   using chromiumoxide (ADR-002) with lazy initialization
//! - BrowserTool: unconditional Tool trait impl, delegates to BrowserBackend
//!   when the browser feature is enabled

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

// ============================================================================
// Browser Action Types (unconditional)
// ============================================================================

/// Actions supported by the browser tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BrowserAction {
    /// Navigate to a URL.
    Navigate {
        /// Target URL to navigate to.
        url: String,
    },
    /// Click an element matching a CSS selector.
    Click {
        /// CSS selector for the target element.
        selector: String,
    },
    /// Type text into an input element.
    TypeText {
        /// CSS selector for the input element.
        selector: String,
        /// Text to type.
        text: String,
    },
    /// Take a screenshot of the current page.
    Screenshot,
    /// Extract text content from elements matching a CSS selector.
    ExtractText {
        /// CSS selector for the target element(s).
        selector: String,
    },
    /// Wait for an element matching a CSS selector to appear.
    WaitFor {
        /// CSS selector to wait for.
        selector: String,
        /// Maximum wait time in milliseconds (default: 5000).
        #[serde(default = "default_timeout")]
        timeout_ms: u64,
    },
}

fn default_timeout() -> u64 {
    5000
}

/// Result of a browser action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserActionResult {
    /// Whether the action succeeded.
    pub success: bool,
    /// Output data (e.g., extracted text, screenshot path).
    pub output: Option<String>,
    /// Current page URL after the action.
    pub current_url: Option<String>,
    /// Current page title after the action.
    pub page_title: Option<String>,
}

// ============================================================================
// Runtime Browser Detection (unconditional)
// ============================================================================

/// Detect an installed Chrome or Chromium browser at runtime.
///
/// Checks common installation paths on macOS, Linux, and Windows.
/// Returns the path to the browser executable if found, or `None` if
/// no browser could be located.
///
/// This replaces compile-time feature gating for browser availability,
/// allowing BrowserTool to always be registered and provide helpful
/// errors when no browser is found.
pub fn detect_browser() -> Option<PathBuf> {
    let candidates = get_browser_candidate_paths();
    for path in candidates {
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Get a list of candidate Chrome/Chromium executable paths for the current platform.
fn get_browser_candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "macos")]
    {
        paths.push(PathBuf::from(
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        ));
        paths.push(PathBuf::from(
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
        ));
        paths.push(PathBuf::from(
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        ));
        paths.push(PathBuf::from(
            "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
        ));
        paths.push(PathBuf::from(
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ));
        // User-level installations
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join("Applications/Google Chrome.app/Contents/MacOS/Google Chrome"));
            paths.push(home.join("Applications/Chromium.app/Contents/MacOS/Chromium"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        paths.push(PathBuf::from("/usr/bin/google-chrome"));
        paths.push(PathBuf::from("/usr/bin/google-chrome-stable"));
        paths.push(PathBuf::from("/usr/bin/chromium-browser"));
        paths.push(PathBuf::from("/usr/bin/chromium"));
        paths.push(PathBuf::from("/usr/local/bin/google-chrome"));
        paths.push(PathBuf::from("/usr/local/bin/chromium"));
        paths.push(PathBuf::from("/snap/bin/chromium"));
    }

    #[cfg(target_os = "windows")]
    {
        // Standard installation paths
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            paths.push(PathBuf::from(format!(
                "{}\\Google\\Chrome\\Application\\chrome.exe",
                program_files
            )));
            paths.push(PathBuf::from(format!(
                "{}\\Chromium\\Application\\chrome.exe",
                program_files
            )));
            paths.push(PathBuf::from(format!(
                "{}\\Microsoft\\Edge\\Application\\msedge.exe",
                program_files
            )));
            paths.push(PathBuf::from(format!(
                "{}\\BraveSoftware\\Brave-Browser\\Application\\brave.exe",
                program_files
            )));
        }
        if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
            paths.push(PathBuf::from(format!(
                "{}\\Google\\Chrome\\Application\\chrome.exe",
                program_files_x86
            )));
            paths.push(PathBuf::from(format!(
                "{}\\Chromium\\Application\\chrome.exe",
                program_files_x86
            )));
        }
        // Per-user installation
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            paths.push(PathBuf::from(format!(
                "{}\\Google\\Chrome\\Application\\chrome.exe",
                local_app_data
            )));
            paths.push(PathBuf::from(format!(
                "{}\\Chromium\\Application\\chrome.exe",
                local_app_data
            )));
        }
    }

    paths
}

/// Check whether browser automation is available.
///
/// Returns a status struct indicating whether the `browser` feature is compiled in
/// and whether a Chrome/Chromium binary was found at runtime.
pub fn browser_availability() -> BrowserAvailability {
    let browser_path = detect_browser();
    let feature_compiled = cfg!(feature = "browser");
    BrowserAvailability {
        feature_compiled,
        browser_detected: browser_path.is_some(),
        browser_path,
    }
}

/// Browser automation availability status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserAvailability {
    /// Whether the `browser` Cargo feature was enabled at compile time.
    pub feature_compiled: bool,
    /// Whether a Chrome/Chromium binary was found at runtime.
    pub browser_detected: bool,
    /// Path to the detected browser binary (if any).
    pub browser_path: Option<PathBuf>,
}

impl BrowserAvailability {
    /// Whether browser automation is fully available (feature compiled AND browser found).
    pub fn is_available(&self) -> bool {
        self.feature_compiled && self.browser_detected
    }

    /// Human-readable status message.
    pub fn status_message(&self) -> String {
        match (self.feature_compiled, self.browser_detected) {
            (true, true) => format!(
                "Browser automation available ({})",
                self.browser_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            ),
            (true, false) => "Browser feature compiled, but no Chrome/Chromium found. \
                Install Google Chrome or Chromium to enable browser automation."
                .to_string(),
            (false, true) => format!(
                "Chrome/Chromium detected at {}, but browser feature not compiled. \
                 Rebuild with `--features browser` to enable browser automation.",
                self.browser_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            ),
            (false, false) => "Browser automation unavailable. Install Chrome/Chromium \
                and rebuild with `--features browser`."
                .to_string(),
        }
    }
}

// ============================================================================
// BrowserBackend (feature-gated: requires "browser" feature)
// ============================================================================

#[cfg(feature = "browser")]
mod backend {
    use super::*;
    use base64::Engine;
    use chromiumoxide::browser::{Browser, BrowserConfig};
    use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
    use chromiumoxide::page::ScreenshotParams;
    use futures::StreamExt;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tokio::task::JoinHandle;
    use tracing::{debug, info, warn};

    /// Internal state for an active browser session.
    struct BrowserState {
        /// The chromiumoxide Browser handle.
        browser: Browser,
        /// The active page/tab.
        page: chromiumoxide::Page,
        /// Handle for the CDP handler task.
        _handler_handle: JoinHandle<()>,
    }

    /// Browser automation backend using chromiumoxide (CDP-native, async).
    ///
    /// Implements the Lazy Service Initialization pattern: the headless
    /// Chrome process is only started when the first browser action is
    /// requested. The browser state is stored behind `Arc<Mutex<Option<...>>>`
    /// for thread-safe, async-compatible lazy init.
    pub(super) struct BrowserBackend {
        /// Lazily initialized browser state. `None` means not yet started.
        state: Arc<Mutex<Option<BrowserState>>>,
    }

    impl BrowserBackend {
        /// Create a new BrowserBackend (no browser process started yet).
        pub fn new() -> Self {
            Self {
                state: Arc::new(Mutex::new(None)),
            }
        }

        /// Ensure the browser is initialized. Returns a guard holding the lock.
        /// If the browser has not been started yet, launches a headless Chrome
        /// instance and creates a new page.
        async fn ensure_initialized(
            &self,
        ) -> Result<tokio::sync::MutexGuard<'_, Option<BrowserState>>, String> {
            let mut guard = self.state.lock().await;

            if guard.is_none() {
                info!("BrowserBackend: Launching headless Chrome...");

                let config = BrowserConfig::builder()
                    .no_sandbox()
                    .arg("--disable-gpu")
                    .arg("--disable-dev-shm-usage")
                    .arg("--disable-extensions")
                    .window_size(1280, 720)
                    .build()
                    .map_err(|e| format!("Failed to build browser config: {}", e))?;

                let (browser, mut handler) = Browser::launch(config)
                    .await
                    .map_err(|e| format!("Failed to launch browser: {}", e))?;

                // Spawn the CDP handler task. This task processes WebSocket
                // messages between our code and the Chrome DevTools Protocol.
                let handler_handle = tokio::spawn(async move {
                    while let Some(event) = handler.next().await {
                        if event.is_err() {
                            debug!("BrowserBackend: CDP handler event loop ended");
                            break;
                        }
                    }
                });

                let page = browser
                    .new_page("about:blank")
                    .await
                    .map_err(|e| format!("Failed to create browser page: {}", e))?;

                info!("BrowserBackend: Headless Chrome launched successfully");

                *guard = Some(BrowserState {
                    browser,
                    page,
                    _handler_handle: handler_handle,
                });
            }

            Ok(guard)
        }

        /// Execute a browser action. Lazily initializes the browser on first call.
        pub async fn execute_action(
            &self,
            action: &BrowserAction,
        ) -> Result<BrowserActionResult, String> {
            let mut guard = self.ensure_initialized().await?;
            let state = guard.as_mut().ok_or_else(|| {
                "Browser state unexpectedly None after initialization".to_string()
            })?;

            match action {
                BrowserAction::Navigate { url } => {
                    Self::action_navigate(&mut state.page, url).await
                }
                BrowserAction::Click { selector } => {
                    Self::action_click(&mut state.page, selector).await
                }
                BrowserAction::TypeText { selector, text } => {
                    Self::action_type_text(&mut state.page, selector, text).await
                }
                BrowserAction::Screenshot => Self::action_screenshot(&mut state.page).await,
                BrowserAction::ExtractText { selector } => {
                    Self::action_extract_text(&mut state.page, selector).await
                }
                BrowserAction::WaitFor {
                    selector,
                    timeout_ms,
                } => Self::action_wait_for(&mut state.page, selector, *timeout_ms).await,
            }
        }

        /// Execute a screenshot action and return the raw PNG bytes as well.
        /// Used by BrowserTool to provide multimodal image data.
        pub async fn execute_screenshot_raw(
            &self,
        ) -> Result<(BrowserActionResult, Vec<u8>), String> {
            let mut guard = self.ensure_initialized().await?;
            let state = guard.as_mut().ok_or_else(|| {
                "Browser state unexpectedly None after initialization".to_string()
            })?;

            let page = &mut state.page;

            let screenshot_bytes = page
                .screenshot(
                    ScreenshotParams::builder()
                        .format(CaptureScreenshotFormat::Png)
                        .full_page(false)
                        .build(),
                )
                .await
                .map_err(|e| format!("Screenshot failed: {}", e))?;

            let url = page
                .url()
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "unknown".to_string());
            let title = page
                .evaluate("document.title")
                .await
                .ok()
                .and_then(|v| v.into_value::<String>().ok())
                .unwrap_or_else(|| "untitled".to_string());

            let size = screenshot_bytes.len();
            let result = BrowserActionResult {
                success: true,
                output: Some(format!("Screenshot captured ({} bytes, PNG format)", size)),
                current_url: Some(url),
                page_title: Some(title),
            };

            Ok((result, screenshot_bytes))
        }

        /// Shut down the browser process.
        pub async fn cleanup(&self) {
            let mut guard = self.state.lock().await;
            if let Some(mut state) = guard.take() {
                info!("BrowserBackend: Shutting down browser...");
                if let Err(e) = state.browser.close().await {
                    warn!("BrowserBackend: Error closing browser: {}", e);
                }
                info!("BrowserBackend: Browser shut down");
            }
        }

        // ── Action Implementations ──────────────────────────────────────

        /// Navigate to a URL and wait for the page to load.
        async fn action_navigate(
            page: &mut chromiumoxide::Page,
            url: &str,
        ) -> Result<BrowserActionResult, String> {
            // SSRF prevention: validate URL before navigation
            crate::services::tools::url_validation::validate_url_ssrf(url)
                .await
                .map_err(|e| format!("Navigation blocked: {}", e))?;

            debug!("BrowserBackend: Navigating to {}", url);

            page.goto(url)
                .await
                .map_err(|e| format!("Navigation to '{}' failed: {}", url, e))?;

            // Get page metadata after navigation
            let current_url = page
                .url()
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| url.to_string());
            let page_title = page
                .evaluate("document.title")
                .await
                .ok()
                .and_then(|v| v.into_value::<String>().ok());

            // Extract a brief text summary of the page content
            let body_text = page
                .evaluate("document.body ? document.body.innerText.substring(0, 500) : ''")
                .await
                .ok()
                .and_then(|v| v.into_value::<String>().ok())
                .unwrap_or_default();

            let output = format!(
                "Navigated to {}\nTitle: {}\nContent preview:\n{}",
                current_url,
                page_title.as_deref().unwrap_or("(no title)"),
                if body_text.is_empty() {
                    "(empty page)"
                } else {
                    &body_text
                }
            );

            Ok(BrowserActionResult {
                success: true,
                output: Some(output),
                current_url: Some(current_url),
                page_title,
            })
        }

        /// Click an element matching a CSS selector.
        async fn action_click(
            page: &mut chromiumoxide::Page,
            selector: &str,
        ) -> Result<BrowserActionResult, String> {
            debug!("BrowserBackend: Clicking element '{}'", selector);

            let element = page
                .find_element(selector)
                .await
                .map_err(|e| format!("Element '{}' not found: {}", selector, e))?;

            element
                .click()
                .await
                .map_err(|e| format!("Click on '{}' failed: {}", selector, e))?;

            // Brief pause to allow any navigation/JS to settle
            tokio::time::sleep(Duration::from_millis(100)).await;

            let current_url = page.url().await.ok().flatten();
            let page_title = page
                .evaluate("document.title")
                .await
                .ok()
                .and_then(|v| v.into_value::<String>().ok());

            Ok(BrowserActionResult {
                success: true,
                output: Some(format!("Clicked element matching '{}'", selector)),
                current_url,
                page_title,
            })
        }

        /// Type text into an input element.
        async fn action_type_text(
            page: &mut chromiumoxide::Page,
            selector: &str,
            text: &str,
        ) -> Result<BrowserActionResult, String> {
            debug!(
                "BrowserBackend: Typing into '{}': '{}'",
                selector,
                if text.len() > 20 {
                    format!("{}...", &text[..20])
                } else {
                    text.to_string()
                }
            );

            let element = page
                .find_element(selector)
                .await
                .map_err(|e| format!("Input element '{}' not found: {}", selector, e))?;

            // Click to focus, then type
            element
                .click()
                .await
                .map_err(|e| format!("Failed to focus input '{}': {}", selector, e))?;

            element
                .type_str(text)
                .await
                .map_err(|e| format!("Failed to type into '{}': {}", selector, e))?;

            let current_url = page.url().await.ok().flatten();
            let page_title = page
                .evaluate("document.title")
                .await
                .ok()
                .and_then(|v| v.into_value::<String>().ok());

            Ok(BrowserActionResult {
                success: true,
                output: Some(format!(
                    "Typed {} characters into '{}'",
                    text.len(),
                    selector
                )),
                current_url,
                page_title,
            })
        }

        /// Take a screenshot of the current viewport.
        async fn action_screenshot(
            page: &mut chromiumoxide::Page,
        ) -> Result<BrowserActionResult, String> {
            debug!("BrowserBackend: Taking screenshot");

            let screenshot_bytes = page
                .screenshot(
                    ScreenshotParams::builder()
                        .format(CaptureScreenshotFormat::Png)
                        .full_page(false)
                        .build(),
                )
                .await
                .map_err(|e| format!("Screenshot failed: {}", e))?;

            let current_url = page.url().await.ok().flatten();
            let page_title = page
                .evaluate("document.title")
                .await
                .ok()
                .and_then(|v| v.into_value::<String>().ok());

            let size = screenshot_bytes.len();
            let base64_data = base64::engine::general_purpose::STANDARD.encode(&screenshot_bytes);

            Ok(BrowserActionResult {
                success: true,
                output: Some(format!(
                    "Screenshot captured ({} bytes, PNG, base64 length: {})",
                    size,
                    base64_data.len()
                )),
                current_url,
                page_title,
            })
        }

        /// Extract text content from elements matching a CSS selector.
        async fn action_extract_text(
            page: &mut chromiumoxide::Page,
            selector: &str,
        ) -> Result<BrowserActionResult, String> {
            debug!("BrowserBackend: Extracting text from '{}'", selector);

            let elements = page
                .find_elements(selector)
                .await
                .map_err(|e| format!("Elements '{}' not found: {}", selector, e))?;

            if elements.is_empty() {
                return Err(format!(
                    "No elements found matching selector '{}'",
                    selector
                ));
            }

            let mut texts = Vec::new();
            for element in &elements {
                if let Ok(text) = element.inner_text().await {
                    if let Some(t) = text {
                        texts.push(t);
                    }
                }
            }

            let current_url = page.url().await.ok().flatten();
            let page_title = page
                .evaluate("document.title")
                .await
                .ok()
                .and_then(|v| v.into_value::<String>().ok());

            let combined_text = texts.join("\n---\n");
            Ok(BrowserActionResult {
                success: true,
                output: Some(format!(
                    "Extracted text from {} element(s) matching '{}':\n{}",
                    elements.len(),
                    selector,
                    combined_text
                )),
                current_url,
                page_title,
            })
        }

        /// Wait for an element matching a CSS selector to appear.
        async fn action_wait_for(
            page: &mut chromiumoxide::Page,
            selector: &str,
            timeout_ms: u64,
        ) -> Result<BrowserActionResult, String> {
            debug!(
                "BrowserBackend: Waiting for '{}' (timeout: {}ms)",
                selector, timeout_ms
            );

            let timeout = Duration::from_millis(timeout_ms);
            let poll_interval = Duration::from_millis(100);
            let start = std::time::Instant::now();

            loop {
                // Try to find the element
                match page.find_element(selector).await {
                    Ok(_element) => {
                        let elapsed = start.elapsed().as_millis();
                        let current_url = page.url().await.ok().flatten();
                        let page_title = page
                            .evaluate("document.title")
                            .await
                            .ok()
                            .and_then(|v| v.into_value::<String>().ok());

                        return Ok(BrowserActionResult {
                            success: true,
                            output: Some(format!(
                                "Element '{}' found after {}ms",
                                selector, elapsed
                            )),
                            current_url,
                            page_title,
                        });
                    }
                    Err(_) => {
                        if start.elapsed() >= timeout {
                            return Err(format!(
                                "Timeout waiting for element '{}' after {}ms",
                                selector, timeout_ms
                            ));
                        }
                        tokio::time::sleep(poll_interval).await;
                    }
                }
            }
        }
    }

    impl Drop for BrowserBackend {
        fn drop(&mut self) {
            // We cannot do async cleanup in Drop, but the browser process
            // should terminate when the Browser handle is dropped by
            // chromiumoxide's own cleanup logic.
            debug!("BrowserBackend: Dropping (browser process will be cleaned up)");
        }
    }
}

// ============================================================================
// BrowserTool (unconditional struct, feature-gated internals)
// ============================================================================

/// Browser automation tool that wraps headless browser functionality.
///
/// Uses lazy initialization to avoid starting the browser process
/// until the first action is requested. The actual browser instance
/// is gated behind `#[cfg(feature = "browser")]`.
pub struct BrowserTool {
    /// Whether the tool has been lazily initialized.
    _initialized: std::sync::atomic::AtomicBool,
    /// The browser backend (only present when feature "browser" is enabled).
    #[cfg(feature = "browser")]
    backend: backend::BrowserBackend,
}

impl BrowserTool {
    /// Create a new BrowserTool (lazy initialization).
    pub fn new() -> Self {
        Self {
            _initialized: std::sync::atomic::AtomicBool::new(false),
            #[cfg(feature = "browser")]
            backend: backend::BrowserBackend::new(),
        }
    }

    /// Parse a BrowserAction from tool arguments.
    fn parse_action(args: &Value) -> Result<BrowserAction, String> {
        let action_str = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required 'action' parameter".to_string())?;

        match action_str {
            "navigate" => {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'url' parameter for navigate action".to_string())?;
                Ok(BrowserAction::Navigate {
                    url: url.to_string(),
                })
            }
            "click" => {
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'selector' parameter for click action".to_string())?;
                Ok(BrowserAction::Click {
                    selector: selector.to_string(),
                })
            }
            "type_text" => {
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        "Missing 'selector' parameter for type_text action".to_string()
                    })?;
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        "Missing 'text' parameter for type_text action".to_string()
                    })?;
                Ok(BrowserAction::TypeText {
                    selector: selector.to_string(),
                    text: text.to_string(),
                })
            }
            "screenshot" => Ok(BrowserAction::Screenshot),
            "extract_text" => {
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        "Missing 'selector' parameter for extract_text action".to_string()
                    })?;
                Ok(BrowserAction::ExtractText {
                    selector: selector.to_string(),
                })
            }
            "wait_for" => {
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        "Missing 'selector' parameter for wait_for action".to_string()
                    })?;
                let timeout_ms = args
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(default_timeout());
                Ok(BrowserAction::WaitFor {
                    selector: selector.to_string(),
                    timeout_ms,
                })
            }
            other => Err(format!(
                "Unknown action '{}'. Supported: navigate, click, type_text, screenshot, extract_text, wait_for",
                other
            )),
        }
    }

    /// Shut down the browser backend (if initialized).
    ///
    /// This is a graceful cleanup method. If the browser feature is not
    /// enabled, this is a no-op.
    #[allow(dead_code)]
    pub async fn cleanup(&self) {
        #[cfg(feature = "browser")]
        {
            self.backend.cleanup().await;
        }
    }
}

impl Default for BrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "Browser"
    }

    fn description(&self) -> &str {
        "Headless browser automation tool. Supports actions: navigate(url), click(selector), \
         type_text(selector, text), screenshot(), extract_text(selector), wait_for(selector, timeout_ms). \
         Uses runtime detection to find Chrome/Chromium. Returns a helpful error if no browser is available."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            ParameterSchema::string(Some(
                "The browser action: navigate, click, type_text, screenshot, extract_text, wait_for",
            )),
        );
        properties.insert(
            "url".to_string(),
            ParameterSchema::string(Some("URL to navigate to (for 'navigate' action)")),
        );
        properties.insert(
            "selector".to_string(),
            ParameterSchema::string(Some(
                "CSS selector for the target element (for click, type_text, extract_text, wait_for)",
            )),
        );
        properties.insert(
            "text".to_string(),
            ParameterSchema::string(Some("Text to type (for 'type_text' action)")),
        );
        properties.insert(
            "timeout_ms".to_string(),
            ParameterSchema::integer(Some("Max wait time in ms (for 'wait_for', default: 5000)")),
        );

        ParameterSchema::object(
            Some("Browser automation parameters"),
            properties,
            vec!["action".to_string()],
        )
    }

    fn is_long_running(&self) -> bool {
        true
    }

    async fn execute(&self, _ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        // Parse the action from arguments
        let action = match Self::parse_action(&args) {
            Ok(a) => a,
            Err(e) => return ToolResult::err(e),
        };

        let action_name = match &action {
            BrowserAction::Navigate { .. } => "navigate",
            BrowserAction::Click { .. } => "click",
            BrowserAction::TypeText { .. } => "type_text",
            BrowserAction::Screenshot => "screenshot",
            BrowserAction::ExtractText { .. } => "extract_text",
            BrowserAction::WaitFor { .. } => "wait_for",
        };

        // Step 1: Check runtime browser availability
        let availability = browser_availability();
        if !availability.browser_detected {
            return ToolResult::err(format!(
                "Browser action '{}' failed: No Chrome or Chromium browser found on this system. \
                 Please install Google Chrome or Chromium to use browser automation.\n\
                 Checked paths for {} platform.",
                action_name,
                std::env::consts::OS
            ));
        }

        // Step 2: Check if the browser feature is compiled in
        #[cfg(feature = "browser")]
        {
            // For screenshot actions, use the raw variant to get base64 image data
            if matches!(action, BrowserAction::Screenshot) {
                match self.backend.execute_screenshot_raw().await {
                    Ok((result, png_bytes)) => {
                        let base64_data = base64::Engine::encode(
                            &base64::engine::general_purpose::STANDARD,
                            &png_bytes,
                        );
                        let output = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| format!("{:?}", result));
                        return ToolResult::ok_with_image(
                            output,
                            "image/png".to_string(),
                            base64_data,
                        );
                    }
                    Err(e) => return ToolResult::err(e),
                }
            }

            // For all other actions, use the standard execute path
            match self.backend.execute_action(&action).await {
                Ok(result) => {
                    let output = serde_json::to_string_pretty(&result)
                        .unwrap_or_else(|_| format!("{:?}", result));
                    return ToolResult::ok(output);
                }
                Err(e) => return ToolResult::err(e),
            }
        }

        #[cfg(not(feature = "browser"))]
        {
            // Browser found at runtime but feature not compiled in
            ToolResult::err(format!(
                "Browser action '{}' failed: Chrome/Chromium detected at '{}', but the 'browser' \
                 feature was not compiled in. Rebuild with `--features browser` to enable \
                 browser automation.",
                action_name,
                availability
                    .browser_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ))
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::test_helpers::make_test_ctx;
    use super::*;
    use std::path::Path;

    fn make_ctx() -> ToolExecutionContext {
        make_test_ctx(Path::new("/tmp/test"))
    }

    // ── Runtime detection tests ──────────────────────────────────────

    #[test]
    fn test_detect_browser_returns_option() {
        // detect_browser() should not panic regardless of platform/environment
        let result = detect_browser();
        // On dev machines Chrome is typically installed; in CI it may not be.
        // Just verify the function runs without panicking and returns Option.
        let _ = result;
    }

    #[test]
    fn test_get_browser_candidate_paths_not_empty() {
        let paths = get_browser_candidate_paths();
        // Every platform should have at least one candidate path
        assert!(
            !paths.is_empty(),
            "Candidate browser paths should not be empty for any platform"
        );
    }

    #[test]
    fn test_browser_availability_struct() {
        let avail = browser_availability();
        // Verify all fields are populated
        let _ = avail.feature_compiled;
        let _ = avail.browser_detected;
        let _ = avail.browser_path;
    }

    #[test]
    fn test_browser_availability_is_available() {
        let avail = BrowserAvailability {
            feature_compiled: true,
            browser_detected: true,
            browser_path: Some(PathBuf::from("/usr/bin/chromium")),
        };
        assert!(avail.is_available());

        let avail_no_feature = BrowserAvailability {
            feature_compiled: false,
            browser_detected: true,
            browser_path: Some(PathBuf::from("/usr/bin/chromium")),
        };
        assert!(!avail_no_feature.is_available());

        let avail_no_browser = BrowserAvailability {
            feature_compiled: true,
            browser_detected: false,
            browser_path: None,
        };
        assert!(!avail_no_browser.is_available());

        let avail_neither = BrowserAvailability {
            feature_compiled: false,
            browser_detected: false,
            browser_path: None,
        };
        assert!(!avail_neither.is_available());
    }

    #[test]
    fn test_browser_availability_status_message_all_cases() {
        let avail_both = BrowserAvailability {
            feature_compiled: true,
            browser_detected: true,
            browser_path: Some(PathBuf::from("/usr/bin/chromium")),
        };
        assert!(avail_both.status_message().contains("available"));

        let avail_feature_only = BrowserAvailability {
            feature_compiled: true,
            browser_detected: false,
            browser_path: None,
        };
        assert!(avail_feature_only.status_message().contains("Install"));

        let avail_browser_only = BrowserAvailability {
            feature_compiled: false,
            browser_detected: true,
            browser_path: Some(PathBuf::from("/usr/bin/chromium")),
        };
        assert!(avail_browser_only
            .status_message()
            .contains("--features browser"));

        let avail_neither = BrowserAvailability {
            feature_compiled: false,
            browser_detected: false,
            browser_path: None,
        };
        assert!(avail_neither.status_message().contains("unavailable"));
    }

    #[test]
    fn test_browser_availability_serde() {
        let avail = BrowserAvailability {
            feature_compiled: true,
            browser_detected: true,
            browser_path: Some(PathBuf::from("/usr/bin/google-chrome")),
        };
        let json = serde_json::to_string(&avail).unwrap();
        let parsed: BrowserAvailability = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.feature_compiled, true);
        assert_eq!(parsed.browser_detected, true);
        assert_eq!(
            parsed.browser_path,
            Some(PathBuf::from("/usr/bin/google-chrome"))
        );
    }

    // ── Tool identity tests ──────────────────────────────────────────

    #[test]
    fn test_browser_tool_name() {
        let tool = BrowserTool::new();
        assert_eq!(tool.name(), "Browser");
    }

    #[test]
    fn test_browser_tool_description() {
        let tool = BrowserTool::new();
        assert!(tool.description().contains("browser automation"));
    }

    #[test]
    fn test_browser_tool_description_mentions_runtime_detection() {
        let tool = BrowserTool::new();
        assert!(
            tool.description().contains("runtime detection"),
            "Description should mention runtime detection"
        );
    }

    #[test]
    fn test_browser_tool_is_long_running() {
        let tool = BrowserTool::new();
        assert!(tool.is_long_running());
    }

    #[test]
    fn test_browser_tool_default() {
        let tool = BrowserTool::default();
        assert_eq!(tool.name(), "Browser");
    }

    #[test]
    fn test_browser_tool_always_registerable() {
        // BrowserTool should always be constructable and implement Tool,
        // regardless of feature flags
        let tool = BrowserTool::new();
        assert_eq!(tool.name(), "Browser");
        assert!(!tool.description().is_empty());
        let schema = tool.parameters_schema();
        let json = serde_json::to_value(&schema).unwrap();
        assert!(json.get("properties").is_some());
    }

    #[test]
    fn test_browser_tool_registered_in_registry() {
        // Verify BrowserTool is in the static registry
        let registry = crate::services::tools::executor::ToolExecutor::build_registry_static();
        let browser = registry.get("Browser");
        assert!(
            browser.is_some(),
            "BrowserTool should be registered in the tool registry"
        );
    }

    // ── Action parsing tests ─────────────────────────────────────────

    #[test]
    fn test_parse_navigate_action() {
        let args = serde_json::json!({
            "action": "navigate",
            "url": "https://example.com"
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::Navigate { url } => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected Navigate"),
        }
    }

    #[test]
    fn test_parse_click_action() {
        let args = serde_json::json!({
            "action": "click",
            "selector": "#submit-btn"
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::Click { selector } => assert_eq!(selector, "#submit-btn"),
            _ => panic!("Expected Click"),
        }
    }

    #[test]
    fn test_parse_type_text_action() {
        let args = serde_json::json!({
            "action": "type_text",
            "selector": "input[name='email']",
            "text": "test@example.com"
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::TypeText { selector, text } => {
                assert_eq!(selector, "input[name='email']");
                assert_eq!(text, "test@example.com");
            }
            _ => panic!("Expected TypeText"),
        }
    }

    #[test]
    fn test_parse_screenshot_action() {
        let args = serde_json::json!({"action": "screenshot"});
        let action = BrowserTool::parse_action(&args).unwrap();
        assert!(matches!(action, BrowserAction::Screenshot));
    }

    #[test]
    fn test_parse_extract_text_action() {
        let args = serde_json::json!({
            "action": "extract_text",
            "selector": ".main-content"
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::ExtractText { selector } => assert_eq!(selector, ".main-content"),
            _ => panic!("Expected ExtractText"),
        }
    }

    #[test]
    fn test_parse_wait_for_action() {
        let args = serde_json::json!({
            "action": "wait_for",
            "selector": ".loaded",
            "timeout_ms": 10000
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::WaitFor {
                selector,
                timeout_ms,
            } => {
                assert_eq!(selector, ".loaded");
                assert_eq!(timeout_ms, 10000);
            }
            _ => panic!("Expected WaitFor"),
        }
    }

    #[test]
    fn test_parse_wait_for_default_timeout() {
        let args = serde_json::json!({
            "action": "wait_for",
            "selector": ".loaded"
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::WaitFor { timeout_ms, .. } => {
                assert_eq!(timeout_ms, 5000);
            }
            _ => panic!("Expected WaitFor"),
        }
    }

    #[test]
    fn test_parse_unknown_action() {
        let args = serde_json::json!({"action": "fly_to_moon"});
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown action"));
    }

    #[test]
    fn test_parse_missing_action() {
        let args = serde_json::json!({"url": "https://example.com"});
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required 'action'"));
    }

    #[test]
    fn test_parse_navigate_missing_url() {
        let args = serde_json::json!({"action": "navigate"});
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'url'"));
    }

    #[test]
    fn test_parse_click_missing_selector() {
        let args = serde_json::json!({"action": "click"});
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'selector'"));
    }

    #[test]
    fn test_parse_type_text_missing_text() {
        let args = serde_json::json!({
            "action": "type_text",
            "selector": "#input"
        });
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'text'"));
    }

    // ── Execution tests (runtime detection) ──────────────────────────

    #[tokio::test]
    async fn test_execute_returns_error_not_panic() {
        // The tool should never panic, regardless of browser availability
        let tool = BrowserTool::new();
        let ctx = make_ctx();
        let args = serde_json::json!({
            "action": "navigate",
            "url": "https://example.com"
        });
        let result = tool.execute(&ctx, args).await;
        // On machines without Chrome, result.success should be false
        // On machines with Chrome but without browser feature, result.success should be false
        // On machines with Chrome AND browser feature, it might succeed or fail (browser launch)
        // In all cases: no panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_execute_graceful_error_message() {
        // When neither feature nor browser is available, we get a clear error
        let tool = BrowserTool::new();
        let ctx = make_ctx();
        let args = serde_json::json!({
            "action": "navigate",
            "url": "https://example.com"
        });
        let result = tool.execute(&ctx, args).await;

        // Only check error content when we know the result was an error
        if !result.success {
            let err = result.error.as_deref().unwrap_or("");
            // Error should mention either "No Chrome" or "not compiled"
            assert!(
                err.contains("Chrome") || err.contains("browser") || err.contains("Chromium"),
                "Error should mention browser: {}",
                err
            );
        }
    }

    #[tokio::test]
    async fn test_execute_with_bad_args() {
        let tool = BrowserTool::new();
        let ctx = make_ctx();
        let args = serde_json::json!({});
        let result = tool.execute(&ctx, args).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Missing required"));
    }

    // ── BrowserAction serialization tests ────────────────────────────

    #[test]
    fn test_browser_action_navigate_serde() {
        let action = BrowserAction::Navigate {
            url: "https://example.com".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"action\":\"navigate\""));
        assert!(json.contains("\"url\":\"https://example.com\""));

        let parsed: BrowserAction = serde_json::from_str(&json).unwrap();
        match parsed {
            BrowserAction::Navigate { url } => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected Navigate"),
        }
    }

    #[test]
    fn test_browser_action_screenshot_serde() {
        let action = BrowserAction::Screenshot;
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"action\":\"screenshot\""));

        let parsed: BrowserAction = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, BrowserAction::Screenshot));
    }

    #[test]
    fn test_browser_action_result_serde() {
        let result = BrowserActionResult {
            success: true,
            output: Some("Page loaded".to_string()),
            current_url: Some("https://example.com".to_string()),
            page_title: Some("Example".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: BrowserActionResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.success);
        assert_eq!(parsed.output, Some("Page loaded".to_string()));
        assert_eq!(parsed.current_url, Some("https://example.com".to_string()));
        assert_eq!(parsed.page_title, Some("Example".to_string()));
    }

    // ── Parameters schema test ───────────────────────────────────────

    #[test]
    fn test_parameters_schema_has_action() {
        let tool = BrowserTool::new();
        let schema = tool.parameters_schema();
        // The schema should have 'action' as required
        let json = serde_json::to_value(&schema).unwrap();
        let required = json.get("required").and_then(|v| v.as_array());
        assert!(required.is_some());
        let required_list: Vec<&str> = required
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(required_list.contains(&"action"));
    }

    // ── BrowserActionResult construction tests ───────────────────────

    #[test]
    fn test_browser_action_result_success() {
        let result = BrowserActionResult {
            success: true,
            output: Some("Navigated successfully".to_string()),
            current_url: Some("https://example.com".to_string()),
            page_title: Some("Example Domain".to_string()),
        };
        assert!(result.success);
        assert_eq!(result.output.as_deref(), Some("Navigated successfully"));
        assert_eq!(result.current_url.as_deref(), Some("https://example.com"));
        assert_eq!(result.page_title.as_deref(), Some("Example Domain"));
    }

    #[test]
    fn test_browser_action_result_failure() {
        let result = BrowserActionResult {
            success: false,
            output: None,
            current_url: None,
            page_title: None,
        };
        assert!(!result.success);
        assert!(result.output.is_none());
        assert!(result.current_url.is_none());
        assert!(result.page_title.is_none());
    }

    // ── BrowserAction variant tests ──────────────────────────────────

    #[test]
    fn test_browser_action_click_serde() {
        let action = BrowserAction::Click {
            selector: "#btn".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"action\":\"click\""));
        assert!(json.contains("\"selector\":\"#btn\""));

        let parsed: BrowserAction = serde_json::from_str(&json).unwrap();
        match parsed {
            BrowserAction::Click { selector } => assert_eq!(selector, "#btn"),
            _ => panic!("Expected Click"),
        }
    }

    #[test]
    fn test_browser_action_type_text_serde() {
        let action = BrowserAction::TypeText {
            selector: "input#search".to_string(),
            text: "hello world".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"action\":\"type_text\""));

        let parsed: BrowserAction = serde_json::from_str(&json).unwrap();
        match parsed {
            BrowserAction::TypeText { selector, text } => {
                assert_eq!(selector, "input#search");
                assert_eq!(text, "hello world");
            }
            _ => panic!("Expected TypeText"),
        }
    }

    #[test]
    fn test_browser_action_extract_text_serde() {
        let action = BrowserAction::ExtractText {
            selector: ".content p".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"action\":\"extract_text\""));

        let parsed: BrowserAction = serde_json::from_str(&json).unwrap();
        match parsed {
            BrowserAction::ExtractText { selector } => assert_eq!(selector, ".content p"),
            _ => panic!("Expected ExtractText"),
        }
    }

    #[test]
    fn test_browser_action_wait_for_serde() {
        let action = BrowserAction::WaitFor {
            selector: ".loaded".to_string(),
            timeout_ms: 3000,
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"action\":\"wait_for\""));
        assert!(json.contains("\"timeout_ms\":3000"));

        let parsed: BrowserAction = serde_json::from_str(&json).unwrap();
        match parsed {
            BrowserAction::WaitFor {
                selector,
                timeout_ms,
            } => {
                assert_eq!(selector, ".loaded");
                assert_eq!(timeout_ms, 3000);
            }
            _ => panic!("Expected WaitFor"),
        }
    }

    #[test]
    fn test_browser_action_wait_for_serde_default_timeout() {
        // Deserialize with missing timeout_ms to test default
        let json = r#"{"action":"wait_for","selector":".ready"}"#;
        let parsed: BrowserAction = serde_json::from_str(json).unwrap();
        match parsed {
            BrowserAction::WaitFor {
                selector,
                timeout_ms,
            } => {
                assert_eq!(selector, ".ready");
                assert_eq!(timeout_ms, 5000); // default
            }
            _ => panic!("Expected WaitFor"),
        }
    }

    // ── Parse edge cases ─────────────────────────────────────────────

    #[test]
    fn test_parse_extract_text_missing_selector() {
        let args = serde_json::json!({"action": "extract_text"});
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'selector'"));
    }

    #[test]
    fn test_parse_wait_for_missing_selector() {
        let args = serde_json::json!({"action": "wait_for"});
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'selector'"));
    }

    #[test]
    fn test_parse_type_text_missing_selector() {
        let args = serde_json::json!({
            "action": "type_text",
            "text": "hello"
        });
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'selector'"));
    }

    #[test]
    fn test_parse_action_with_extra_fields() {
        // Extra fields should be ignored gracefully
        let args = serde_json::json!({
            "action": "navigate",
            "url": "https://example.com",
            "extra_field": "should be ignored"
        });
        let action = BrowserTool::parse_action(&args).unwrap();
        match action {
            BrowserAction::Navigate { url } => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected Navigate"),
        }
    }

    #[test]
    fn test_parse_action_null_value() {
        let args = serde_json::json!(null);
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_action_non_string_action() {
        let args = serde_json::json!({"action": 123});
        let result = BrowserTool::parse_action(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required 'action'"));
    }

    // ── Cleanup test ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_cleanup_noop_when_not_initialized() {
        let tool = BrowserTool::new();
        // Cleanup should be a no-op when browser is not initialized
        tool.cleanup().await;
        // No panic = success
    }

    // ── Execute all action types: graceful degradation ───────────────

    #[tokio::test]
    async fn test_execute_click_graceful() {
        let tool = BrowserTool::new();
        let ctx = make_ctx();
        let result = tool
            .execute(
                &ctx,
                serde_json::json!({"action": "click", "selector": "#btn"}),
            )
            .await;
        // Should not panic; error message should reference the action
        if !result.success {
            let err = result.error.as_deref().unwrap_or("");
            assert!(
                err.contains("click") || err.contains("Chrome") || err.contains("browser"),
                "Error should be descriptive: {}",
                err
            );
        }
    }

    #[tokio::test]
    async fn test_execute_type_text_graceful() {
        let tool = BrowserTool::new();
        let ctx = make_ctx();
        let result = tool
            .execute(
                &ctx,
                serde_json::json!({
                    "action": "type_text",
                    "selector": "#input",
                    "text": "hello"
                }),
            )
            .await;
        if !result.success {
            let err = result.error.as_deref().unwrap_or("");
            assert!(
                err.contains("type_text") || err.contains("Chrome") || err.contains("browser"),
                "Error should be descriptive: {}",
                err
            );
        }
    }

    #[tokio::test]
    async fn test_execute_screenshot_graceful() {
        let tool = BrowserTool::new();
        let ctx = make_ctx();
        let result = tool
            .execute(&ctx, serde_json::json!({"action": "screenshot"}))
            .await;
        if !result.success {
            let err = result.error.as_deref().unwrap_or("");
            assert!(
                err.contains("screenshot") || err.contains("Chrome") || err.contains("browser"),
                "Error should be descriptive: {}",
                err
            );
        }
    }

    #[tokio::test]
    async fn test_execute_extract_text_graceful() {
        let tool = BrowserTool::new();
        let ctx = make_ctx();
        let result = tool
            .execute(
                &ctx,
                serde_json::json!({
                    "action": "extract_text",
                    "selector": ".content"
                }),
            )
            .await;
        if !result.success {
            let err = result.error.as_deref().unwrap_or("");
            assert!(
                err.contains("extract_text") || err.contains("Chrome") || err.contains("browser"),
                "Error should be descriptive: {}",
                err
            );
        }
    }

    #[tokio::test]
    async fn test_execute_wait_for_graceful() {
        let tool = BrowserTool::new();
        let ctx = make_ctx();
        let result = tool
            .execute(
                &ctx,
                serde_json::json!({
                    "action": "wait_for",
                    "selector": ".ready",
                    "timeout_ms": 1000
                }),
            )
            .await;
        if !result.success {
            let err = result.error.as_deref().unwrap_or("");
            assert!(
                err.contains("wait_for") || err.contains("Chrome") || err.contains("browser"),
                "Error should be descriptive: {}",
                err
            );
        }
    }
}
