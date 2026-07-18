use super::types::{
    ModelTokenStats, SessionTokenStats, TimeRange, TokenUsageQuery, TokenUsageRecord,
    TokenUsageSummary,
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{Mutex, RwLock};

const MODEL_STATS_FILE: &str = "model_stats.json";
const RECORDS_DIR: &str = "records";

pub struct TokenUsageService {
    base_dir: PathBuf,
    model_stats: Arc<RwLock<HashMap<String, ModelTokenStats>>>,
    session_cache: Arc<RwLock<HashMap<String, SessionTokenStats>>>,
    record_batches: Arc<Mutex<HashMap<String, Arc<Mutex<RecordsBatch>>>>>,
    usage_lifecycle: Arc<RwLock<()>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecordsBatch {
    records: Vec<TokenUsageRecord>,
}

impl TokenUsageService {
    pub async fn new(base_dir: PathBuf) -> Result<Self, String> {
        let service = Self {
            base_dir,
            model_stats: Arc::new(RwLock::new(HashMap::new())),
            session_cache: Arc::new(RwLock::new(HashMap::new())),
            record_batches: Arc::new(Mutex::new(HashMap::new())),
            usage_lifecycle: Arc::new(RwLock::new(())),
        };

        service.init_storage().await?;
        service.load_model_stats().await?;

        info!("Token usage service initialized");
        Ok(service)
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    async fn init_storage(&self) -> Result<(), String> {
        let records_dir = self.base_dir.join(RECORDS_DIR);

        fs::create_dir_all(&self.base_dir)
            .await
            .map_err(|e| format!("Failed to create token usage directory: {}", e))?;
        fs::create_dir_all(&records_dir)
            .await
            .map_err(|e| format!("Failed to create records directory: {}", e))?;

        debug!("Token usage storage initialized at: {:?}", self.base_dir);
        Ok(())
    }

    fn get_model_stats_path(&self) -> PathBuf {
        self.base_dir.join(MODEL_STATS_FILE)
    }

    fn get_records_path(&self, date: DateTime<Utc>) -> PathBuf {
        let filename = format!("{}.json", records_date_key(date));
        self.base_dir.join(RECORDS_DIR).join(filename)
    }

    fn get_records_path_for_key(&self, date_key: &str) -> PathBuf {
        self.base_dir
            .join(RECORDS_DIR)
            .join(format!("{}.json", date_key))
    }

    async fn load_model_stats(&self) -> Result<(), String> {
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
                let backup_path = path.with_extension("json.bak");
                if let Err(backup_err) = fs::rename(&path, &backup_path).await {
                    warn!(
                        "Failed to backup corrupted model stats file: {}",
                        backup_err
                    );
                }
                return Ok(());
            }
        };

        let mut model_stats = self.model_stats.write().await;
        *model_stats = stats;

        info!("Loaded statistics for {} models", model_stats.len());
        Ok(())
    }

    async fn save_model_stats(&self) -> Result<(), String> {
        let path = self.get_model_stats_path();
        let model_stats = self.model_stats.read().await;

        let content = serde_json::to_string_pretty(&*model_stats)
            .map_err(|e| format!("Failed to serialize model stats: {}", e))?;

        fs::write(&path, content)
            .await
            .map_err(|e| format!("Failed to write model stats file: {}", e))?;

        debug!("Saved statistics for {} models", model_stats.len());
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_usage(
        &self,
        model_config_id: String,
        effective_model_name: String,
        session_id: String,
        turn_id: String,
        input_tokens: u32,
        output_tokens: u32,
        cached_tokens: Option<u32>,
        token_details: Option<serde_json::Value>,
        is_subagent: bool,
    ) -> Result<(), String> {
        let _usage_guard = self.usage_lifecycle.read().await;
        let now = Utc::now();
        let total_tokens = input_tokens + output_tokens;
        let cached_tokens_available = cached_tokens.is_some();
        let cached_tokens = cached_tokens.unwrap_or(0);
        let cache_write_tokens: u32 = token_details
            .as_ref()
            .and_then(|details| details.get("cacheCreationTokenCount"))
            .and_then(|value| value.as_u64())
            .map(|value| value as u32)
            .unwrap_or(0);

        let record = TokenUsageRecord {
            model_config_id: model_config_id.clone(),
            effective_model_name: effective_model_name.clone(),
            session_id: session_id.clone(),
            turn_id,
            timestamp: now,
            input_tokens,
            output_tokens,
            cached_tokens,
            cached_tokens_available,
            cache_write_tokens,
            total_tokens,
            token_details,
            is_subagent,
        };

        self.update_model_stats(&record).await?;
        self.update_session_cache(&record).await?;
        self.persist_record(&record).await?;

        debug!(
            "Recorded token usage: model_config_id={}, effective_model_name={}, session={}, input={}, output={}, total={}, is_subagent={}",
            model_config_id,
            effective_model_name,
            session_id,
            input_tokens,
            output_tokens,
            total_tokens,
            is_subagent
        );

        Ok(())
    }

    async fn update_model_stats(&self, record: &TokenUsageRecord) -> Result<(), String> {
        let mut model_stats = self.model_stats.write().await;

        let stats = model_stats
            .entry(record.effective_model_name.clone())
            .or_insert_with(|| ModelTokenStats {
                model_id: record.effective_model_name.clone(),
                ..Default::default()
            });

        stats.total_input += record.input_tokens as u64;
        stats.total_output += record.output_tokens as u64;
        stats.total_cached += record.cached_tokens as u64;
        stats.total_cache_write += record.cache_write_tokens as u64;
        if record.cached_tokens_available {
            stats.cache_reported_input_tokens += record.input_tokens as u64;
        }
        stats.total_tokens += record.total_tokens as u64;
        stats.request_count += 1;

        if stats.session_ids.insert(record.session_id.clone()) {
            stats.session_count += 1;
        }

        if stats.first_used.is_none() {
            stats.first_used = Some(record.timestamp);
        }
        stats.last_used = Some(record.timestamp);

        drop(model_stats);
        self.save_model_stats().await
    }

    async fn update_session_cache(&self, record: &TokenUsageRecord) -> Result<(), String> {
        let mut session_cache = self.session_cache.write().await;

        let stats = session_cache
            .entry(record.session_id.clone())
            .or_insert_with(|| SessionTokenStats {
                session_id: record.session_id.clone(),
                model_id: record.effective_model_name.clone(),
                total_input: 0,
                total_output: 0,
                total_cached: 0,
                total_cache_write: 0,
                cache_reported_input_tokens: 0,
                total_tokens: 0,
                request_count: 0,
                created_at: record.timestamp,
                last_updated: record.timestamp,
            });

        stats.total_input += record.input_tokens;
        stats.total_output += record.output_tokens;
        stats.total_cached += record.cached_tokens;
        stats.total_cache_write += record.cache_write_tokens;
        if record.cached_tokens_available {
            stats.cache_reported_input_tokens += record.input_tokens;
        }
        stats.total_tokens += record.total_tokens;
        stats.request_count += 1;
        stats.last_updated = record.timestamp;

        Ok(())
    }

    async fn persist_record(&self, record: &TokenUsageRecord) -> Result<(), String> {
        let date_key = records_date_key(record.timestamp);
        let path = self.get_records_path_for_key(&date_key);
        let batch_lock = self.record_batch_for_key(&date_key, &path).await?;
        let mut batch = batch_lock.lock().await;
        let previous_len = batch.records.len();
        batch.records.push(record.clone());

        let content = match serde_json::to_string_pretty(&*batch) {
            Ok(content) => content,
            Err(e) => {
                batch.records.truncate(previous_len);
                return Err(format!("Failed to serialize token usage records: {}", e));
            }
        };

        if let Err(e) = fs::write(&path, content).await {
            batch.records.truncate(previous_len);
            return Err(format!("Failed to write token usage records: {}", e));
        }

        Ok(())
    }

    async fn record_batch_for_key(
        &self,
        date_key: &str,
        path: &Path,
    ) -> Result<Arc<Mutex<RecordsBatch>>, String> {
        {
            let record_batches = self.record_batches.lock().await;
            if let Some(batch) = record_batches.get(date_key) {
                return Ok(Arc::clone(batch));
            }
        }

        let loaded_batch = read_records_batch(path).await?;
        let mut record_batches = self.record_batches.lock().await;
        Ok(Arc::clone(
            record_batches
                .entry(date_key.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(loaded_batch))),
        ))
    }

    pub async fn get_model_stats(&self, model_id: &str) -> Option<ModelTokenStats> {
        let model_stats = self.model_stats.read().await;
        model_stats.get(model_id).cloned()
    }

    pub async fn get_model_stats_filtered(
        &self,
        model_id: &str,
        time_range: TimeRange,
        include_subagent: bool,
    ) -> Result<Option<ModelTokenStats>, String> {
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
            stats.total_cache_write += record.cache_write_tokens as u64;
            if record.cached_tokens_available {
                stats.cache_reported_input_tokens += record.input_tokens as u64;
            }
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

    pub async fn get_all_model_stats(&self) -> HashMap<String, ModelTokenStats> {
        let model_stats = self.model_stats.read().await;
        model_stats.clone()
    }

    pub async fn get_session_stats(&self, session_id: &str) -> Option<SessionTokenStats> {
        let session_cache = self.session_cache.read().await;
        session_cache.get(session_id).cloned()
    }

    pub async fn query_records(
        &self,
        query: TokenUsageQuery,
    ) -> Result<Vec<TokenUsageRecord>, String> {
        self.query_records_filtered(query, None).await
    }

    /// Query records while bounding the returned and retained set to the
    /// supplied session identities. The date files are still scanned once,
    /// but unrelated records never accumulate in the request's working set.
    pub async fn query_records_for_sessions(
        &self,
        query: TokenUsageQuery,
        session_ids: &HashSet<String>,
    ) -> Result<Vec<TokenUsageRecord>, String> {
        self.query_records_filtered(query, Some(session_ids)).await
    }

    async fn query_records_filtered(
        &self,
        query: TokenUsageQuery,
        session_ids: Option<&HashSet<String>>,
    ) -> Result<Vec<TokenUsageRecord>, String> {
        let _usage_guard = self.usage_lifecycle.read().await;
        let record_paths = self.record_paths_for_range(&query.time_range).await?;
        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(usize::MAX);
        if limit == 0 {
            return Ok(Vec::new());
        }
        let mut matched = 0usize;
        let mut records = Vec::new();

        for path in record_paths {
            let content = fs::read_to_string(&path)
                .await
                .map_err(|e| format!("Failed to read token usage records: {}", e))?;
            if let Ok(batch) = serde_json::from_str::<RecordsBatch>(&content) {
                for record in batch.records {
                    if !query.include_subagent && record.is_subagent {
                        continue;
                    }
                    if query
                        .model_id
                        .as_ref()
                        .is_some_and(|model_id| &record.effective_model_name != model_id)
                    {
                        continue;
                    }
                    if query
                        .session_id
                        .as_ref()
                        .is_some_and(|session_id| &record.session_id != session_id)
                    {
                        continue;
                    }
                    if session_ids
                        .is_some_and(|session_ids| !session_ids.contains(&record.session_id))
                    {
                        continue;
                    }
                    if matched < offset {
                        matched += 1;
                        continue;
                    }
                    records.push(record);
                    if records.len() == limit {
                        return Ok(records);
                    }
                }
            }
        }
        Ok(records)
    }

    async fn record_paths_for_range(&self, time_range: &TimeRange) -> Result<Vec<PathBuf>, String> {
        if matches!(time_range, TimeRange::All) {
            return self.all_record_paths().await;
        }

        let (start_date, end_date) = self.get_date_range(time_range);
        let mut paths = Vec::new();
        let mut current_date = start_date;

        while current_date <= end_date {
            let path = self.get_records_path(current_date);
            if fs::try_exists(&path).await.unwrap_or(false) {
                paths.push(path);
            }
            current_date += Duration::days(1);
        }

        Ok(paths)
    }

    async fn all_record_paths(&self) -> Result<Vec<PathBuf>, String> {
        let records_dir = self.base_dir.join(RECORDS_DIR);
        let mut read_dir = match fs::read_dir(&records_dir).await {
            Ok(read_dir) => read_dir,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(format!("Failed to read token usage records dir: {}", error));
            }
        };

        let mut paths = Vec::new();
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| format!("Failed to read token usage record entry: {}", e))?
        {
            let path = entry.path();
            if is_token_usage_record_path(&path) {
                paths.push(path);
            }
        }
        paths.sort();
        Ok(paths)
    }

    fn get_date_range(&self, time_range: &TimeRange) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();
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
            TimeRange::All => (epoch, now),
            TimeRange::Custom { start, end } => (*start, *end),
        }
    }

    pub async fn get_summary(&self, query: TokenUsageQuery) -> Result<TokenUsageSummary, String> {
        let records = self.query_records(query).await?;

        let mut total_input = 0u64;
        let mut total_output = 0u64;
        let mut total_cached = 0u64;
        let mut total_cache_write = 0u64;
        let mut cache_reported_input_tokens = 0u64;
        let mut total_tokens = 0u64;

        let mut by_model: HashMap<String, ModelTokenStats> = HashMap::new();
        let mut by_session: HashMap<String, SessionTokenStats> = HashMap::new();

        for record in &records {
            total_input += record.input_tokens as u64;
            total_output += record.output_tokens as u64;
            total_cached += record.cached_tokens as u64;
            total_cache_write += record.cache_write_tokens as u64;
            if record.cached_tokens_available {
                cache_reported_input_tokens += record.input_tokens as u64;
            }
            total_tokens += record.total_tokens as u64;

            let model_stats = by_model
                .entry(record.effective_model_name.clone())
                .or_insert_with(|| ModelTokenStats {
                    model_id: record.effective_model_name.clone(),
                    ..Default::default()
                });

            model_stats.total_input += record.input_tokens as u64;
            model_stats.total_output += record.output_tokens as u64;
            model_stats.total_cached += record.cached_tokens as u64;
            model_stats.total_cache_write += record.cache_write_tokens as u64;
            if record.cached_tokens_available {
                model_stats.cache_reported_input_tokens += record.input_tokens as u64;
            }
            model_stats.total_tokens += record.total_tokens as u64;
            model_stats.request_count += 1;
            model_stats.session_ids.insert(record.session_id.clone());

            if model_stats.first_used.is_none() || Some(record.timestamp) < model_stats.first_used {
                model_stats.first_used = Some(record.timestamp);
            }
            if model_stats.last_used.is_none() || Some(record.timestamp) > model_stats.last_used {
                model_stats.last_used = Some(record.timestamp);
            }

            let session_stats = by_session
                .entry(record.session_id.clone())
                .or_insert_with(|| SessionTokenStats {
                    session_id: record.session_id.clone(),
                    model_id: record.effective_model_name.clone(),
                    total_input: 0,
                    total_output: 0,
                    total_cached: 0,
                    total_cache_write: 0,
                    cache_reported_input_tokens: 0,
                    total_tokens: 0,
                    request_count: 0,
                    created_at: record.timestamp,
                    last_updated: record.timestamp,
                });

            session_stats.total_input += record.input_tokens;
            session_stats.total_output += record.output_tokens;
            session_stats.total_cached += record.cached_tokens;
            session_stats.total_cache_write += record.cache_write_tokens;
            if record.cached_tokens_available {
                session_stats.cache_reported_input_tokens += record.input_tokens;
            }
            session_stats.total_tokens += record.total_tokens;
            session_stats.request_count += 1;

            if record.timestamp < session_stats.created_at {
                session_stats.created_at = record.timestamp;
            }
            if record.timestamp > session_stats.last_updated {
                session_stats.last_updated = record.timestamp;
            }
        }

        for stats in by_model.values_mut() {
            stats.session_count = stats.session_ids.len() as u32;
        }

        Ok(TokenUsageSummary {
            total_input,
            total_output,
            total_cached,
            total_cache_write,
            cache_reported_input_tokens,
            total_tokens,
            by_model,
            by_session,
            record_count: records.len(),
        })
    }

    pub async fn clear_model_stats(&self, model_id: &str) -> Result<(), String> {
        let _usage_guard = self.usage_lifecycle.write().await;
        let mut model_stats = self.model_stats.write().await;
        model_stats.remove(model_id);
        drop(model_stats);

        self.save_model_stats().await?;

        info!("Cleared statistics for model: {}", model_id);
        Ok(())
    }

    pub async fn clear_all_stats(&self) -> Result<(), String> {
        let _usage_guard = self.usage_lifecycle.write().await;
        let mut model_stats = self.model_stats.write().await;
        model_stats.clear();
        drop(model_stats);

        let mut session_cache = self.session_cache.write().await;
        session_cache.clear();
        drop(session_cache);

        self.save_model_stats().await?;

        let mut record_batches = self.record_batches.lock().await;
        record_batches.clear();

        let records_dir = self.base_dir.join(RECORDS_DIR);
        if records_dir.exists() {
            fs::remove_dir_all(&records_dir)
                .await
                .map_err(|e| format!("Failed to delete records directory: {}", e))?;
            fs::create_dir_all(&records_dir)
                .await
                .map_err(|e| format!("Failed to recreate records directory: {}", e))?;
        }
        drop(record_batches);

        info!("Cleared all token usage statistics");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[tokio::test]
    async fn query_records_for_sessions_filters_before_returning_records() {
        let dir = tempfile::tempdir().expect("temp dir");
        let service = TokenUsageService::new(dir.path().to_path_buf())
            .await
            .expect("token usage service");
        service
            .record_usage(
                "model-config-a".to_string(),
                "model-a".to_string(),
                "parent-session".to_string(),
                "parent-turn".to_string(),
                10,
                2,
                None,
                None,
                false,
            )
            .await
            .expect("parent record");
        service
            .record_usage(
                "model-config-b".to_string(),
                "model-b".to_string(),
                "unrelated-session".to_string(),
                "unrelated-turn".to_string(),
                20,
                4,
                None,
                None,
                false,
            )
            .await
            .expect("unrelated record");

        let records = service
            .query_records_for_sessions(
                TokenUsageQuery {
                    model_id: None,
                    session_id: None,
                    time_range: TimeRange::All,
                    limit: None,
                    offset: None,
                    include_subagent: true,
                },
                &HashSet::from(["parent-session".to_string()]),
            )
            .await
            .expect("scoped records");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].session_id, "parent-session");
    }
}

fn records_date_key(date: DateTime<Utc>) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn is_token_usage_record_path(path: &Path) -> bool {
    if path.extension().and_then(|value| value.to_str()) != Some("json") {
        return false;
    }

    path.file_stem()
        .and_then(|value| value.to_str())
        .is_some_and(|stem| NaiveDate::parse_from_str(stem, "%Y-%m-%d").is_ok())
}

async fn read_records_batch(path: &Path) -> Result<RecordsBatch, String> {
    if !fs::try_exists(path).await.unwrap_or(false) {
        return Ok(RecordsBatch {
            records: Vec::new(),
        });
    }

    let content = fs::read_to_string(path)
        .await
        .map_err(|e| format!("Failed to read token usage records: {}", e))?;
    Ok(
        serde_json::from_str::<RecordsBatch>(&content).unwrap_or_else(|_| RecordsBatch {
            records: Vec::new(),
        }),
    )
}
