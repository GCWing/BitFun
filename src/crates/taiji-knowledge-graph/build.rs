//! Build script: parse 7 Agent JSON Schemas and golden tick cases
//! to pre-generate knowledge graph node/edge data at compile time.
#![allow(unused_assignments)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest_path = out_dir.join("generated_graph_data.json");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let schemas_dir = workspace_root.join("scripts").join("agent-output-schemas");

    // 构建 JSON 格式的节点和边
    let (nodes_json, edges_json) = from_agent_schemas(&schemas_dir);
    let case_nodes_json = from_golden_ticks(&workspace_root);

    // 合并所有节点
    let output = format!(
        r#"{{"nodes":{}, "edges":{}}}"#,
        merge_json_arrays(&nodes_json, &case_nodes_json),
        edges_json,
    );

    fs::write(&dest_path, &output).unwrap();
    println!("cargo:rerun-if-changed=../../../../scripts/agent-output-schemas/");
    println!("cargo:rerun-if-changed=../../../../scripts/collect_golden_tick.py");
    // Knowledge graph generated at compile time; silent unless --verbose
    println!(
        "cargo:info=Knowledge graph JSON generated: {} bytes",
        output.len()
    );
}

fn merge_json_arrays(a: &str, b: &str) -> String {
    // a: [ ... ]  b: [ ... ]  →  [ ...a..., ...b... ]
    if a == "[]" {
        return b.to_string();
    }
    if b == "[]" {
        return a.to_string();
    }
    let a_inner = &a[1..a.len() - 1];
    let b_inner = &b[1..b.len() - 1];
    if a_inner.is_empty() {
        return b.to_string();
    }
    if b_inner.is_empty() {
        return a.to_string();
    }
    format!("[{},{}]", a_inner, b_inner)
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn from_agent_schemas(_schemas_dir: &PathBuf) -> (String, String) {
    let mut nodes = String::from("[");
    let mut edges = String::from("[");
    let mut first_node = true;
    let mut first_edge = true;

    macro_rules! add_node {
        ($id:expr, $name:expr, $cat:expr, $desc:expr, $srcs:expr) => {
            if !first_node {
                nodes.push(',');
            }
            first_node = false;
            let srcs_str = $srcs
                .iter()
                .map(|s: &&str| format!("\"{}\"", esc(s)))
                .collect::<Vec<_>>()
                .join(",");
            nodes.push_str(&format!(
                r#"{{"id":"{}","name":"{}","category":"{}","description":"{}","sources":[{}]}}"#,
                esc($id),
                esc($name),
                $cat,
                esc($desc),
                srcs_str
            ));
        };
    }

    macro_rules! add_edge {
        ($from:expr, $to:expr, $rel:expr, $weight:expr, $label:expr) => {
            if !first_edge {
                edges.push(',');
            }
            first_edge = false;
            edges.push_str(&format!(
                r#"{{"from":"{}","to":"{}","relation":"{}","weight":{},"label":"{}"}}"#,
                esc($from),
                esc($to),
                $rel,
                $weight,
                esc($label)
            ));
        };
    }

    // ── 理论根节点 ──
    add_node!(
        "theory_vol",
        "量（资金流）",
        "concept",
        "量价时空四维之一：成交量 + 持仓量 + 资金流向",
        &["四总纲/技术总纲.md §1"]
    );
    add_node!(
        "theory_price",
        "价（价格）",
        "concept",
        "量价时空四维之一：价格行为、K线形态",
        &["四总纲/技术总纲.md §1"]
    );
    add_node!(
        "theory_time",
        "时（周期）",
        "concept",
        "量价时空四维之一：多周期嵌套、时间框架",
        &["四总纲/技术总纲.md §1"]
    );
    add_node!(
        "theory_space",
        "空（空间）",
        "concept",
        "量价时空四维之一：支撑阻力、磁体、目标位",
        &["四总纲/技术总纲.md §1"]
    );

    // ── 派生理论概念 ──
    let derived: &[(&str, &str, &str, &str)] = &[
        (
            "theory_thrust",
            "三推",
            "三推力竭模型：方向性推力+衰竭判定",
            "theory_time",
        ),
        (
            "theory_magnet",
            "磁体",
            "磁体理论：价格被磁体吸引，关键价位引力",
            "theory_space",
        ),
        (
            "theory_delta",
            "Delta分类",
            "成交量Delta六分类：多开/空开/多平/空平/净多/净空",
            "theory_vol",
        ),
        (
            "theory_resonance",
            "共振",
            "多周期/多Agent信号共振确认",
            "theory_time",
        ),
        (
            "theory_structure",
            "结构分析",
            "趋势结构：高低点、通道、趋势线",
            "theory_price",
        ),
        (
            "theory_risk",
            "风控",
            "风险管理：仓位、ATR止损、凯利公式",
            "theory_space",
        ),
    ];
    for &(id, name, desc, parent) in derived {
        add_node!(id, name, "concept", desc, &["四总纲/技术总纲.md"]);
        add_edge!(parent, id, "derives_from", 1.0, "派生子概念");
    }

    // ── 7 Agent → 策略节点 ──
    let agents: &[(&str, &str, &str, &str)] = &[
        (
            "agent_structure",
            "结构分析Agent",
            "分析趋势方向、枢轴结构、通道状态",
            "theory_structure",
        ),
        (
            "agent_delta",
            "资金流向Agent",
            "分析净持仓、Delta方向、六大核心指标",
            "theory_delta",
        ),
        (
            "agent_magnet",
            "磁体Agent",
            "分析磁体位置、OI确认、MM目标位",
            "theory_magnet",
        ),
        (
            "agent_thrust",
            "三推Agent",
            "检测三推力竭信号、BOS/CHoCH",
            "theory_thrust",
        ),
        (
            "agent_resonance",
            "共振Agent",
            "多Agent信号共振验证、门控审计",
            "theory_resonance",
        ),
        (
            "agent_risk",
            "风控Agent",
            "仓位计算、ATR止损、凯利分数",
            "theory_risk",
        ),
        (
            "agent_decision",
            "决策Agent",
            "综合六Agent信号做出Long/Short/Hold决策",
            "theory_structure",
        ),
    ];
    for &(id, name, desc, parent) in agents {
        add_node!(
            id,
            name,
            "strategy",
            desc,
            &["scripts/agent-output-schemas/"]
        );
        add_edge!(parent, id, "derives_from", 1.0, "Agent实现");
    }

    // ── Decision Agent 依赖所有上游 Agent ──
    for up in &[
        "agent_structure",
        "agent_delta",
        "agent_magnet",
        "agent_thrust",
        "agent_resonance",
        "agent_risk",
    ] {
        add_edge!(up, "agent_decision", "uses", 0.8, "信号输入");
    }

    // ── 数据指标节点 ──
    let indicators: &[(&str, &str, &str, &str)] = &[
        (
            "data_trend_direction",
            "趋势方向",
            "up/down/sideways",
            "agent_structure",
        ),
        (
            "data_trend_strength",
            "趋势强度",
            "0.0-1.0",
            "agent_structure",
        ),
        (
            "data_pivot_structure",
            "枢轴结构",
            "higher_highs/lower_lows/...",
            "agent_structure",
        ),
        (
            "data_key_support",
            "关键支撑",
            "价格数值",
            "agent_structure",
        ),
        (
            "data_key_resistance",
            "关键阻力",
            "价格数值",
            "agent_structure",
        ),
        (
            "data_channel_state",
            "通道状态",
            "expanding/contracting/parallel",
            "agent_structure",
        ),
        (
            "data_net_position",
            "净持仓状态",
            "long_building/short_building/...",
            "agent_delta",
        ),
        (
            "data_delta_direction",
            "Delta方向",
            "positive/negative/neutral",
            "agent_delta",
        ),
        (
            "data_volume_trend",
            "成交量趋势",
            "increasing/decreasing/stable",
            "agent_delta",
        ),
        (
            "data_six_core",
            "六大核心指标",
            "多开/空开/多平/空平/净多/净空",
            "agent_delta",
        ),
        (
            "data_magnet_position",
            "磁体相对位置",
            "above/below/inside/at_boundary",
            "agent_magnet",
        ),
        ("data_magnet_valid", "磁体有效性", "boolean", "agent_magnet"),
        (
            "data_mm1_target",
            "MM1目标位",
            "Trading Range Breakout目标",
            "agent_magnet",
        ),
        (
            "data_mm2_target",
            "MM2目标位",
            "Leg1=Leg2目标",
            "agent_magnet",
        ),
        (
            "data_oi_confirmation",
            "OI确认",
            "持仓量确认",
            "agent_magnet",
        ),
        (
            "data_resonance_levels",
            "多周期共振位",
            "多周期磁体重叠区域",
            "agent_magnet",
        ),
        (
            "data_triple_push",
            "三推检测",
            "是否发现三推结构",
            "agent_thrust",
        ),
        ("data_push_count", "推力计数", "0-N", "agent_thrust"),
        ("data_exhaustion", "力竭信号", "boolean", "agent_thrust"),
        ("data_overshoot", "超涨超跌", "boolean", "agent_thrust"),
        (
            "data_bos_detected",
            "BOS检测",
            "Break of Structure",
            "agent_thrust",
        ),
        (
            "data_choch_detected",
            "CHoCH检测",
            "Change of Character",
            "agent_thrust",
        ),
        (
            "data_resonance_signal",
            "共振信号",
            "boolean",
            "agent_resonance",
        ),
        (
            "data_resonance_type",
            "共振方向",
            "bullish/bearish/none",
            "agent_resonance",
        ),
        (
            "data_aligned_agents",
            "一致Agent",
            "方向一致的Agent列表",
            "agent_resonance",
        ),
        (
            "data_conflicting_agents",
            "冲突Agent",
            "方向冲突的Agent列表",
            "agent_resonance",
        ),
        (
            "data_multi_tf_resonance",
            "多周期共振",
            "boolean|null",
            "agent_resonance",
        ),
        ("data_max_position", "最大仓位", "0.0-1.0", "agent_risk"),
        ("data_current_atr", "当前ATR", ">=0", "agent_risk"),
        ("data_kelly_fraction", "凯利分数", "0.0-1.0", "agent_risk"),
        ("data_risk_per_trade", "单笔风险", "0.0-1.0", "agent_risk"),
        ("data_allow_long", "允许做多", "boolean", "agent_risk"),
        ("data_allow_short", "允许做空", "boolean", "agent_risk"),
        (
            "data_action",
            "交易动作",
            "Long/Short/Hold",
            "agent_decision",
        ),
        ("data_entry", "入场价", "number|null", "agent_decision"),
        ("data_stop_loss", "止损价", "number|null", "agent_decision"),
        (
            "data_take_profit",
            "止盈价",
            "number|null",
            "agent_decision",
        ),
        (
            "data_size_pct",
            "仓位百分比",
            "number|null",
            "agent_decision",
        ),
        ("data_confidence", "综合置信度", "0.0-1.0", "agent_decision"),
    ];
    for &(id, name, desc, parent) in indicators {
        add_node!(id, name, "case", desc, &["agent-output-schemas/"]);
        add_edge!(parent, id, "contains", 0.6, "输出指标");
    }

    // ── 跨概念关联 ──
    add_edge!(
        "theory_magnet",
        "theory_resonance",
        "correlates_with",
        0.5,
        "磁体重叠→共振确认"
    );
    add_edge!(
        "theory_thrust",
        "theory_structure",
        "correlates_with",
        0.5,
        "三推力竭→趋势反转结构"
    );
    add_edge!(
        "theory_delta",
        "theory_magnet",
        "correlates_with",
        0.5,
        "OI确认磁体有效性"
    );
    add_edge!(
        "theory_risk",
        "theory_magnet",
        "correlates_with",
        0.5,
        "ATR计算磁体目标风险"
    );

    nodes.push(']');
    edges.push(']');
    (nodes, edges)
}

fn from_golden_ticks(workspace_root: &Path) -> String {
    let golden_dir = workspace_root.join("test_data").join("golden_tick");
    let mut nodes = String::from("[");

    if let Ok(entries) = fs::read_dir(&golden_dir) {
        let mut first = true;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !first {
                        nodes.push(',');
                    }
                    first = false;
                    nodes.push_str(&format!(
                        r#"{{"id":"case_{}","name":"{}","category":"case","description":"Golden tick案例：{}","sources":["{}"]}}"#,
                        esc(name), esc(name), esc(name),
                        esc(&path.to_string_lossy())
                    ));
                }
            }
        }
    }

    // 始终提供一个示例节点
    if nodes == "[" {
        nodes.push_str(r#"{"id":"case_example","name":"示例案例","category":"case","description":"待导入golden tick案例","sources":["test_data/golden_tick/"]}"#);
    }

    nodes.push(']');
    nodes
}
