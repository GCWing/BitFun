//! Token usage tracking service implementation

use super::types::{
    ModelTokenStats, SessionTokenStats, TimeRange, TokenUsageQuery, TokenUsageRecord,
    TokenUsageSummary,
};
use crate::infrastructure::PathManager;
use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Duration, Utc};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

const TOKEN_USAGE_DIR: &str = "token_usage";
const MODEL_STATS_FILE: &str = "model_stats.json";
const RECORDS_DIR: &str = "records";

/// Token usage tracking service
pub struct TokenUsageService {
    path_manager: Arc<PathManager>,
    model_stats: Arc<RwLock<HashMap<String, ModelTokenStats>>>,
    session_cache: Arc<RwLock<HashMap<String, SessionTokenStats>>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecordsBatch {
    records: Vec<TokenUsageRecord>,
}

impl TokenUsageService {
    /// Create a new token usage service
    pub async fn new(path_manager: Arc<PathManager>) -> Result<Self> {
        let service = Self {
            path_manager,
            model_stats: Arc::new(RwLock::new(HashMap::new())),
            session_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        // Initialize storage directories
        service.init_storage().await?;

        // Load existing statistics
        service.load_model_stats().await?;

        info!("Token usage service initialized");
        Ok(service)
    }

    /// Initialize storage directories
    async fn init_storage(&self) -> Result<()> {
        let base_dir = self.get_base_dir();
        let records_dir = base_dir.join(RECORDS_DIR);

        fs::create_dir_all(&base_dir)
            .await
            .context("Failed to create token usage directory")?;
        fs::create_dir_all(&records_dir)
            .await
            .context("Failed to create records directory")?;

        debug!("Token usage storage initialized at: {:?}", base_dir);
        Ok(())
    }

    /// Get base directory for token usage data
    fn get_base_dir(&self) -> PathBuf {
        self.path_manager.user_data_dir().join(TOKEN_USAGE_DIR)
    }

    /// Get model stats file path
    fn get_model_stats_path(&self) -> PathBuf {
        self.get_base_dir().join(MODEL_STATS_FILE)
    }

    /// Get records file path for a specific date
    fn get_records_path(&self, date: DateTime<Utc>) -> PathBuf {
        let filename = format!("{}.json", date.format("%Y-%m-%d"));
        self.get_base_dir().join(RECORDS_DIR).join(filename)
    }

    /// Load model statistics from disk
    async fn load_model_stats(&self) -> Result<()> {
        let path = self.get_model_stats_path();
        
        if !path.exists() {
            debug!("No existing model stats file found");
            return Ok(());
        }

        let content = match fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read model stats file, starting fresh: {}", e);
                return Ok(());
            }
        };
        
