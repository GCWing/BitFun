# Evidence Package

## Metadata

- Task: Complete Intent Coding MVP delivery summary
- Date: 2026-05-25
- Risk Level: L1
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-intent-coding-mvp-completion.md`

## Summary

The Intent Coding MVP is implemented as a BitFun-native workflow. It adds a dedicated `IntentCoding` mode, persistent `.agent` workflow artifacts, bounded `.agent` context loading, risk/acceptance/repair/provenance/review rules, Evidence Package structure, a local workflow checker, frontend mode support, usage documentation, and tests around the critical registration/display/context paths.

This completes the MVP goal: Coding Agent work can now be driven by an intent-first loop and delivered with a structured evidence trail, without implementing the full five-phase platform.

## Provenance Chain

- Original request: implement the intent-aligned Coding Agent workflow in the BitFun project based on the referenced article.
- Context inputs: article direction provided by the user, repository AGENTS instructions, BitFun mode registry, prompt system, workspace instruction context, frontend agent mode UI, `.agent` MVP artifacts.
- Intent Record: `.agent/intents/intent-20260525-intent-coding-mvp-completion.md`.
- Acceptance: MVP deliverables summarized, verification summarized, remaining gaps explicit, workflow checker run.
- Execution: created final completion evidence only.
- Verification: final `pnpm run agent:check` passed.
- Repair loop: none in this summary slice.
- Review escalation: not required for L1.
- Evidence Package: `.agent/evidence/evidence-20260525-intent-coding-mvp-completion.md`.

## Files Changed

Primary implementation surfaces:

- `.agent/`
- `scripts/check-agent-workflow.mjs`
- `package.json`
- `src/crates/core/src/agentic/agents/definitions/modes/intent_coding.rs`
- `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`
- `src/crates/core/src/agentic/agents/definitions/modes/mod.rs`
- `src/crates/core/src/agentic/agents/mod.rs`
- `src/crates/core/src/agentic/agents/registry/catalog.rs`
- `src/crates/core/src/agentic/agents/registry/builtin.rs`
- `src/crates/core/src/agentic/agents/registry/tests.rs`
- `src/crates/core/src/service/agent_memory/instruction_context.rs`
- `src/web-ui/src/flow_chat/store/FlowChatStore.ts`
- `src/web-ui/src/app/scenes/agents/utils.ts`
- `src/web-ui/src/app/scenes/agents/utils.test.ts`
- `src/web-ui/src/flow_chat/components/ChatInput.tsx`
- `src/web-ui/src/flow_chat/components/modeDisplay.ts`
- `src/web-ui/src/flow_chat/components/modeDisplay.test.ts`
- `src/web-ui/src/locales/*/flow-chat.json`
- `src/web-ui/src/locales/*/scenes/agents.json`
- `src/web-ui/vite.config.ts`
- `src/web-ui/src/test/monaco-editor.mock.ts`
- `src/web-ui/src/flow_chat/services/flow-chat-manager/EventHandlerModule.test.ts`

## Verification

Passed during the MVP implementation:

- `cargo test -p bitfun-core intent_coding -- --nocapture`
- `cargo test -p bitfun-core workspace_instruction_context -- --nocapture`
- `cargo test -p bitfun-core intent_coding_prompt_embeds_acceptance_and_evidence_workflow -- --nocapture`
- `cargo check --workspace`
- `cargo test --workspace`
- `pnpm --dir src/web-ui run test:run src/app/scenes/agents/utils.test.ts src/flow_chat/components/modeDisplay.test.ts`
- `pnpm --dir src/web-ui run test:run src/flow_chat/services/flow-chat-manager/EventHandlerModule.test.ts`
- `pnpm --dir src/web-ui run test:run`
- `pnpm run lint:web`
- `pnpm run type-check:web`
- `pnpm run agent:check`: passed after this final Evidence Package was written.
- `git diff --check`: passed for tracked changes.
- Untracked text trailing whitespace scan: passed after normalizing `.agent/templates/*` placeholder lines.

## Repair Loop

- Failure classes: test environment/dependency resolution for Monaco in Vitest; workflow artifact pairing during in-progress evidence creation.
- Repair attempts: Monaco/Vitest gap repaired with test-only alias and mock; workflow pairing failures resolved by writing matching Evidence Packages; `.agent/templates/*` placeholder trailing whitespace normalized.
- Final repair status: complete.
- Remaining verification gaps: none for the summary slice.

## Risk Handling

- Final risk level: L1 for this summary slice; overall MVP implementation touched L2 surfaces across Rust core and shared frontend.
- Risk factors: mode registration, prompt behavior, workspace context injection, frontend mode persistence/display, test config.
- Verification matched expected level: yes.
- Skipped verification: none known for the MVP verification surface.
- Review escalation: not required; no L3/L4 auth/payment/data-integrity surface.

## Accepted Checks

- [x] MVP deliverables are summarized.
- [x] Verification outcomes are summarized.
- [x] Remaining gaps are explicit.
- [x] Workflow structure check passes after this Evidence Package is written.

## Accepted Tests

- [x] `pnpm run agent:check`

## Acceptance Coverage Result

- Automated: broad web verification, Rust workspace check, focused Rust tests, focused frontend tests, and workflow checker have passed across prior slices.
- Manual: current git status and diff stat reviewed for scope.
- Coverage gaps: no rendered UI screenshot test of the mode picker; no runtime enforcement that every Intent Coding task writes artifacts.

## Risks

- The MVP is prompt/file/checker based, not a complete runtime-enforced governance platform.
- `agent:check` validates structure, not quality of acceptance criteria or product behavior.
- The Monaco mock is test-only and should not be treated as editor behavior coverage.

## Human Review Focus

- Confirm `IntentCoding` should remain a separate mode instead of replacing Agentic.
- Review prompt wording for strictness and user experience.
- Review `.agent/README.md` and rules for team usability.
- Decide whether P1 should prioritize runtime artifact enforcement, accepted-check status validation, or structured session provenance.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 4 checks, 1 verification command
- verification_passed: true
- rework_needed: false
