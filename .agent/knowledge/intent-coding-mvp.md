# Knowledge Note

## Topic

Intent Coding MVP architecture in BitFun.

## Applies To

- Intent Coding mode.
- `.agent/` workspace workflow files.
- Simplified Context Compiler behavior.
- Evidence Package and Intent Record conventions.

## Stable Facts

- Intent Coding is implemented as a separate built-in mode with id `IntentCoding`.
- The mode implementation lives in `src/crates/core/src/agentic/agents/definitions/modes/intent_coding.rs`.
- The mode prompt lives in `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`.
- Built-in mode registration flows through `src/crates/core/src/agentic/agents/registry/catalog.rs`.
- Frontend persistence allows `IntentCoding` in `src/web-ui/src/flow_chat/store/FlowChatStore.ts`.
- Frontend display/capability mapping lives in `src/web-ui/src/app/scenes/agents/utils.ts`.
- Workspace `.agent` context loading is implemented in `src/crates/core/src/service/agent_memory/instruction_context.rs`.

## Constraints

- Intent Coding should not replace the default Agentic mode.
- Product logic stays platform-agnostic; desktop-specific behavior should not be introduced for this workflow.
- The MVP is intentionally file/prompt based before adding runtime enforcement.
- `.agent/rules`, `.agent/knowledge`, and `.agent/changes` are loaded as bounded shallow Markdown context.
- `.agent` bucket `README.md` files are human guidance and are skipped during automatic context injection.

## Common Traps

- Do not add a second parallel agent registry path for Intent Coding.
- Do not silently broaden Intent Coding into auto-merge, policy engine, or Deep Review auto-trigger behavior.
- Do not put large logs or secrets in Intent/Evidence files.
- Do not rely on `.agent/knowledge/README.md` or `.agent/changes/README.md` as Agent context; use named Markdown notes.

## Related Files

- `src/crates/core/src/agentic/agents/definitions/modes/intent_coding.rs`
- `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`
- `src/crates/core/src/service/agent_memory/instruction_context.rs`
- `.agent/templates/intent-template.md`
- `.agent/templates/evidence-template.md`
- `.agent/rules/accepted-checks.md`
- `.agent/rules/context-budget.md`
- `.agent/rules/error-classification.md`
- `.agent/rules/provenance-chain.md`
- `.agent/rules/risk-classification.md`

## Last Reviewed

2026-05-25

