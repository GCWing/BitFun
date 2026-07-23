use crate::types::tick::{RawTick, SourceId, TickData};
use std::collections::{HashMap, HashSet};

/// 字段缺失信息
#[derive(Debug, Clone)]
pub struct FieldMissing {
    pub field_name: String,
}

/// 字段映射：源字段名 → 目标字段名
struct FieldMapping {
    source_field: String,
    target_field: String,
    required: bool,
}

/// SchemaAdapter：将各数据源的 RawTick 映射到标准 TickData。
/// 缺失字段不补默认值——对应 TickData 字段保持初始化值（0.0 / "" / None）。
pub struct SchemaAdapter {
    mappings: HashMap<SourceId, Vec<FieldMapping>>,
    available_fields: HashMap<SourceId, HashSet<String>>,
}

impl Default for SchemaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaAdapter {
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
            available_fields: HashMap::new(),
        }
    }

    /// 注册数据源的字段映射
    pub fn register_source(&mut self, source: SourceId, mappings: Vec<(&str, &str, bool)>) {
        let fields: HashSet<String> = mappings.iter().map(|(src, _, _)| src.to_string()).collect();
        self.available_fields.insert(source.clone(), fields);
        self.mappings.insert(
            source,
            mappings
                .into_iter()
                .map(|(src, tgt, req)| FieldMapping {
                    source_field: src.to_string(),
                    target_field: tgt.to_string(),
                    required: req,
                })
                .collect(),
        );
    }

    /// 将 RawTick 映射到 TickData。缺失字段保持默认值。
    pub fn adapt(&self, source: &SourceId, raw: RawTick) -> (TickData, Vec<FieldMissing>) {
        let mut tick = TickData {
            instrument: raw.instrument.clone(),
            timestamp_ms: raw.timestamp,
            ..Default::default()
        };

        let mut missing: Vec<FieldMissing> = Vec::new();

        if let Some(mappings) = self.mappings.get(source) {
            for mapping in mappings {
                if let Some(&value) = raw.fields.get(&mapping.source_field) {
                    Self::set_field(&mut tick, &mapping.target_field, value);
                } else if mapping.required {
                    missing.push(FieldMissing {
                        field_name: mapping.source_field.clone(),
                    });
                }
                // non-required missing fields: silently skip, leave as default
            }
        }

        (tick, missing)
    }

    fn set_field(tick: &mut TickData, field: &str, value: f64) {
        // Reject NaN/Inf to prevent propagation through the computation chain
        if !value.is_finite() {
            return;
        }
        match field {
            "last_price" => tick.last_price = value,
            "open_price" => tick.open_price = value,
            "highest_price" => tick.highest_price = value,
            "lowest_price" => tick.lowest_price = value,
            "volume" => tick.volume = value,
            "turnover" => tick.turnover = value,
            "open_interest" => tick.open_interest = value,
            "close_price" => tick.close_price = value,
            "pre_settlement_price" => tick.pre_settlement_price = value,
            "pre_close_price" => tick.pre_close_price = value,
            "pre_open_interest" => tick.pre_open_interest = value,
            "settlement_price" => tick.settlement_price = value,
            "upper_limit_price" => tick.upper_limit_price = value,
            "lower_limit_price" => tick.lower_limit_price = value,
            "pre_delta" => tick.pre_delta = value,
            "curr_delta" => tick.curr_delta = value,
            "average_price" => tick.average_price = value,
            _ => {} // unknown field, skip
        }
    }

    /// 列出某数据源缺失的字段
    pub fn missing_fields(&self, source: &SourceId, raw: &RawTick) -> Vec<String> {
        if let Some(mappings) = self.mappings.get(source) {
            mappings
                .iter()
                .filter(|m| !raw.fields.contains_key(&m.source_field))
                .map(|m| m.source_field.clone())
                .collect()
        } else {
            vec![]
        }
    }
}
