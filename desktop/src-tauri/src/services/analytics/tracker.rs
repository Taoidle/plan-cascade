//! Usage Tracker
//!
//! Tracks API usage in real-time, capturing tokens, costs, and requests.
//! Uses async channels for non-blocking writes and batch inserts.

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

use crate::models::analytics::UsageRecord;
use crate::utils::error::{AppError, AppResult};

use super::cost_calculator::CostCalculator;
use super::service::AnalyticsService;

/// Message type for the tracker channel
#[derive(Debug, Clone)]
pub enum TrackerMessage {
    /// Track a new usage record
    Track(UsageRecord),
    /// Flush all buffered records to database
    Flush,
    /// Shutdown the tracker
    Shutdown,
}

/// Configuration for the usage tracker
#[derive(Debug, Clone)]
pub struct TrackerConfig {
    /// Buffer size before auto-flush
    pub buffer_size: usize,
    /// Flush interval in seconds
    pub flush_interval_secs: u64,
    /// Whether tracking is enabled
    pub enabled: bool,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            buffer_size: 100,
            flush_interval_secs: 30,
            enabled: true,
        }
    }
}

/// Usage tracker for real-time API usage tracking
pub struct UsageTracker {
    /// Channel sender for non-blocking writes
    sender: mpsc::Sender<TrackerMessage>,
    /// Cost calculator for computing costs
    cost_calculator: Arc<CostCalculator>,
    /// Current session ID (if tracking within a session)
    current_session: Arc<RwLock<Option<String>>>,
    /// Current project ID (if tracking within a project)
    current_project: Arc<RwLock<Option<String>>>,
    /// Configuration
    config: TrackerConfig,
}

impl UsageTracker {
    /// Create a new usage tracker
    pub fn new(
        service: Arc<AnalyticsService>,
        cost_calculator: Arc<CostCalculator>,
        config: TrackerConfig,
    ) -> Self {
        let (sender, receiver) = mpsc::channel::<TrackerMessage>(1000);

        // Spawn background task for processing
        let service_clone = service.clone();
        let config_clone = config.clone();
        tokio::spawn(async move {
            Self::process_messages(receiver, service_clone, config_clone).await;
        });

        Self {
            sender,
            cost_calculator,
            current_session: Arc::new(RwLock::new(None)),
            current_project: Arc::new(RwLock::new(None)),
            config,
        }
    }

    /// Background task that processes tracking messages
    async fn process_messages(
        mut receiver: mpsc::Receiver<TrackerMessage>,
        service: Arc<AnalyticsService>,
        config: TrackerConfig,
    ) {
        let mut buffer: Vec<UsageRecord> = Vec::with_capacity(config.buffer_size);
        let mut flush_interval = tokio::time::interval(
            tokio::time::Duration::from_secs(config.flush_interval_secs)
        );

        loop {
            tokio::select! {
                // Process incoming messages
                msg = receiver.recv() => {
                    match msg {
                        Some(TrackerMessage::Track(record)) => {
                            buffer.push(record);

                            // Auto-flush when buffer is full
                            if buffer.len() >= config.buffer_size {
                                Self::flush_buffer(&service, &mut buffer).await;
                            }
                        }
                        Some(TrackerMessage::Flush) => {
                            Self::flush_buffer(&service, &mut buffer).await;
                        }
                        Some(TrackerMessage::Shutdown) | None => {
                            // Flush remaining records before shutdown
                            Self::flush_buffer(&service, &mut buffer).await;
                            break;
                        }
                    }
                }
                // Periodic flush
                _ = flush_interval.tick() => {
                    if !buffer.is_empty() {
                        Self::flush_buffer(&service, &mut buffer).await;
                    }
                }
            }
        }
    }

    /// Flush buffer to database
    async fn flush_buffer(service: &AnalyticsService, buffer: &mut Vec<UsageRecord>) {
        if buffer.is_empty() {
            return;
        }

        match service.insert_usage_records_batch(buffer) {
            Ok(_) => {
                tracing::debug!("Flushed {} usage records to database", buffer.len());
            }
            Err(e) => {
                tracing::error!("Failed to flush usage records: {}", e);
                // Keep records in buffer for retry? For now, we log and clear
            }
        }

        buffer.clear();
    }

    /// Track a new API usage
    pub async fn track(
        &self,
        provider: &str,
        model_name: &str,
        input_tokens: i64,
        output_tokens: i64,
    ) -> AppResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let cost = self.cost_calculator.calculate_cost(provider, model_name, input_tokens, output_tokens);

        let session_id = self.current_session.read().await.clone();
        let project_id = self.current_project.read().await.clone();

