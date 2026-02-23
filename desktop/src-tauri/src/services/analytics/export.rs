//! Export Functionality
//!
//! Provides CSV and JSON export capabilities for usage analytics data.
//! Supports filtered exports with summary statistics.

use std::io::Write;

use crate::models::analytics::{
    ExportFormat, ExportRequest, ExportResult, UsageFilter, UsageRecord, UsageStats,
};
use crate::utils::error::{AppError, AppResult};

use super::service::AnalyticsService;

impl AnalyticsService {
    /// Export usage data based on request parameters
    pub fn export_usage(&self, request: &ExportRequest) -> AppResult<ExportResult> {
        let records = self.list_usage_records(&request.filter, None, None)?;
        let summary = if request.include_summary {
            Some(self.get_usage_stats(&request.filter)?)
        } else {
            None
        };

        let (data, filename) = match request.format {
            ExportFormat::Csv => {
                let csv = self.records_to_csv(&records, summary.as_ref())?;
                let filename = format!("usage_export_{}.csv", Self::timestamp_for_filename());
                (csv, filename)
            }
            ExportFormat::Json => {
                let json = self.records_to_json(&records, summary.as_ref())?;
                let filename = format!("usage_export_{}.json", Self::timestamp_for_filename());
                (json, filename)
            }
        };

        Ok(ExportResult {
            data,
            record_count: records.len() as i64,
            summary,
            suggested_filename: filename,
        })
    }

    /// Export to CSV format
    fn records_to_csv(
        &self,
        records: &[UsageRecord],
        summary: Option<&UsageStats>,
    ) -> AppResult<String> {
        let mut output = Vec::new();

        // Write header
        writeln!(
            output,
            "id,session_id,project_id,model_name,provider,input_tokens,output_tokens,total_tokens,cost_microdollars,cost_dollars,timestamp,timestamp_formatted,metadata"
        ).map_err(|e| AppError::internal(e.to_string()))?;

        // Write records
        for record in records {
            let timestamp_formatted = chrono::DateTime::from_timestamp(record.timestamp, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default();

            let session_id = Self::csv_escape(record.session_id.as_deref().unwrap_or(""));
            let project_id = Self::csv_escape(record.project_id.as_deref().unwrap_or(""));
            let metadata = Self::csv_escape(record.metadata.as_deref().unwrap_or(""));

            writeln!(
                output,
                "{},{},{},{},{},{},{},{},{},{:.6},{},\"{}\",{}",
                record.id,
                session_id,
                project_id,
                Self::csv_escape(&record.model_name),
                Self::csv_escape(&record.provider),
                record.input_tokens,
                record.output_tokens,
                record.total_tokens(),
                record.cost_microdollars,
                record.cost_dollars(),
                record.timestamp,
                timestamp_formatted,
                metadata,
            )
            .map_err(|e| AppError::internal(e.to_string()))?;
        }

        // Write summary row if requested
        if let Some(stats) = summary {
            writeln!(output, "\n# Summary").map_err(|e| AppError::internal(e.to_string()))?;
            writeln!(output, "# Total Input Tokens: {}", stats.total_input_tokens)
                .map_err(|e| AppError::internal(e.to_string()))?;
            writeln!(
                output,
                "# Total Output Tokens: {}",
                stats.total_output_tokens
            )
            .map_err(|e| AppError::internal(e.to_string()))?;
            writeln!(output, "# Total Tokens: {}", stats.total_tokens())
                .map_err(|e| AppError::internal(e.to_string()))?;
            writeln!(
                output,
                "# Total Cost (microdollars): {}",
                stats.total_cost_microdollars
            )
            .map_err(|e| AppError::internal(e.to_string()))?;
            writeln!(
                output,
                "# Total Cost (dollars): ${:.6}",
                stats.total_cost_dollars()
            )
            .map_err(|e| AppError::internal(e.to_string()))?;
            writeln!(output, "# Request Count: {}", stats.request_count)
                .map_err(|e| AppError::internal(e.to_string()))?;
            writeln!(
                output,
                "# Average Tokens per Request: {:.2}",
                stats.avg_tokens_per_request
            )
            .map_err(|e| AppError::internal(e.to_string()))?;
            writeln!(
                output,
                "# Average Cost per Request (microdollars): {:.2}",
                stats.avg_cost_per_request
            )
            .map_err(|e| AppError::internal(e.to_string()))?;
        }

        String::from_utf8(output).map_err(|e| AppError::internal(e.to_string()))
    }

    /// Export to JSON format
    fn records_to_json(
        &self,
        records: &[UsageRecord],
        summary: Option<&UsageStats>,
    ) -> AppResult<String> {
        #[derive(serde::Serialize)]
        struct ExportData<'a> {
            exported_at: String,
            record_count: usize,
            summary: Option<&'a UsageStats>,
            records: &'a [UsageRecord],
        }

        let export_data = ExportData {
            exported_at: chrono::Utc::now().to_rfc3339(),
            record_count: records.len(),
            summary,
            records,
        };

        serde_json::to_string_pretty(&export_data).map_err(|e| AppError::internal(e.to_string()))
    }

