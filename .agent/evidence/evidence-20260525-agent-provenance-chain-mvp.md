# Evidence Package

## Metadata

- Task: Add MVP provenance chain fields
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-agent-provenance-chain-mvp.md`

## Summary

Added lightweight Provenance Chain guidance to Intent Coding. Intent Records now include provenance anchors, Evidence Packages include a compact provenance chain, and the Intent Coding prompt instructs Agents to preserve key request-to-delivery links without pasting full logs or sensitive data.

## Provenance Chain

- Original request: User asked to continue implementing the intent-aligned Coding Agent workflow.
- Context inputs: `.agent/rules/provenance-chain.md`, `.agent/templates/intent-template.md`, `.agent/templates/evidence-template.md`, `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`.
- Intent Record: `.agent/intents/intent-20260525-agent-provenance-chain-mvp.md`.
- Acceptance: Add provenance rule, template fields, prompt instruction, and focused checks.
- Execution: Added provenance rule and updated templates plus Intent Coding prompt.
- Verification: Text check with `rg`; `cargo test -p bitfun-core intent_coding -- --nocapture`.
- Repair loop: No failures; repair status `not_needed`.
- Review escalation: Not required for L1.
- Evidence Package: `.agent/evidence/evidence-20260525-agent-provenance-chain-mvp.md`.

## Files Changed

- `.agent/evidence/evidence-20260525-agent-provenance-chain-mvp.md`
- `.agent/intents/intent-20260525-agent-provenance-chain-mvp.md`
- `.agent/rules/provenance-chain.md`
- `.agent/templates/evidence-template.md`
- `.agent/templates/intent-template.md`
- `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`

## Verification

- `rg -n "Provenance Chain|Provenance Anchors|provenance chain|provenance anchors|Context inputs|Evidence Package" .agent/rules/provenance-chain.md .agent/templates src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`: passed.
- `cargo test -p bitfun-core intent_coding -- --nocapture`: passed.

## Repair Loop

- Failure classes: none observed.
- Repair attempts: 0.
- Final repair status: not_needed.
- Remaining verification gaps: full workspace tests were not run for this prompt/template/rule-only slice.

## Risk Handling

- Final risk level: L1
- Risk factors: Prompt/template/rule guidance change.
- Verification matched expected level: yes.
- Skipped verification: full workspace tests were not run because no runtime event store, session schema, or UI behavior changed.
- Review escalation: Not required for L1.

## Accepted Checks

- [x] Provenance rule exists.
- [x] Intent template includes `Provenance Anchors`.
- [x] Evidence template includes `Provenance Chain`.
- [x] Intent Coding prompt references provenance.
- [x] No runtime event store is added.

## Accepted Tests

- Text checks with `rg`.
- `intent_coding_mode_uses_dedicated_prompt_and_planning_tools`

## Risks

- Provenance is still manually summarized in markdown.
- Tool calls and runtime events are not yet automatically projected into the chain.
- Evidence quality depends on Agent compliance until session-level provenance exists.

## Human Review Focus

- Whether the minimum chain has the right amount of detail.
- Whether provenance should later be stored in `.bitfun/sessions/{session_id}` as structured data.
- Whether sensitive-data filtering should be runtime-enforced before automatic provenance export.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 5 checks, 2 verification commands
- verification_passed: true
- rework_needed: false

