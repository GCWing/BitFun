use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

use crate::error::{Result, TaijiError};
use crate::source::adapter::SchemaAdapter;
use crate::source::datasource::{DataSource, DataSourceConfig, FieldDef, SourceHealth};
use crate::source::validator::{TickStatus, TickValidator};
use crate::types::tick::{RawTick, SourceId};

/// CSV 回放数据源——从 golden_tick CSV 文件中逐行回放历史行情。
///
/// 列映射（CSV → TickData）：
///   symbol → instrument, created_at → timestamp, price → last_price,
///   open → open_price, high → highest_price, low → lowest_price,
///   cum_volume → volume, cum_amount → turnover, cum_position → open_interest
pub struct CsvReplaySource {
    csv_path: PathBuf,
    column_map: HashMap<String, usize>,
    current_line: u64,
    adapter: SchemaAdapter,
    validator: TickValidator,
    source_id: SourceId,
    reader: Option<csv::Reader<File>>,
    headers: Vec<String>,
    subscribed: Vec<String>,
}

impl CsvReplaySource {
    pub fn new(csv_path: impl Into<PathBuf>, source_id: SourceId) -> Self {
        Self {
            csv_path: csv_path.into(),
            column_map: HashMap::new(),
            current_line: 0,
            adapter: SchemaAdapter::new(),
            validator: TickValidator::new(),
            source_id,
            reader: None,
            headers: Vec::new(),
            subscribed: Vec::new(),
        }
    }

    fn parse_timestamp(s: &str) -> Result<i64> {
        let dt = chrono::DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f%:z")
            .map_err(|e| TaijiError::DataSource(format!("invalid timestamp '{}': {}", s, e)))?;
        Ok(dt.timestamp_millis())
    }

    fn extract_instrument(symbol: &str) -> &str {
        symbol.split('.').nth(1).unwrap_or(symbol)
    }

    fn register_adapter_mappings(&mut self) {
        self.adapter.register_source(
            self.source_id.clone(),
            vec![
                ("price", "last_price", true),
                ("open", "open_price", true),
                ("high", "highest_price", true),
                ("low", "lowest_price", true),
                ("cum_volume", "volume", true),
                ("cum_amount", "turnover", true),
                ("cum_position", "open_interest", true),
            ],
        );
    }
}

