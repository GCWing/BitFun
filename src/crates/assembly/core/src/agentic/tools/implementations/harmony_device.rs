//! HarmonyOS device listing and resolution — direct port of deveco-code `harmony-device.ts`.
//!
//! Parses `devecocli device list` / `devecocli emulator list` output, matches a
//! query against connected devices or emulators, and auto-starts a stopped
//! emulator when needed. Used by `start_app` and `hdc_log`.

use crate::agentic::tools::framework::ToolUseContext;
use crate::util::errors::{BitFunError, BitFunResult};
use std::time::Duration;
use tokio::join;

use super::devecocli_run::{run_devecocli, DevecocliOptions};

const EMULATOR_BOOT_TIMEOUT: Duration = Duration::from_secs(300);
const DEVICE_WAIT_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone)]
pub(crate) struct HarmonyTarget {
    pub name: String,
    pub status: TargetStatus,
    pub serial: Option<String>,
    pub kind: TargetKind,
    pub device_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetStatus {
    Connected,
    Running,
    Stopped,
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

fn parse_emulator_list(output: &str) -> Vec<HarmonyTarget> {
    let mut targets = Vec::new();
    for line in parse_table_rows(output) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }
        let status_str = parts[1].to_lowercase();
        if status_str != "running" && status_str != "stopped" {
            continue;
        }
        let name = parts[0].to_string();
        let status = if status_str == "running" {
            TargetStatus::Running
        } else {
            TargetStatus::Stopped
        };
        let serial = parts[2];
        let serial = if serial == "-" {
            None
        } else {
            Some(serial.to_string())
        };
        let device_type = parts[4..].join(" ");
        targets.push(HarmonyTarget {
            name,
            status,
            serial,
            kind: TargetKind::Emulator,
            device_type: Some(device_type),
        });
    }
    targets
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
            status: TargetStatus::Connected,
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
    let status = match target.status {
        TargetStatus::Connected => "connected".to_string(),
        TargetStatus::Running => "running (not connected yet)".to_string(),
        TargetStatus::Stopped => "stopped".to_string(),
    };
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
        "{}. {} ({}, {}{}{})",
        index + 1,
        target.name,
        kind,
        status,
        serial,
        device_type
    )
}