    /// Export with streaming for large datasets
    pub fn export_usage_streaming<W: Write>(
        &self,
        request: &ExportRequest,
        writer: &mut W,
        chunk_size: usize,
    ) -> AppResult<i64> {
        let total_count = self.count_usage_records(&request.filter)?;
        let mut exported = 0_i64;
        let mut offset = 0_i64;

        match request.format {
            ExportFormat::Csv => {
                // Write header
                writeln!(
                    writer,
                    "id,session_id,project_id,model_name,provider,input_tokens,output_tokens,total_tokens,cost_microdollars,cost_dollars,timestamp,timestamp_formatted,metadata"
                ).map_err(|e| AppError::internal(e.to_string()))?;

                // Stream records in chunks
                loop {
                    let records = self.list_usage_records(
                        &request.filter,
                        Some(chunk_size as i64),
                        Some(offset),
                    )?;

                    if records.is_empty() {
                        break;
                    }

                    for record in &records {
                        let timestamp_formatted =
                            chrono::DateTime::from_timestamp(record.timestamp, 0)
                                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                .unwrap_or_default();

                        writeln!(
                            writer,
                            "{},{},{},{},{},{},{},{},{},{:.6},{},\"{}\",{}",
                            record.id,
                            Self::csv_escape(record.session_id.as_deref().unwrap_or("")),
                            Self::csv_escape(record.project_id.as_deref().unwrap_or("")),
                            Self::csv_escape(&record.model_name),
                            Self::csv_escape(&record.provider),
                            record.input_tokens,
                            record.output_tokens,
                            record.total_tokens(),
                            record.cost_microdollars,
                            record.cost_dollars(),
                            record.timestamp,
                            timestamp_formatted,
                            Self::csv_escape(record.metadata.as_deref().unwrap_or("")),
                        )
                        .map_err(|e| AppError::internal(e.to_string()))?;
                    }

                    exported += records.len() as i64;
                    offset += chunk_size as i64;

                    if records.len() < chunk_size {
                        break;
                    }
                }
            }
            ExportFormat::Json => {
                // For JSON, we need to build incrementally
                writeln!(writer, "{{").map_err(|e| AppError::internal(e.to_string()))?;
                writeln!(
                    writer,
                    "  \"exported_at\": \"{}\",",
                    chrono::Utc::now().to_rfc3339()
                )
                .map_err(|e| AppError::internal(e.to_string()))?;
                writeln!(writer, "  \"record_count\": {},", total_count)
                    .map_err(|e| AppError::internal(e.to_string()))?;

                if request.include_summary {
                    let summary = self.get_usage_stats(&request.filter)?;
                    let summary_json = serde_json::to_string_pretty(&summary)
                        .map_err(|e| AppError::internal(e.to_string()))?;
                    writeln!(writer, "  \"summary\": {},", summary_json)
                        .map_err(|e| AppError::internal(e.to_string()))?;
                }

                writeln!(writer, "  \"records\": [")
                    .map_err(|e| AppError::internal(e.to_string()))?;

                let mut first = true;
                loop {
                    let records = self.list_usage_records(
                        &request.filter,
                        Some(chunk_size as i64),
                        Some(offset),
                    )?;

                    if records.is_empty() {
                        break;
                    }

                    for record in &records {
                        if !first {
                            writeln!(writer, ",").map_err(|e| AppError::internal(e.to_string()))?;
                        }
                        first = false;

                        let record_json = serde_json::to_string(&record)
                            .map_err(|e| AppError::internal(e.to_string()))?;
                        write!(writer, "    {}", record_json)
                            .map_err(|e| AppError::internal(e.to_string()))?;
                    }

                    exported += records.len() as i64;
                    offset += chunk_size as i64;

                    if records.len() < chunk_size {
                        break;
                    }
                }

                writeln!(writer).map_err(|e| AppError::internal(e.to_string()))?;
                writeln!(writer, "  ]").map_err(|e| AppError::internal(e.to_string()))?;
                writeln!(writer, "}}").map_err(|e| AppError::internal(e.to_string()))?;
            }
        }

        Ok(exported)
    }

    /// Helper to escape CSV values
    fn csv_escape(value: &str) -> String {
        if value.contains(',') || value.contains('"') || value.contains('\n') {
            format!("\"{}\"", value.replace('"', "\"\""))
        } else {
            value.to_string()
        }
    }