        let mut record = UsageRecord::new(model_name, provider, input_tokens, output_tokens)
            .with_cost(cost);

        if let Some(session) = session_id {
            record = record.with_session(session);
        }
        if let Some(project) = project_id {
            record = record.with_project(project);
        }

        self.sender
            .send(TrackerMessage::Track(record))
            .await
            .map_err(|_| AppError::internal("Failed to send tracking message"))?;

        Ok(())
    }

    /// Track with full record details
    pub async fn track_record(&self, record: UsageRecord) -> AppResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        self.sender
            .send(TrackerMessage::Track(record))
            .await
            .map_err(|_| AppError::internal("Failed to send tracking message"))?;

        Ok(())
    }

    /// Track with session and project context
    pub async fn track_with_context(
        &self,
        provider: &str,
        model_name: &str,
        input_tokens: i64,
        output_tokens: i64,
        session_id: Option<String>,
        project_id: Option<String>,
    ) -> AppResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let cost = self.cost_calculator.calculate_cost(provider, model_name, input_tokens, output_tokens);

        let mut record = UsageRecord::new(model_name, provider, input_tokens, output_tokens)
            .with_cost(cost);

        if let Some(session) = session_id {
            record = record.with_session(session);
        }
        if let Some(project) = project_id {
            record = record.with_project(project);
        }

        self.sender
            .send(TrackerMessage::Track(record))
            .await
            .map_err(|_| AppError::internal("Failed to send tracking message"))?;

        Ok(())
    }

    /// Set the current session for tracking
    pub async fn set_session(&self, session_id: Option<String>) {
        let mut session = self.current_session.write().await;
        *session = session_id;
    }

    /// Set the current project for tracking
    pub async fn set_project(&self, project_id: Option<String>) {
        let mut project = self.current_project.write().await;
        *project = project_id;
    }

    /// Get the current session ID
    pub async fn get_session(&self) -> Option<String> {
        self.current_session.read().await.clone()
    }

    /// Get the current project ID
    pub async fn get_project(&self) -> Option<String> {
        self.current_project.read().await.clone()
    }

    /// Manually flush buffered records
    pub async fn flush(&self) -> AppResult<()> {
        self.sender
            .send(TrackerMessage::Flush)
            .await
            .map_err(|_| AppError::internal("Failed to send flush message"))?;

        Ok(())
    }

    /// Shutdown the tracker
    pub async fn shutdown(&self) -> AppResult<()> {
        self.sender
            .send(TrackerMessage::Shutdown)
            .await
            .map_err(|_| AppError::internal("Failed to send shutdown message"))?;

        Ok(())
    }

    /// Check if tracking is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Enable or disable tracking
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }
}

/// Builder for creating UsageTracker instances
pub struct UsageTrackerBuilder {
    config: TrackerConfig,
    cost_calculator: Option<Arc<CostCalculator>>,
}

impl Default for UsageTrackerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl UsageTrackerBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: TrackerConfig::default(),
            cost_calculator: None,
        }
    }

    /// Set buffer size
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = size;
        self
    }

    /// Set flush interval
    pub fn flush_interval_secs(mut self, secs: u64) -> Self {
        self.config.flush_interval_secs = secs;
        self
    }

    /// Set enabled state
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Set cost calculator
    pub fn cost_calculator(mut self, calc: Arc<CostCalculator>) -> Self {
        self.cost_calculator = Some(calc);
        self
    }

    /// Build the tracker
    pub fn build(self, service: Arc<AnalyticsService>) -> UsageTracker {
        let cost_calc = self.cost_calculator.unwrap_or_else(|| Arc::new(CostCalculator::new()));
        UsageTracker::new(service, cost_calc, self.config)
    }
}

/// Synchronous wrapper for tracking that doesn't require async context
pub struct SyncUsageTracker {
    sender: mpsc::Sender<TrackerMessage>,
    cost_calculator: Arc<CostCalculator>,
    enabled: bool,
}

impl SyncUsageTracker {
    /// Create from an existing UsageTracker's sender
    pub fn from_sender(
        sender: mpsc::Sender<TrackerMessage>,
        cost_calculator: Arc<CostCalculator>,
        enabled: bool,
    ) -> Self {
        Self {
            sender,
            cost_calculator,
            enabled,
        }
    }