fn format_harmony_target_list(targets: &[HarmonyTarget]) -> String {
    if targets.is_empty() {
        return "No HarmonyOS devices or emulators available.\nConnect a USB device with debugging enabled, or create/start an emulator with devecocli emulator."
            .to_string();
    }
    let mut lines = vec!["Available HarmonyOS targets:".to_string()];
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

async fn list_harmony_targets(
    context: &ToolUseContext,
) -> BitFunResult<(Vec<HarmonyTarget>, Vec<HarmonyTarget>, Vec<HarmonyTarget>)> {
    let (device_out, emulator_out) = join!(
        run_devecocli(&["device", "list"], context, DevecocliOptions::default()),
        run_devecocli(&["emulator", "list"], context, DevecocliOptions::default()),
    );
    let device_out = device_out?;
    let emulator_out = emulator_out?;

    if device_out.exit_code != 0 {
        return Err(BitFunError::tool(format!(
            "device list failed (exit {}):\n{}",
            device_out.exit_code,
            if device_out.stderr.is_empty() {
                &device_out.stdout
            } else {
                &device_out.stderr
            }
        )));
    }
    if emulator_out.exit_code != 0 {
        return Err(BitFunError::tool(format!(
            "emulator list failed (exit {}):\n{}",
            emulator_out.exit_code,
            if emulator_out.stderr.is_empty() {
                &emulator_out.stdout
            } else {
                &emulator_out.stderr
            }
        )));
    }

    let device_raw = format!("{}\n{}", device_out.stdout, device_out.stderr)
        .trim()
        .to_string();
    let emulator_raw = format!("{}\n{}", emulator_out.stdout, emulator_out.stderr)
        .trim()
        .to_string();

    let connected = parse_connected_devices(&device_raw);
    let emulators = parse_emulator_list(&emulator_raw);
    let stopped: Vec<HarmonyTarget> = emulators
        .iter()
        .filter(|t| t.status == TargetStatus::Stopped)
        .cloned()
        .collect();
    let running_not_connected: Vec<HarmonyTarget> = emulators
        .iter()
        .filter(|t| {
            t.status == TargetStatus::Running
                && !connected.iter().any(|c| match_target(&t.name, c))
        })
        .cloned()
        .collect();

    let mut available = connected.clone();
    available.extend(running_not_connected);
    available.extend(stopped);

    Ok((connected, emulators, available))
}

async fn wait_for_connected_device(
    name_or_serial: &str,
    context: &ToolUseContext,
    accept_any_emulator: bool,
) -> BitFunResult<Option<String>> {
    let deadline = std::time::Instant::now() + DEVICE_WAIT_TIMEOUT;
    loop {
        if std::time::Instant::now() > deadline {
            return Err(BitFunError::tool(format!(
                "Timed out waiting for device \"{}\" to connect ({}s).",
                name_or_serial,
                DEVICE_WAIT_TIMEOUT.as_secs()
            )));
        }
        let out = run_devecocli(
            &["device", "list"],
            context,
            DevecocliOptions::default(),
        )
        .await?;
        if out.exit_code != 0 {
            return Err(BitFunError::tool(format!(
                "device list failed while waiting for \"{}\":\n{}",
                name_or_serial,
                if out.stderr.is_empty() {
                    &out.stdout
                } else {
                    &out.stderr
                }
            )));
        }
        let raw = format!("{}\n{}", out.stdout, out.stderr);
        let connected = parse_connected_devices(&raw);

        if accept_any_emulator {
            if let Some(emu) = connected
                .iter()
                .find(|t| t.kind == TargetKind::Emulator)
            {
                if let Some(serial) = &emu.serial {
                    return Ok(Some(serial.clone()));
                }
            }
        }
        if let Some(match_) = connected
            .iter()
            .find(|t| match_target(name_or_serial, t))
        {
            if let Some(serial) = &match_.serial {
                return Ok(Some(serial.clone()));
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

pub(crate) enum DeviceResolution {
    List {
        output: String,
        device_count: usize,
        emulator_count: usize,
    },
    Ready {
        device: String,
        started_emulator: bool,
        preface: String,
    },
}

pub(crate) async fn resolve_start_app_device(
    hvd: Option<&str>,
    context: &ToolUseContext,
) -> BitFunResult<DeviceResolution> {
    let (connected, emulators, available) = list_harmony_targets(context).await?;

    let hvd = match hvd {
        Some(h) => h.trim(),
        None => "",
    };
    if hvd.is_empty() {
        return Ok(DeviceResolution::List {
            output: format_harmony_target_list(&available),
            device_count: connected.len(),
            emulator_count: emulators.len(),
        });
    }

    if let Some(connected_dev) = connected.iter().find(|t| match_target(hvd, t)) {
        let device = connected_dev
            .serial
            .clone()
            .unwrap_or_else(|| hvd.to_string());
        return Ok(DeviceResolution::Ready {
            device,
            started_emulator: false,
            preface: String::new(),
        });
    }

    let emulator = emulators
        .iter()
        .find(|t| match_target(hvd, t))
        .or_else(|| {
            emulators
                .iter()
                .find(|t| normalize_name(&t.name) == normalize_name(hvd))
        });

    let emulator = match emulator {
        Some(e) => e,
        None => {
            return Err(BitFunError::tool(format!(
                "Device or emulator \"{}\" not found.\n\n{}",
                hvd,
                format_harmony_target_list(&available)
            )));
        }
    };

    if emulator.status == TargetStatus::Stopped {
        let start_out = run_devecocli(
            &["emulator", "start", &emulator.name],
            context,
            DevecocliOptions {
                timeout: EMULATOR_BOOT_TIMEOUT,
                ..Default::default()
            },
        )
        .await?;
        let combined = format!("{}\n{}", start_out.stdout, start_out.stderr)
            .trim()
            .to_string();
        if start_out.exit_code != 0 {
            return Err(BitFunError::tool(format!(
                "Failed to start emulator \"{}\" (exit {}):\n{}",
                emulator.name, start_out.exit_code, combined
            )));
        }
        let serial = wait_for_connected_device(&emulator.name, context, true).await?;
        let device = serial.unwrap_or_else(|| emulator.name.clone());
        let preface = format!("Started emulator \"{}\".\n{}", emulator.name, combined);
        return Ok(DeviceResolution::Ready {
            device,
            started_emulator: true,
            preface,
        });
    }

    let serial = wait_for_connected_device(&emulator.name, context, true).await?;
    let has_serial = serial.is_some();
    let device = serial.unwrap_or_else(|| hvd.to_string());
    let preface = if has_serial {
        String::new()
    } else {
        format!(
            "Emulator \"{}\" is running; waiting for hdc connection…",
            emulator.name
        )
    };
    Ok(DeviceResolution::Ready {
        device,
        started_emulator: false,
        preface,
    })
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
    fn parse_emulator_list_parses_running_and_stopped() {
        let output = "emu1 running 12345 emulator Phone\nemu2 stopped - emulator Tablet";
        let targets = parse_emulator_list(output);
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].name, "emu1");
        assert_eq!(targets[0].status, TargetStatus::Running);
        assert_eq!(targets[1].name, "emu2");
        assert_eq!(targets[1].status, TargetStatus::Stopped);
        assert!(targets[1].serial.is_none());
    }

    #[test]
    fn match_target_matches_by_name_or_serial() {
        let target = HarmonyTarget {
            name: "MyDevice".to_string(),
            status: TargetStatus::Connected,
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