    /// Generate timestamp string for filename
    fn timestamp_for_filename() -> String {
        chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string()
    }

    /// Export model pricing to JSON
    pub fn export_pricing(&self) -> AppResult<String> {
        let pricing = self.list_model_pricing()?;
        serde_json::to_string_pretty(&pricing).map_err(|e| AppError::internal(e.to_string()))
    }

    /// Export aggregated data by model
    pub fn export_by_model(&self, filter: &UsageFilter, format: ExportFormat) -> AppResult<String> {
        let data = self.aggregate_by_model(filter)?;

        match format {
            ExportFormat::Csv => {
                let mut output = Vec::new();
                writeln!(output, "model_name,provider,total_input_tokens,total_output_tokens,total_cost_microdollars,total_cost_dollars,request_count,avg_tokens_per_request,avg_cost_per_request")
                    .map_err(|e| AppError::internal(e.to_string()))?;

                for item in &data {
                    writeln!(
                        output,
                        "{},{},{},{},{},{:.6},{},{:.2},{:.2}",
                        Self::csv_escape(&item.model_name),
                        Self::csv_escape(&item.provider),
                        item.stats.total_input_tokens,
                        item.stats.total_output_tokens,
                        item.stats.total_cost_microdollars,
                        item.stats.total_cost_dollars(),
                        item.stats.request_count,
                        item.stats.avg_tokens_per_request,
                        item.stats.avg_cost_per_request,
                    )
                    .map_err(|e| AppError::internal(e.to_string()))?;
                }

                String::from_utf8(output).map_err(|e| AppError::internal(e.to_string()))
            }
            ExportFormat::Json => {
                serde_json::to_string_pretty(&data).map_err(|e| AppError::internal(e.to_string()))
            }
        }
    }

    /// Export aggregated data by project
    pub fn export_by_project(
        &self,
        filter: &UsageFilter,
        format: ExportFormat,
    ) -> AppResult<String> {
        let data = self.aggregate_by_project(filter)?;

        match format {
            ExportFormat::Csv => {
                let mut output = Vec::new();
                writeln!(output, "project_id,project_name,total_input_tokens,total_output_tokens,total_cost_microdollars,total_cost_dollars,request_count,avg_tokens_per_request,avg_cost_per_request")
                    .map_err(|e| AppError::internal(e.to_string()))?;

                for item in &data {
                    writeln!(
                        output,
                        "{},{},{},{},{},{:.6},{},{:.2},{:.2}",
                        Self::csv_escape(&item.project_id),
                        Self::csv_escape(item.project_name.as_deref().unwrap_or("")),
                        item.stats.total_input_tokens,
                        item.stats.total_output_tokens,
                        item.stats.total_cost_microdollars,
                        item.stats.total_cost_dollars(),
                        item.stats.request_count,
                        item.stats.avg_tokens_per_request,
                        item.stats.avg_cost_per_request,
                    )
                    .map_err(|e| AppError::internal(e.to_string()))?;
                }

                String::from_utf8(output).map_err(|e| AppError::internal(e.to_string()))
            }
            ExportFormat::Json => {
                serde_json::to_string_pretty(&data).map_err(|e| AppError::internal(e.to_string()))
            }
        }
    }