    /// Track usage synchronously (non-blocking)
    pub fn track(
        &self,
        provider: &str,
        model_name: &str,
        input_tokens: i64,
        output_tokens: i64,
        session_id: Option<String>,
        project_id: Option<String>,
    ) {
        if !self.enabled {
            return;
        }

        let cost = self.cost_calculator.calculate_cost(provider, model_name, input_tokens, output_tokens);

        let mut record = UsageRecord::new(model_name, provider, input_tokens, output_tokens)
            .with_cost(cost);

        if let Some(session) = session_id {
            record = record.with_session(session);
        }
        if let Some(project) = project_id {
            record = record.with_project(project);
        }

        // Try to send, but don't block if channel is full
        let _ = self.sender.try_send(TrackerMessage::Track(record));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use r2d2::Pool;
    use r2d2_sqlite::SqliteConnectionManager;
    use crate::models::analytics::UsageFilter;

    fn create_test_service() -> Arc<AnalyticsService> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder()
            .max_size(1)
            .build(manager)
            .unwrap();
        Arc::new(AnalyticsService::from_pool(pool).unwrap())
    }

    #[tokio::test]
    async fn test_tracker_creation() {
        let service = create_test_service();
        let cost_calc = Arc::new(CostCalculator::new());
        let tracker = UsageTracker::new(service, cost_calc, TrackerConfig::default());

        assert!(tracker.is_enabled());
    }

    #[tokio::test]
    async fn test_basic_tracking() {
        let service = create_test_service();
        let cost_calc = Arc::new(CostCalculator::new());
        let config = TrackerConfig {
            buffer_size: 1, // Flush immediately
            flush_interval_secs: 1,
            enabled: true,
        };
        let tracker = UsageTracker::new(service.clone(), cost_calc, config);

        // Track some usage
        tracker.track("anthropic", "claude-3-5-sonnet-20241022", 1000, 500).await.unwrap();

        // Wait for flush
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        tracker.flush().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify in database
        let records = service.list_usage_records(&UsageFilter::default(), None, None).unwrap();
        assert!(!records.is_empty());
    }

    #[tokio::test]
    async fn test_session_context() {
        let service = create_test_service();
        let cost_calc = Arc::new(CostCalculator::new());
        let tracker = UsageTracker::new(service, cost_calc, TrackerConfig::default());

        // Initially no session
        assert!(tracker.get_session().await.is_none());

        // Set session
        tracker.set_session(Some("test-session-123".to_string())).await;
        assert_eq!(tracker.get_session().await, Some("test-session-123".to_string()));

        // Clear session
        tracker.set_session(None).await;
        assert!(tracker.get_session().await.is_none());
    }

    #[tokio::test]
    async fn test_project_context() {
        let service = create_test_service();
        let cost_calc = Arc::new(CostCalculator::new());
        let tracker = UsageTracker::new(service, cost_calc, TrackerConfig::default());

        tracker.set_project(Some("test-project".to_string())).await;
        assert_eq!(tracker.get_project().await, Some("test-project".to_string()));
    }

    #[tokio::test]
    async fn test_tracking_disabled() {
        let service = create_test_service();
        let cost_calc = Arc::new(CostCalculator::new());
        let config = TrackerConfig {
            enabled: false,
            ..Default::default()
        };
        let tracker = UsageTracker::new(service.clone(), cost_calc, config);

        // Should not track when disabled
        tracker.track("anthropic", "claude-3-5-sonnet", 1000, 500).await.unwrap();
        tracker.flush().await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let records = service.list_usage_records(&UsageFilter::default(), None, None).unwrap();
        assert!(records.is_empty());
    }

    #[tokio::test]
    async fn test_track_with_context() {
        let service = create_test_service();
        let cost_calc = Arc::new(CostCalculator::new());
        let config = TrackerConfig {
            buffer_size: 1,
            flush_interval_secs: 1,
            enabled: true,
        };
        let tracker = UsageTracker::new(service.clone(), cost_calc, config);

        tracker.track_with_context(
            "openai",
            "gpt-4o",
            500,
            250,
            Some("session-abc".to_string()),
            Some("project-xyz".to_string()),
        ).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        tracker.flush().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let records = service.list_usage_records(&UsageFilter::default(), None, None).unwrap();
        assert!(!records.is_empty());

        let record = &records[0];
        assert_eq!(record.session_id, Some("session-abc".to_string()));
        assert_eq!(record.project_id, Some("project-xyz".to_string()));
    }

    #[tokio::test]
    async fn test_builder() {
        let service = create_test_service();

        let tracker = UsageTrackerBuilder::new()
            .buffer_size(50)
            .flush_interval_secs(60)
            .enabled(true)
            .cost_calculator(Arc::new(CostCalculator::new()))
            .build(service);

        assert!(tracker.is_enabled());
    }
}
