//! HarmonyOS device listing and resolution — simplified port of deveco-code
//! `harmony-device.ts` with emulator support removed.
//!
//! Parses `devecocli device list` output, matches a query against connected
//! devices. Used by `start_app` and `hdc_log`.

use crate::agentic::tools::framework::ToolUseContext;
use crate::util::errors::{BitFunError, BitFunResult};

use super::devecocli_run::{run_devecocli, DevecocliOptions};

#[derive(Debug, Clone)]
pub(crate) struct HarmonyTarget {
    pub name: String,
    pub serial: Option<String>,
    pub kind: TargetKind,
    pub device_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetKind {
    Device,
    Emulator,
}

fn normalize_name(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_table_rows(output: &str) -> Vec<String> {
    let headers = [
        "name",
        "serial",
        "listing",
        "querying",
        "- listing",
        "- querying",
    ];
    output
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| {
            if line.is_empty() || !line.chars().any(|c| c.is_alphanumeric()) {
                return false;
            }
            let lower = line.to_lowercase();
            !headers.iter().any(|h| lower.starts_with(h))
        })
        .collect()
}

fn parse_connected_devices(output: &str) -> Vec<HarmonyTarget> {
    if output.to_lowercase().contains("no active devices") {
        return Vec::new();
    }

    let mut targets = Vec::new();
    for line in parse_table_rows(output) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let name = parts[0].to_string();
        let serial = parts[1].to_string();
        if serial == "Serial" || serial == "-" {
            continue;
        }
        let kind = if parts.len() > 2 && parts[2].eq_ignore_ascii_case("emulator") {
            TargetKind::Emulator
        } else {
            TargetKind::Device
        };
        let device_type = if parts.len() > 3 && parts[3] != "-" {
            Some(parts[3].to_string())
        } else {
            None
        };
        targets.push(HarmonyTarget {
            name,
            serial: Some(serial),
            kind,
            device_type,
        });
    }
    targets
}

fn match_target(query: &str, target: &HarmonyTarget) -> bool {
    let normalized = normalize_name(query);
    let name = normalize_name(&target.name);
    let serial = target.serial.as_ref().map(|s| normalize_name(s));
    let candidates: [&str; 2] = [&name, serial.as_deref().unwrap_or("")];
    candidates.iter().any(|c| {
        !c.is_empty() && (*c == normalized || c.contains(&normalized) || normalized.contains(c))
    })
}

fn format_target_line(index: usize, target: &HarmonyTarget) -> String {
    let serial = target
        .serial
        .as_ref()
        .map(|s| format!(", serial: {}", s))
        .unwrap_or_default();
    let device_type = target
        .device_type
        .as_ref()
        .map(|t| format!(", type: {}", t))
        .unwrap_or_default();
    let kind = if target.kind == TargetKind::Emulator {
        "emulator"
    } else {
        "device"
    };
    format!(
        "{}. {} ({}, connected{}{})",
        index + 1,
        target.name,
        kind,
        serial,
        device_type
    )
}

fn format_harmony_target_list(targets: &[HarmonyTarget]) -> String {
    if targets.is_empty() {
        return "No HarmonyOS devices connected.\nConnect a USB device with debugging enabled."
            .to_string();
    }
    let mut lines = vec!["Connected HarmonyOS targets:".to_string()];
    for (i, t) in targets.iter().enumerate() {
        lines.push(format_target_line(i, t));
    }
    lines.join("\n")
}

pub(crate) fn format_connected_device_list(output: &str) -> (String, usize) {
    let devices = parse_connected_devices(output);
    if devices.is_empty() {
        return ("No connected devices detected.".to_string(), 0);
    }
    let mut lines = vec!["Connected devices:".to_string()];
    for (i, d) in devices.iter().enumerate() {
        let serial = d.serial.as_deref().unwrap_or("?");
        lines.push(format!("{}. {} ({})", i + 1, d.name, serial));
    }
    (lines.join("\n"), devices.len())
}

/// List connected HarmonyOS devices via `devecocli device list`.
async fn list_connected_devices(context: &ToolUseContext) -> BitFunResult<Vec<HarmonyTarget>> {
    let out = run_devecocli(&["device", "list"], context, DevecocliOptions::default()).await?;
    if out.exit_code != 0 {
        return Err(BitFunError::tool(format!(
            "device list failed (exit {}):\n{}",
            out.exit_code,
            if out.stderr.is_empty() {
                &out.stdout
            } else {
                &out.stderr
            }
        )));
    }
    let raw = format!("{}\n{}", out.stdout, out.stderr);
    Ok(parse_connected_devices(&raw))
}

pub(crate) enum DeviceResolution {
    List {
        output: String,
        device_count: usize,
    },
    Ready {
        device: String,
    },
}

/// Resolve the target device for `start_app`. When `hvd` is omitted, lists
/// connected devices; when `hvd` matches a connected device, returns its serial.
pub(crate) async fn resolve_start_app_device(
    hvd: Option<&str>,
    context: &ToolUseContext,
) -> BitFunResult<DeviceResolution> {
    let devices = list_connected_devices(context).await?;

    let hvd = match hvd {
        Some(h) => h.trim(),
        None => "",
    };
    if hvd.is_empty() {
        return Ok(DeviceResolution::List {
            output: format_harmony_target_list(&devices),
            device_count: devices.len(),
        });
    }

    if let Some(dev) = devices.iter().find(|t| match_target(hvd, t)) {
        let device = dev.serial.clone().unwrap_or_else(|| hvd.to_string());
        return Ok(DeviceResolution::Ready { device });
    }

    Err(BitFunError::tool(format!(
        "Device \"{}\" not found.\n\n{}",
        hvd,
        format_harmony_target_list(&devices)
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_connected_devices_handles_empty() {
        assert!(parse_connected_devices("No active devices").is_empty());
        assert!(parse_connected_devices("").is_empty());
    }

    #[test]
    fn match_target_matches_by_name_or_serial() {
        let target = HarmonyTarget {
            name: "MyDevice".to_string(),
            serial: Some("12345".to_string()),
            kind: TargetKind::Device,
            device_type: None,
        };
        assert!(match_target("mydevice", &target));
        assert!(match_target("12345", &target));
        assert!(match_target("myd", &target));
        assert!(!match_target("nonexistent", &target));
    }

    #[test]
    fn format_connected_device_list_handles_no_devices() {
        let (output, count) = format_connected_device_list("No active devices");
        assert_eq!(count, 0);
        assert!(output.contains("No connected devices"));
    }
}
