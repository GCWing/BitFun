//! ScorecardFusionNode — 5 指标加权融合评分卡。
//!
//! 权重：vol_regime=0.25, vol_anomaly=0.20, corr_fracture=0.15, gap_alert=0.25, trend_accel=0.15
//! 阈值：warn=70 → 告警, reduce=85 → 降仓位, emergency=95 → 熔断
//! 输出：abnormal:score (f64) + abnormal:alert_level (JSON string)

use crate::{AbnormalWeights, AbnormalLevel, AlertThresholds};
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::state::{StateKey, StateValue};

const OUTPUT_SCORE_KEY: &str = "abnormal:score";
const OUTPUT_LEVEL_KEY: &str = "abnormal:alert_level";

/// 5 个指标在 StateStore 中的 key
const INDICATOR_KEYS: [&str; 5] = [
    "abnormal:vol_regime",
    "abnormal:vol_anomaly",
    "abnormal:corr_fracture",
    "abnormal:gap_alert",
    "abnormal:trend_accel",
];

pub struct ScorecardFusionNode {
    id: NodeId,
    weights: AbnormalWeights,
    alert_thresholds: AlertThresholds,
}

impl ScorecardFusionNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            weights: AbnormalWeights::default(),
            alert_thresholds: AlertThresholds::default(),
        }
    }

    pub fn with_config(
        id: NodeId,
        weights: AbnormalWeights,
        alert_thresholds: AlertThresholds,
    ) -> Self {
        Self {
            id,
            weights,
            alert_thresholds,
        }
    }

    /// 读取 5 个指标分数，返回加权融合分数
    fn read_scores(&self, state: &StateStore) -> [f64; 5] {
        let mut scores = [0.0_f64; 5];
        for (i, key) in INDICATOR_KEYS.iter().enumerate() {
            scores[i] = state.get::<f64>(&(*key).into()).unwrap_or(0.0);
        }
        scores
    }

    /// 加权融合
    fn fuse(&self, scores: &[f64; 5]) -> f64 {
        self.weights.vol_regime * scores[0]
            + self.weights.vol_anomaly * scores[1]
            + self.weights.corr_fracture * scores[2]
            + self.weights.gap_alert * scores[3]
            + self.weights.trend_accel * scores[4]
    }
}

