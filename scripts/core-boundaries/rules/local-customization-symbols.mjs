// Local fork customization symbol manifest (R-AD-04 boundary patch).
//
// The 34 kept symbols below come from the type-contract v2.0 定制符号契约表
// (§三.1 移动保持) and were verified 1:1 by the R-AD-01 40-symbol audit
// (报告-阶段2-RAD01-40符号核对-20260812.md). Each entry pins the exact
// top-level declaration site (`path` + `anchor` regex). The boundary checker
// asserts every anchor still matches the target file, so deleting any one of
// these local symbols fails `node scripts/check-core-boundaries.mjs` and the
// CI gate.
//
// Adding a new local customization symbol REQUIRES a new entry here — a
// registered manifest is the only way for new symbols to pass the boundary
// check without a review (防漂移).

// local_customizations.rs top-level `pub` symbols that must survive upstream
// syncs (GroupChat 主人标识 + AgentType + steering helpers; 常开 + agent-api).
// R-AD-GC (2026-08-14): GroupChat 旧 IM 模型移除，仅保留主人标识（司令官裁决
// GROUP_MASTER_ACTOR / GroupChatActor）；GroupChatRoom 等 22 个契约符号已删。
export const localCustomizationSymbols = [
  { path: 'src/crates/contracts/runtime-ports/src/local_customizations.rs', anchor: /^pub enum AgentType\b/m, note: 'R-AD-01 #25' },
  { path: 'src/crates/contracts/runtime-ports/src/local_customizations.rs', anchor: /^pub const GROUP_MASTER_ACTOR\b/m, note: 'R-AD-01 #1' },
  { path: 'src/crates/contracts/runtime-ports/src/local_customizations.rs', anchor: /^pub enum GroupChatActor\b/m, note: 'R-AD-01 #3' },
  { path: 'src/crates/contracts/runtime-ports/src/local_customizations.rs', anchor: /^pub fn round_injection_dedup_key\b/m, note: 'R-AD-01 #29' },
  { path: 'src/crates/contracts/runtime-ports/src/local_customizations.rs', anchor: /^pub fn round_injection_push_reminder\b/m, note: 'R-AD-01 #30' },
  { path: 'src/crates/contracts/runtime-ports/src/agent_api.rs', anchor: /pub include_hidden: bool\b/m, note: 'R-AD-01 #31' },
  { path: 'src/crates/contracts/runtime-ports/src/agent_api.rs', anchor: /pub parent_session_id: Option<String>/m, note: 'R-AD-01 #32' },
  { path: 'src/crates/contracts/runtime-ports/src/agent_api.rs', anchor: /pub status: Option<String>/m, note: 'R-AD-01 #32' },
  { path: 'src/crates/contracts/runtime-ports/src/agent_api.rs', anchor: /pub is_daemon: bool\b/m, note: 'R-AD-01 #32' },
  { path: 'src/crates/contracts/runtime-ports/src/agent_api.rs', anchor: /pub prepended_reminders: Vec<AgentDialogPrependedReminder>/m, note: 'R-AD-01 #33' },
  { path: 'src/crates/contracts/runtime-ports/src/agent_api.rs', anchor: /pub reference_files: Vec<String>/m, note: 'R-AD-01 #34' },
  { path: 'src/crates/contracts/runtime-ports/src/agent_api.rs', anchor: /pub reference_files: Option<Vec<String>>/m, note: 'R-AD-01 #34' },
  { path: 'src/crates/contracts/runtime-ports/src/agent_api.rs', anchor: /pub include_hidden_subagents: bool\b/m, note: 'R-AD-01 #36' },
  { path: 'src/crates/contracts/runtime-ports/src/agent_api.rs', anchor: /pub fn dedup_key\(&self\) -> Option<&str>/m, note: 'R-AD-01 #37' },
  { path: 'src/crates/contracts/runtime-ports/src/lib.rs', anchor: /^pub const MAX_FISSION_DEPTH: u8 = 10;$/m, note: 'R-AD-01 #35' },
];

// Symbols that are deliberately allowed to leave the manifest after R-AD-03
// removes the Warden contract (kept here so the removal commit is explicit).
export const retiredLocalCustomizationSymbols = [
  'WardenAuditJudgementRequest',
  'WardenAuditJudgementResponse',
  'WardenModelJudgementPort',
  'POKE_PENALTY_KIND',
  'SELF_BOOT_CHECK_KIND',
  'RBAC_ROLE_REMINDER_KIND',
];

// New local customization symbols must be registered here before they can be
// added to localCustomizationSymbols — a review checkpoint for 防漂移.
export const registrationCheckpoint =
  'local-customization-symbols registration checkpoint (R-AD-04)';

// Boundary check for the registered manifest. Lives next to the data so the
// checker stays a thin orchestrator (kept under the 1200-line module budget).
export function checkLocalCustomizationSymbols(symbols, { failures, repoPathToFsPath, existsSync, readText }) {
  const seen = new Map();
  for (const entry of symbols) {
    const key = `${entry.path}|${entry.anchor.source}`;
    if (seen.has(key)) {
      failures.push({
        path: repoPathToFsPath(entry.path),
        line: 1,
        message: `duplicate local customization symbol anchor: ${entry.note ?? ''}`,
      });
      continue;
    }
    seen.set(key, entry);
    const path = repoPathToFsPath(entry.path);
    if (!existsSync(path)) {
      failures.push({
        path,
        line: 1,
        message: `missing local customization symbol owner file: ${entry.path}`,
      });
      continue;
    }
    if (!entry.anchor.test(readText(path))) {
      failures.push({
        path,
        line: 1,
        message: `missing local customization symbol (${entry.note ?? entry.anchor.source}): ${entry.anchor.source}`,
      });
    }
  }
}
