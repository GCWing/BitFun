# Evidence Package

## Metadata

- Task: Implement BitFun Intent Coding MVP
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-bitfun-intent-coding-mode.md`

## Summary

Implemented the first BitFun-native Intent Coding MVP as a separate built-in mode. The mode uses a dedicated prompt that requires Intent Record creation, targeted clarification, accepted checks/tests, scoped execution, verification, and an Evidence Package. Workspace `.agent/rules/*.md` files are now loaded into the existing workspace instruction context.

## Files Changed

- `.agent/evidence/evidence-20260525-bitfun-intent-coding-mode.md`
- `.agent/intents/intent-20260525-bitfun-intent-coding-mode.md`
- `.agent/rules/architecture.md`
- `.agent/rules/coding-style.md`
- `.agent/rules/security.md`
- `.agent/templates/evidence-template.md`
- `.agent/templates/intent-template.md`
- `src/crates/core/src/agentic/agents/definitions/modes/intent_coding.rs`
- `src/crates/core/src/agentic/agents/definitions/modes/mod.rs`
- `src/crates/core/src/agentic/agents/mod.rs`
- `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`
- `src/crates/core/src/agentic/agents/registry/builtin.rs`
- `src/crates/core/src/agentic/agents/registry/catalog.rs`
- `src/crates/core/src/service/agent_memory/instruction_context.rs`
- `src/web-ui/src/app/scenes/agents/utils.ts`
- `src/web-ui/src/flow_chat/store/FlowChatStore.ts`
- `src/web-ui/src/locales/en-US/flow-chat.json`
- `src/web-ui/src/locales/en-US/scenes/agents.json`
- `src/web-ui/src/locales/zh-CN/flow-chat.json`
- `src/web-ui/src/locales/zh-CN/scenes/agents.json`
- `src/web-ui/src/locales/zh-TW/flow-chat.json`
- `src/web-ui/src/locales/zh-TW/scenes/agents.json`

## Verification

- `node -e "...JSON.parse(...)"`: passed for updated locale JSON files.
- `cargo test -p bitfun-core intent_coding -- --nocapture`: passed.
- `cargo test -p bitfun-core workspace_instruction_context_includes_agent_rules -- --nocapture`: passed.
- `pnpm run type-check:web`: passed.

## Accepted Checks

- [x] New core mode is registered.
- [x] New prompt file is embedded and referenced.
- [x] `.agent/rules` context builder is covered by a focused test.
- [x] Frontend mode labels include Intent Coding.
- [x] No new dependencies are added.

## Accepted Tests

- `intent_coding_mode_uses_dedicated_prompt_and_planning_tools`
- `workspace_instruction_context_includes_agent_rules`

## Risks

- This is the P0/P1 workflow shell, not the full five-phase platform from the article.
- Intent/Evidence persistence is workspace markdown first; it is not yet deeply bound to `.bitfun/sessions/{session_id}` or provenance events.
- The Disagreement Detector is prompt-guided in this version, not a real multi-candidate behavior comparator.

## Human Review Focus

- Whether the mode id `IntentCoding` is the preferred product-facing identifier.
- Whether the prompt is strict enough about "no edits before Intent Record" without making small coding tasks too heavy.
- Whether `.agent/rules/*.md` should be loaded for all modes through workspace instructions, or only for coding modes.

## Metrics

- intent_created: true
- questions_asked: 2 answered by user direction
- tests_or_checks_created: 5 checks, 2 focused tests
- verification_passed: true
- rework_needed: false