    /// Export time series data
    pub fn export_time_series(
        &self,
        filter: &UsageFilter,
        period: crate::models::analytics::AggregationPeriod,
        format: ExportFormat,
    ) -> AppResult<String> {
        let data = self.get_time_series(filter, period)?;

        match format {
            ExportFormat::Csv => {
                let mut output = Vec::new();
                writeln!(output, "timestamp,timestamp_formatted,total_input_tokens,total_output_tokens,total_cost_microdollars,total_cost_dollars,request_count")
                    .map_err(|e| AppError::internal(e.to_string()))?;

                for point in &data {
                    writeln!(
                        output,
                        "{},{},{},{},{},{:.6},{}",
                        point.timestamp,
                        Self::csv_escape(&point.timestamp_formatted),
                        point.stats.total_input_tokens,
                        point.stats.total_output_tokens,
                        point.stats.total_cost_microdollars,
                        point.stats.total_cost_dollars(),
                        point.stats.request_count,
                    )
                    .map_err(|e| AppError::internal(e.to_string()))?;
                }

                String::from_utf8(output).map_err(|e| AppError::internal(e.to_string()))
            }
            ExportFormat::Json => {
                serde_json::to_string_pretty(&data).map_err(|e| AppError::internal(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::analytics::UsageRecord;
    use r2d2::Pool;
    use r2d2_sqlite::SqliteConnectionManager;

    fn create_test_service() -> AppResult<AnalyticsService> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder()
            .max_size(1)
            .build(manager)
            .map_err(|e| AppError::database(e.to_string()))?;
        AnalyticsService::from_pool(pool)
    }

    fn seed_test_data(service: &AnalyticsService) {
        let now = chrono::Utc::now().timestamp();
        let records = vec![
            UsageRecord {
                id: 0,
                session_id: Some("s1".to_string()),
                project_id: Some("p1".to_string()),
                model_name: "claude-3-5-sonnet".to_string(),
                provider: "anthropic".to_string(),
                input_tokens: 1000,
                output_tokens: 500,
                thinking_tokens: 0,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                cost_microdollars: 100,
                timestamp: now,
                metadata: None,
            },
            UsageRecord {
                id: 0,
                session_id: Some("s2".to_string()),
                project_id: Some("p2".to_string()),
                model_name: "gpt-4o".to_string(),
                provider: "openai".to_string(),
                input_tokens: 2000,
                output_tokens: 1000,
                thinking_tokens: 0,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                cost_microdollars: 200,
                timestamp: now - 3600,
                metadata: Some("{\"test\": true}".to_string()),
            },
        ];
        service.insert_usage_records_batch(&records).unwrap();
    }

    #[test]
    fn test_export_csv() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        let request = ExportRequest {
            filter: UsageFilter::default(),
            format: ExportFormat::Csv,
            include_summary: true,
        };

        let result = service.export_usage(&request).unwrap();

        assert_eq!(result.record_count, 2);
        assert!(result.data.contains("session_id"));
        assert!(result.data.contains("claude-3-5-sonnet"));
        assert!(result.data.contains("gpt-4o"));
        assert!(result.data.contains("# Summary"));
        assert!(result.suggested_filename.ends_with(".csv"));
    }

    #[test]
    fn test_export_json() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        let request = ExportRequest {
            filter: UsageFilter::default(),
            format: ExportFormat::Json,
            include_summary: true,
        };

        let result = service.export_usage(&request).unwrap();

        assert_eq!(result.record_count, 2);
        assert!(result.data.contains("\"model_name\""));
        assert!(result.data.contains("\"summary\""));

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&result.data).unwrap();
        assert!(parsed.get("records").is_some());
        assert!(parsed.get("summary").is_some());
    }

    #[test]
    fn test_export_without_summary() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        let request = ExportRequest {
            filter: UsageFilter::default(),
            format: ExportFormat::Csv,
            include_summary: false,
        };

        let result = service.export_usage(&request).unwrap();

        assert!(result.summary.is_none());
        assert!(!result.data.contains("# Summary"));
    }

    #[test]
    fn test_export_with_filter() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        let request = ExportRequest {
            filter: UsageFilter::default().with_provider("anthropic"),
            format: ExportFormat::Csv,
            include_summary: false,
        };

        let result = service.export_usage(&request).unwrap();

        assert_eq!(result.record_count, 1);
        assert!(result.data.contains("anthropic"));
        assert!(!result.data.contains("openai"));
    }

    #[test]
    fn test_csv_escape() {
        assert_eq!(AnalyticsService::csv_escape("simple"), "simple");
        assert_eq!(AnalyticsService::csv_escape("with,comma"), "\"with,comma\"");
        assert_eq!(
            AnalyticsService::csv_escape("with\"quote"),
            "\"with\"\"quote\""
        );
        assert_eq!(
            AnalyticsService::csv_escape("with\nnewline"),
            "\"with\nnewline\""
        );
    }

    #[test]
    fn test_export_streaming() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        let request = ExportRequest {
            filter: UsageFilter::default(),
            format: ExportFormat::Csv,
            include_summary: false,
        };

        let mut output = Vec::new();
        let count = service
            .export_usage_streaming(&request, &mut output, 1)
            .unwrap();

        assert_eq!(count, 2);

        let content = String::from_utf8(output).unwrap();
        assert!(content.contains("session_id"));
        assert!(content.contains("claude-3-5-sonnet"));
    }

    #[test]
    fn test_export_by_model() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        // CSV
        let csv = service
            .export_by_model(&UsageFilter::default(), ExportFormat::Csv)
            .unwrap();
        assert!(csv.contains("model_name,provider"));
        assert!(csv.contains("claude-3-5-sonnet"));

        // JSON
        let json = service
            .export_by_model(&UsageFilter::default(), ExportFormat::Json)
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
    }

    #[test]
    fn test_export_pricing() {
        let service = create_test_service().unwrap();

        let json = service.export_pricing().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.is_array());
        assert!(parsed.as_array().unwrap().len() > 0);
    }
}