impl ComputeNode for ScorecardFusionNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "ScorecardFusionNode"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        INDICATOR_KEYS.iter().map(|k| (*k).into()).collect()
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec![OUTPUT_SCORE_KEY.into(), OUTPUT_LEVEL_KEY.into()]
    }

    fn on_init(&mut self, config: &NodeConfig, _state: &StateStore) -> Result<()> {
        // 支持通过 config 覆盖权重和阈值
        if let Some(vol_regime) = config.get_f64("vol_regime_weight") {
            self.weights.vol_regime = vol_regime;
        }
        if let Some(vol_anomaly) = config.get_f64("vol_anomaly_weight") {
            self.weights.vol_anomaly = vol_anomaly;
        }
        if let Some(corr_fracture) = config.get_f64("corr_fracture_weight") {
            self.weights.corr_fracture = corr_fracture;
        }
        if let Some(gap_alert) = config.get_f64("gap_alert_weight") {
            self.weights.gap_alert = gap_alert;
        }
        if let Some(trend_accel) = config.get_f64("trend_accel_weight") {
            self.weights.trend_accel = trend_accel;
        }
        if let Some(warn) = config.get_f64("warn_threshold") {
            self.alert_thresholds.warn = warn;
        }
        if let Some(reduce) = config.get_f64("reduce_threshold") {
            self.alert_thresholds.reduce = reduce;
        }
        if let Some(emergency) = config.get_f64("emergency_threshold") {
            self.alert_thresholds.emergency = emergency;
        }
        Ok(())
    }

    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, state: &StateStore) -> Result<()> {
        let scores = self.read_scores(state);
        let abnormal_score = self.fuse(&scores).clamp(0.0, 100.0);
        let level = AbnormalLevel::from_score(abnormal_score, &self.alert_thresholds);

        state.set(
            OUTPUT_SCORE_KEY.into(),
            StateValue::F64(abnormal_score),
            self.id(),
        );

        // 告警等级存为 JSON string
        let level_str = match level {
            AbnormalLevel::Normal => "normal",
            AbnormalLevel::Warn => "warn",
            AbnormalLevel::Reduce => "reduce",
            AbnormalLevel::Emergency => "emergency",
        };
        state.set(
            OUTPUT_LEVEL_KEY.into(),
            StateValue::Json(serde_json::Value::String(level_str.into())),
            self.id(),
        );
        Ok(())
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::D]
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use taiji_engine::types::bar::Symbol;

    fn bar() -> RawBar {
        RawBar {
            symbol: Symbol::from("TEST"),
            dt: Utc::now(),
            freq: Freq::D,
            id: 0,
            open: 4000.0,
            high: 4010.0,
            low: 3990.0,
            close: 4005.0,
            vol: 10000.0,
            amount: 40_050_000.0,
            open_interest: None,
            delta: None,
        }
    }

    fn seed_scores(store: &StateStore, scores: &[f64; 5]) {
        let keys = [
            "abnormal:vol_regime",
            "abnormal:vol_anomaly",
            "abnormal:corr_fracture",
            "abnormal:gap_alert",
            "abnormal:trend_accel",
        ];
        for (i, key) in keys.iter().enumerate() {
            store.set(
                (*key).into(),
                StateValue::F64(scores[i]),
                "test_seeder".into(),
            );
        }
    }

    #[test]
    fn test_all_zero_scores() {
        let store = StateStore::new();
        seed_scores(&store, &[0.0; 5]);

        let mut node = ScorecardFusionNode::new("sf".into());
        node.on_bar(&bar(), Freq::D, &store).unwrap();

        let score: Option<f64> = store.get(&OUTPUT_SCORE_KEY.into());
        assert!((score.unwrap() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_all_max_scores() {
        let store = StateStore::new();
        seed_scores(&store, &[100.0; 5]);

        let mut node = ScorecardFusionNode::new("sf".into());
        node.on_bar(&bar(), Freq::D, &store).unwrap();

        let score: Option<f64> = store.get(&OUTPUT_SCORE_KEY.into());
        assert!((score.unwrap() - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_score_in_range() {
        // 混合分数测试：加权融合后应在 [0, 100]
        let store = StateStore::new();
        seed_scores(&store, &[50.0, 30.0, 80.0, 20.0, 60.0]);

        let mut node = ScorecardFusionNode::new("sf".into());
        node.on_bar(&bar(), Freq::D, &store).unwrap();

        let score: Option<f64> = store.get(&OUTPUT_SCORE_KEY.into());
        let s = score.unwrap();
        assert!(s >= 0.0 && s <= 100.0, "score={}", s);
    }

    #[test]
    fn test_expected_weighted_score() {
        // 权重: 0.25, 0.20, 0.15, 0.25, 0.15
        // scores: [40, 50, 60, 70, 80]
        // expected = 40*0.25 + 50*0.20 + 60*0.15 + 70*0.25 + 80*0.15
        //          = 10 + 10 + 9 + 17.5 + 12 = 58.5
        let store = StateStore::new();
        seed_scores(&store, &[40.0, 50.0, 60.0, 70.0, 80.0]);

        let mut node = ScorecardFusionNode::new("sf".into());
        node.on_bar(&bar(), Freq::D, &store).unwrap();

        let score: Option<f64> = store.get(&OUTPUT_SCORE_KEY.into());
        let s = score.unwrap();
        let expected = 40.0 * 0.25 + 50.0 * 0.20 + 60.0 * 0.15 + 70.0 * 0.25 + 80.0 * 0.15;
        assert!(
            (s - expected).abs() < 1e-10,
            "score={}, expected={}",
            s,
            expected
        );
    }

    #[test]
    fn test_alert_level_warn() {
        let store = StateStore::new();
        // score = 75 → warn
        seed_scores(&store, &[75.0, 75.0, 75.0, 75.0, 75.0]);

        let mut node = ScorecardFusionNode::new("sf".into());
        node.on_bar(&bar(), Freq::D, &store).unwrap();

        let level_raw = store.get_json(&OUTPUT_LEVEL_KEY.into());
        assert_eq!(level_raw, Some(serde_json::Value::String("warn".into())));
    }

    #[test]
    fn test_alert_level_emergency() {
        let store = StateStore::new();
        // score = 100 → emergency
        seed_scores(&store, &[100.0; 5]);

        let mut node = ScorecardFusionNode::new("sf".into());
        node.on_bar(&bar(), Freq::D, &store).unwrap();

        let level_raw = store.get_json(&OUTPUT_LEVEL_KEY.into());
        assert_eq!(
            level_raw,
            Some(serde_json::Value::String("emergency".into()))
        );
    }

    #[test]
    fn test_missing_indicator_defaults_to_zero() {
        // 只设置部分指标，缺失的应默认为 0.0
        let store = StateStore::new();
        store.set(
            "abnormal:vol_regime".into(),
            StateValue::F64(50.0),
            "test".into(),
        );
        // 其余 4 个指标不存在 → read_scores 返回 0.0

        let mut node = ScorecardFusionNode::new("sf".into());
        node.on_bar(&bar(), Freq::D, &store).unwrap();

        let score: Option<f64> = store.get(&OUTPUT_SCORE_KEY.into());
        let expected = 50.0 * 0.25; // 12.5
        assert!((score.unwrap() - expected).abs() < 1e-10);
    }

    #[test]
    fn test_custom_weights() {
        // 使用自定义权重
        let weights = AbnormalWeights {
            vol_regime: 0.5,
            vol_anomaly: 0.5,
            corr_fracture: 0.0,
            gap_alert: 0.0,
            trend_accel: 0.0,
        };
        let thresholds = AlertThresholds::default();
        let mut node = ScorecardFusionNode::with_config("sf".into(), weights, thresholds);

        let store = StateStore::new();
        seed_scores(&store, &[60.0, 40.0, 80.0, 20.0, 10.0]);

        node.on_bar(&bar(), Freq::D, &store).unwrap();

        let score: Option<f64> = store.get(&OUTPUT_SCORE_KEY.into());
        let expected = 60.0 * 0.5 + 40.0 * 0.5;
        assert!((score.unwrap() - expected).abs() < 1e-10);
    }

    #[test]
    fn test_on_init_overrides_weights() {
        let mut config = NodeConfig::new();
        config.params.insert(
            "vol_regime_weight".into(),
            serde_json::Value::Number(serde_json::Number::from_f64(0.30).unwrap()),
        );
        config.params.insert(
            "warn_threshold".into(),
            serde_json::Value::Number(serde_json::Number::from_f64(60.0).unwrap()),
        );

        let mut node = ScorecardFusionNode::new("sf".into());
        let store = StateStore::new();
        node.on_init(&config, &store).unwrap();

        assert!((node.weights.vol_regime - 0.30).abs() < 1e-10);
        assert!((node.alert_thresholds.warn - 60.0).abs() < 1e-10);
    }

    #[test]
    fn test_input_output_keys() {
        let node = ScorecardFusionNode::new("sf".into());
        let inputs = node.input_keys();
        assert_eq!(inputs.len(), 5);
        assert!(inputs.contains(&"abnormal:vol_regime".into()));
        assert!(inputs.contains(&"abnormal:vol_anomaly".into()));
        assert!(inputs.contains(&"abnormal:corr_fracture".into()));
        assert!(inputs.contains(&"abnormal:gap_alert".into()));
        assert!(inputs.contains(&"abnormal:trend_accel".into()));

        let outputs = node.output_keys();
        assert_eq!(outputs.len(), 2);
        assert!(outputs.contains(&OUTPUT_SCORE_KEY.into()));
        assert!(outputs.contains(&OUTPUT_LEVEL_KEY.into()));
    }
}