        let stats: HashMap<String, ModelTokenStats> = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to parse model stats file, starting fresh: {}", e);
                // Backup the corrupted file for debugging
                let backup_path = path.with_extension("json.bak");
                if let Err(backup_err) = fs::rename(&path, &backup_path).await {
                    warn!("Failed to backup corrupted model stats file: {}", backup_err);
                }
                return Ok(());
            }
        };

        let mut model_stats = self.model_stats.write().await;
        *model_stats = stats;

        info!("Loaded statistics for {} models", model_stats.len());
        Ok(())
    }

    /// Save model statistics to disk
    async fn save_model_stats(&self) -> Result<()> {
        let path = self.get_model_stats_path();
        let model_stats = self.model_stats.read().await;
        
        let content = serde_json::to_string_pretty(&*model_stats)
            .context("Failed to serialize model stats")?;
        
        fs::write(&path, content)
            .await
            .context("Failed to write model stats file")?;

        debug!("Saved statistics for {} models", model_stats.len());
        Ok(())
    }

    /// Record a token usage event
    pub async fn record_usage(
        &self,
        model_id: String,
        session_id: String,
        turn_id: String,
        input_tokens: u32,
        output_tokens: u32,
        cached_tokens: u32,
        is_subagent: bool,
    ) -> Result<()> {
        let now = Utc::now();
        let total_tokens = input_tokens + output_tokens;

        let record = TokenUsageRecord {
            model_id: model_id.clone(),
            session_id: session_id.clone(),
            turn_id,
            timestamp: now,
            input_tokens,
            output_tokens,
            cached_tokens,
            total_tokens,
            is_subagent,
        };

        // Update model statistics (all-time aggregation, includes everything)
        self.update_model_stats(&record).await?;

        // Update session cache
        self.update_session_cache(&record).await?;

        // Persist record to disk
        self.persist_record(&record).await?;

        debug!(
            "Recorded token usage: model={}, session={}, input={}, output={}, total={}, is_subagent={}",
            model_id, session_id, input_tokens, output_tokens, total_tokens, is_subagent
        );

        Ok(())
    }

    /// Update model statistics
    async fn update_model_stats(&self, record: &TokenUsageRecord) -> Result<()> {
        let mut model_stats = self.model_stats.write().await;
        
        let stats = model_stats
            .entry(record.model_id.clone())
            .or_insert_with(|| ModelTokenStats {
                model_id: record.model_id.clone(),
                ..Default::default()
            });

        stats.total_input += record.input_tokens as u64;
        stats.total_output += record.output_tokens as u64;
        stats.total_cached += record.cached_tokens as u64;
        stats.total_tokens += record.total_tokens as u64;
        stats.request_count += 1;

        // Track unique sessions
        if stats.session_ids.insert(record.session_id.clone()) {
            stats.session_count += 1;
        }

        if stats.first_used.is_none() {
            stats.first_used = Some(record.timestamp);
        }
        stats.last_used = Some(record.timestamp);

        drop(model_stats);

        // Save to disk
        self.save_model_stats().await?;

        Ok(())
    }

    /// Update session cache
    async fn update_session_cache(&self, record: &TokenUsageRecord) -> Result<()> {
        let mut session_cache = self.session_cache.write().await;
        
        let stats = session_cache
            .entry(record.session_id.clone())
            .or_insert_with(|| SessionTokenStats {
                session_id: record.session_id.clone(),
                model_id: record.model_id.clone(),
                total_input: 0,
                total_output: 0,
                total_cached: 0,
                total_tokens: 0,
                request_count: 0,
                created_at: record.timestamp,
                last_updated: record.timestamp,
            });

        stats.total_input += record.input_tokens;
        stats.total_output += record.output_tokens;
        stats.total_cached += record.cached_tokens;
        stats.total_tokens += record.total_tokens;
        stats.request_count += 1;
        stats.last_updated = record.timestamp;

        Ok(())
    }

    /// Persist record to disk
    async fn persist_record(&self, record: &TokenUsageRecord) -> Result<()> {
        let path = self.get_records_path(record.timestamp);
        
        // Load existing records for the day
        let mut batch = if path.exists() {
            let content = fs::read_to_string(&path).await?;
            serde_json::from_str::<RecordsBatch>(&content).unwrap_or_else(|_| RecordsBatch {
                records: Vec::new(),
            })
        } else {
            RecordsBatch {
                records: Vec::new(),
            }
        };

        // Add new record
        batch.records.push(record.clone());

        // Save back
        let content = serde_json::to_string_pretty(&batch)?;
        fs::write(&path, content).await?;

        Ok(())
    }

    /// Get statistics for a specific model
    pub async fn get_model_stats(&self, model_id: &str) -> Option<ModelTokenStats> {
        let model_stats = self.model_stats.read().await;
        model_stats.get(model_id).cloned()
    }

    /// Get statistics for a specific model with time range and subagent filter
    pub async fn get_model_stats_filtered(
        &self,
        model_id: &str,
        time_range: TimeRange,
        include_subagent: bool,
    ) -> Result<Option<ModelTokenStats>> {
        let query = TokenUsageQuery {
            model_id: Some(model_id.to_string()),
            session_id: None,
            time_range,
            limit: None,
            offset: None,
            include_subagent,
        };

        let records = self.query_records(query).await?;
        if records.is_empty() {
            return Ok(None);
        }

        let mut stats = ModelTokenStats {
            model_id: model_id.to_string(),
            ..Default::default()
        };

        for record in &records {
            stats.total_input += record.input_tokens as u64;
            stats.total_output += record.output_tokens as u64;
            stats.total_cached += record.cached_tokens as u64;
            stats.total_tokens += record.total_tokens as u64;
            stats.request_count += 1;
            stats.session_ids.insert(record.session_id.clone());

            if stats.first_used.is_none() || Some(record.timestamp) < stats.first_used {
                stats.first_used = Some(record.timestamp);
            }
            if stats.last_used.is_none() || Some(record.timestamp) > stats.last_used {
                stats.last_used = Some(record.timestamp);
            }
        }

        stats.session_count = stats.session_ids.len() as u32;
        Ok(Some(stats))
    }

    /// Get all model statistics
    pub async fn get_all_model_stats(&self) -> HashMap<String, ModelTokenStats> {
        let model_stats = self.model_stats.read().await;
        model_stats.clone()
    }

    /// Get statistics for a specific session
    pub async fn get_session_stats(&self, session_id: &str) -> Option<SessionTokenStats> {
        let session_cache = self.session_cache.read().await;
        session_cache.get(session_id).cloned()
    }

    /// Query token usage records
    pub async fn query_records(&self, query: TokenUsageQuery) -> Result<Vec<TokenUsageRecord>> {
        let (start_date, end_date) = self.get_date_range(&query.time_range);
        
        let mut all_records = Vec::new();
        let mut current_date = start_date;

        while current_date <= end_date {
            let path = self.get_records_path(current_date);
            
            if path.exists() {
                let content = fs::read_to_string(&path).await?;
                if let Ok(batch) = serde_json::from_str::<RecordsBatch>(&content) {
                    all_records.extend(batch.records);
                }
            }

            current_date = current_date + Duration::days(1);
        }

        // Filter by model_id, session_id, and subagent flag
        let include_subagent = query.include_subagent;
        let filtered: Vec<TokenUsageRecord> = all_records
            .into_iter()
            .filter(|r| {
                // Filter out subagent records unless explicitly included
                if !include_subagent && r.is_subagent {
                    return false;
                }
                if let Some(ref model_id) = query.model_id {
                    if &r.model_id != model_id {
                        return false;
                    }
                }
                if let Some(ref session_id) = query.session_id {
                    if &r.session_id != session_id {
                        return false;
                    }
                }
                true
            })
            .collect();

        // Apply pagination
        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(usize::MAX);
        
        Ok(filtered.into_iter().skip(offset).take(limit).collect())
    }

    /// Get date range from TimeRange enum
    fn get_date_range(&self, time_range: &TimeRange) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();
        // Fallback: use Unix epoch as start if date calculation fails
        let epoch = DateTime::UNIX_EPOCH;
        
        match time_range {
            TimeRange::Today => {
                let start = now
                    .date_naive()
                    .and_hms_opt(0, 0, 0)
                    .map(|t| t.and_utc())
                    .unwrap_or(epoch);
                (start, now)
            }
            TimeRange::ThisWeek => {
                let days_from_monday = now.weekday().num_days_from_monday();
                let start = (now - Duration::days(days_from_monday as i64))
                    .date_naive()
                    .and_hms_opt(0, 0, 0)
                    .map(|t| t.and_utc())
                    .unwrap_or(epoch);
                (start, now)
            }
            TimeRange::ThisMonth => {
                let start = now
                    .date_naive()
                    .with_day(1)
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|t| t.and_utc())
                    .unwrap_or(epoch);
                (start, now)
            }
            TimeRange::All => {
                (epoch, now)
            }
            TimeRange::Custom { start, end } => (*start, *end),
        }
    }

    /// Get summary statistics
    pub async fn get_summary(&self, query: TokenUsageQuery) -> Result<TokenUsageSummary> {
        let records = self.query_records(query).await?;
        
        let mut total_input = 0u64;
        let mut total_output = 0u64;
        let mut total_cached = 0u64;
        let mut total_tokens = 0u64;
        
        let mut by_model: HashMap<String, ModelTokenStats> = HashMap::new();
        let mut by_session: HashMap<String, SessionTokenStats> = HashMap::new();

        for record in &records {
            total_input += record.input_tokens as u64;
            total_output += record.output_tokens as u64;
            total_cached += record.cached_tokens as u64;
            total_tokens += record.total_tokens as u64;

            // Aggregate by model
            let model_stats = by_model
                .entry(record.model_id.clone())
                .or_insert_with(|| ModelTokenStats {
                    model_id: record.model_id.clone(),
                    ..Default::default()
                });
            
            model_stats.total_input += record.input_tokens as u64;
            model_stats.total_output += record.output_tokens as u64;
            model_stats.total_cached += record.cached_tokens as u64;
            model_stats.total_tokens += record.total_tokens as u64;
            model_stats.request_count += 1;
            model_stats.session_ids.insert(record.session_id.clone());

            if model_stats.first_used.is_none() || Some(record.timestamp) < model_stats.first_used {
                model_stats.first_used = Some(record.timestamp);
            }
            if model_stats.last_used.is_none() || Some(record.timestamp) > model_stats.last_used {
                model_stats.last_used = Some(record.timestamp);
            }

            // Aggregate by session
            let session_stats = by_session
                .entry(record.session_id.clone())
                .or_insert_with(|| SessionTokenStats {
                    session_id: record.session_id.clone(),
                    model_id: record.model_id.clone(),
                    total_input: 0,
                    total_output: 0,
                    total_cached: 0,
                    total_tokens: 0,
                    request_count: 0,
                    created_at: record.timestamp,
                    last_updated: record.timestamp,
                });
            
            session_stats.total_input += record.input_tokens;
            session_stats.total_output += record.output_tokens;
            session_stats.total_cached += record.cached_tokens;
            session_stats.total_tokens += record.total_tokens;
            session_stats.request_count += 1;
            
            if record.timestamp < session_stats.created_at {
                session_stats.created_at = record.timestamp;
            }
            if record.timestamp > session_stats.last_updated {
                session_stats.last_updated = record.timestamp;
            }
        }

        // Update session counts from session_ids set
        for stats in by_model.values_mut() {
            stats.session_count = stats.session_ids.len() as u32;
        }

        Ok(TokenUsageSummary {
            total_input,
            total_output,
            total_cached,
            total_tokens,
            by_model,
            by_session,
            record_count: records.len(),
        })
    }

    /// Clear statistics for a specific model
    pub async fn clear_model_stats(&self, model_id: &str) -> Result<()> {
        let mut model_stats = self.model_stats.write().await;
        model_stats.remove(model_id);
        drop(model_stats);

        self.save_model_stats().await?;
        
        info!("Cleared statistics for model: {}", model_id);
        Ok(())
    }

    /// Clear all statistics
    pub async fn clear_all_stats(&self) -> Result<()> {
        let mut model_stats = self.model_stats.write().await;
        model_stats.clear();
        drop(model_stats);

        let mut session_cache = self.session_cache.write().await;
        session_cache.clear();
        drop(session_cache);

        self.save_model_stats().await?;

        // Optionally delete all record files
        let records_dir = self.get_base_dir().join(RECORDS_DIR);
        if records_dir.exists() {
            fs::remove_dir_all(&records_dir).await?;
            fs::create_dir_all(&records_dir).await?;
        }

        info!("Cleared all token usage statistics");
        Ok(())
    }
}