impl DataSource for CsvReplaySource {
    fn name(&self) -> &'static str {
        "csv_replay"
    }

    fn schema(&self) -> Vec<FieldDef> {
        self.headers
            .iter()
            .map(|h| FieldDef {
                name: h.clone(),
                required: false,
            })
            .collect()
    }

    fn connect(&mut self, config: &DataSourceConfig) -> Result<()> {
        if let Some(path) = config.params.get("csv_path") {
            if let Some(s) = path.as_str() {
                self.csv_path = PathBuf::from(s);
            }
        }

        let file = File::open(&self.csv_path).map_err(|e| {
            TaijiError::DataSource(format!("cannot open {}: {}", self.csv_path.display(), e))
        })?;

        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(file);

        let headers = rdr
            .headers()
            .map_err(|e| TaijiError::DataSource(format!("failed to read CSV headers: {}", e)))?;

        self.headers = headers.iter().map(|s| s.to_string()).collect();
        self.column_map.clear();
        for (i, h) in self.headers.iter().enumerate() {
            self.column_map.insert(h.clone(), i);
        }

        self.register_adapter_mappings();
        self.reader = Some(rdr);
        self.current_line = 0;

        Ok(())
    }

    fn disconnect(&mut self) -> Result<()> {
        self.reader = None;
        self.column_map.clear();
        self.headers.clear();
        self.current_line = 0;
        Ok(())
    }

    fn subscribe(&mut self, instruments: &[&str]) -> Result<()> {
        self.subscribed = instruments.iter().map(|s| s.to_string()).collect();
        Ok(())
    }

    fn next_raw(&mut self) -> Result<Option<RawTick>> {
        loop {
            let rdr = self
                .reader
                .as_mut()
                .ok_or_else(|| TaijiError::DataSource("CSV reader not connected".into()))?;

            let mut record = csv::StringRecord::new();
            let has_record = rdr
                .read_record(&mut record)
                .map_err(|e| TaijiError::DataSource(format!("CSV read error: {}", e)))?;

            if !has_record {
                return Ok(None);
            }

            self.current_line += 1;

            let symbol = record.get(0).unwrap_or("");
            let instrument = Self::extract_instrument(symbol).to_string();

            // Filter by subscribed instruments
            if !self.subscribed.is_empty() && !self.subscribed.contains(&instrument) {
                continue;
            }

            let ts_str = record.get(1).unwrap_or("");
            let timestamp = Self::parse_timestamp(ts_str)?;

            let mut fields: HashMap<String, f64> = HashMap::new();
            for (i, header) in self.headers.iter().enumerate() {
                if i <= 1 {
                    continue;
                }
                if let Some(val_str) = record.get(i) {
                    if val_str.is_empty() {
                        continue;
                    }
                    if let Ok(v) = val_str.parse::<f64>() {
                        fields.insert(header.clone(), v);
                    }
                }
            }

            let raw = RawTick {
                instrument: instrument.clone(),
                source_id: self.source_id.clone(),
                fields,
                timestamp,
                sequence: Some(self.current_line),
            };

            // Adapter validation: mapping check
            let (tick, _missing) = self.adapter.adapt(&self.source_id, raw.clone());

            // Tick quality validation
            let status = self
                .validator
                .validate(&instrument, &tick, self.current_line);
            if matches!(status, TickStatus::Rejected(_)) {
                continue;
            }

            return Ok(Some(raw));
        }
    }

    fn health_check(&self) -> SourceHealth {
        if self.reader.is_none() {
            return SourceHealth::Degraded("not connected".into());
        }
        if !self.csv_path.exists() {
            return SourceHealth::Degraded(format!(
                "CSV file missing: {}",
                self.csv_path.display()
            ));
        }
        SourceHealth::Healthy
    }

    fn supports_resume(&self) -> bool {
        true
    }

    fn last_sequence(&self, _instrument: &str) -> Option<u64> {
        Some(self.current_line)
    }

    fn resume_from(&mut self, _instrument: &str, seq: u64) -> Result<()> {
        let file = File::open(&self.csv_path).map_err(|e| {
            TaijiError::DataSource(format!("cannot reopen {}: {}", self.csv_path.display(), e))
        })?;

        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(file);

        let headers = rdr
            .headers()
            .map_err(|e| TaijiError::DataSource(format!("failed to read CSV headers: {}", e)))?;

        self.headers = headers.iter().map(|s| s.to_string()).collect();
        self.column_map.clear();
        for (i, h) in self.headers.iter().enumerate() {
            self.column_map.insert(h.clone(), i);
        }

        self.register_adapter_mappings();

        let mut record = csv::StringRecord::new();
        let mut skipped: u64 = 0;
        while skipped < seq {
            let has_record = rdr.read_record(&mut record).map_err(|e| {
                TaijiError::DataSource(format!("CSV read error during seek: {}", e))
            })?;
            if !has_record {
                return Err(TaijiError::DataSource(format!(
                    "cannot resume to line {}: file has only {} data rows",
                    seq, skipped
                )));
            }
            skipped += 1;
        }

        self.validator.reset(_instrument);
        self.reader = Some(rdr);
        self.current_line = seq;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden_csv_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../test_data/golden_tick/20260721/a2609/a2609_golden_20260721.csv")
    }

    fn connect_source(path: &PathBuf) -> CsvReplaySource {
        let mut source = CsvReplaySource::new(path, "csv:test".into());
        let config = DataSourceConfig {
            type_name: "csv_replay".into(),
            params: HashMap::new(),
        };
        source.connect(&config).unwrap();
        source
    }

    #[test]
    fn test_csv_replay_fields_non_empty() {
        let path = golden_csv_path();
        let mut source = connect_source(&path);

        let raw = source
            .next_raw()
            .unwrap()
            .expect("should have at least one tick");

        assert!(!raw.instrument.is_empty(), "instrument should not be empty");
        assert_eq!(raw.instrument, "a2609");
        assert!(raw.timestamp > 0, "timestamp should be positive");
        assert_eq!(raw.sequence, Some(1));

        // Verify adapter mapping via adapt
        let (tick, _missing) = source.adapter.adapt(&"csv:test".into(), raw);
        assert!(tick.last_price > 0.0, "last_price should be non-zero");
        assert!(tick.open_price > 0.0, "open_price should be non-zero");
        assert!(tick.highest_price > 0.0, "highest_price should be non-zero");
        assert!(tick.lowest_price > 0.0, "lowest_price should be non-zero");
        assert!(tick.volume > 0.0, "volume should be non-zero");
        assert!(tick.turnover > 0.0, "turnover should be non-zero");
        assert!(tick.open_interest > 0.0, "open_interest should be non-zero");
    }

    #[test]
    fn test_resume_from_100() {
        let path = golden_csv_path();
        let mut source = connect_source(&path);

        source.resume_from("a2609", 100).unwrap();

        let raw = source
            .next_raw()
            .unwrap()
            .expect("should have tick at line 101");
        assert_eq!(
            raw.sequence,
            Some(101),
            "resume_from(100) → next should be line 101"
        );
        assert_eq!(raw.instrument, "a2609");
    }

    #[test]
    fn test_file_not_found_degraded() {
        let source = CsvReplaySource::new("nonexistent_file_12345.csv", "csv:test".into());
        let health = source.health_check();
        match health {
            SourceHealth::Degraded(_) => {}
            other => panic!("expected Degraded, got {:?}", other),
        }
    }

    #[test]
    fn test_resume_preserves_headers() {
        let path = golden_csv_path();
        let mut source = connect_source(&path);

        source.resume_from("a2609", 50).unwrap();

        // Column map should be rebuilt after resume
        assert!(source.column_map.contains_key("price"));
        assert!(source.column_map.contains_key("open"));
        assert!(source.column_map.contains_key("high"));
        assert!(source.column_map.contains_key("low"));
        assert!(source.column_map.contains_key("cum_volume"));
        assert!(source.column_map.contains_key("cum_amount"));
        assert!(source.column_map.contains_key("cum_position"));
    }
}
