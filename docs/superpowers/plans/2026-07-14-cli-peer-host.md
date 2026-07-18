# CLI Peer Host Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make BitFun CLI a full Peer Device Mode host so Desktop A can control CLI B via HostInvoke / DeviceEvent.

**Architecture:** CLI decrypts device RPC, special-cases HostInvoke to a Core command registry (no webview), fans out agentic DeviceEvents to attached controllers. Same device_id as Desktop; last AuthConnect wins.

**Tech Stack:** Rust (`bitfun-cli`), `bitfun-core` services, existing account relay WS.

---

### Task 1: Spec + envelope routing — DONE

- [x] Spec under `docs/superpowers/specs/2026-07-14-cli-peer-host.md`
- [x] `account.rs` routes HostInvoke / DeviceEvent; reply target `"rpc"` for HTTP RPC

### Task 2: Control plane + deny tables — DONE

- [x] `peer_mode_ping` / attach / detach
- [x] LOCAL_ONLY + CLI_UNSUPPORTED deny

### Task 3: Bootstrap Core services — DONE

- [x] WorkspaceService, FileSystemService, DialogScheduler, PersistenceManager
- [x] Wire from `initialize_core_services`

### Task 4: Must HostInvoke registry — DONE

- [x] Workspace / FS / session / dialog / system HIGH_PRIORITY commands

### Task 5: DeviceEvent fan-out — DONE

- [x] EventQueue subscriber + sequential encrypt send

### Task 6: Docs + verify — DONE

- [x] `peer-device-mode.md`, CLI `AGENTS.md`
- [x] `cargo check -p bitfun-cli` (clean)
- [x] `cargo test -p bitfun-cli peer_host`
- [x] Enter peer host via chat `/login` (same Auth Server / Username / Password
  + `~/.bitfun` session/hint as Desktop). No separate `peer-host` CLI subcommand.
