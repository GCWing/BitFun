---
name: agent-eval-canvas
description: >-
  Create a BitFun Canvas for single-case agent evaluation and trajectory
  diagnosis. Use when the user asks to analyze one agent run, trace, case,
  benchmark item, failure trajectory, eval result, or critical failure step,
  and wants an incident-review style report covering verdict, step timeline,
  root cause, error propagation, tool/evidence analysis, efficiency, safety,
  and repair recommendations.
---

# Agent Eval Canvas

Use this skill to produce a session-scoped BitFun Canvas for a **single agent
case**. The report should read like an incident review or clinical diagnosis,
not like an aggregate benchmark summary. Its job is to explain what happened in
one run, which step determined success or failure, why, and what to fix next.

Read and follow `bitfun-canvas` first. It defines the Canvas tool workflow,
source rules, SDK surface, design constraints, and final response requirements.

## Inputs

Gather the case evidence before writing TSX:

- Case ID, task type, original user request, expected result, actual result,
  final verdict, score, model, prompt version, tools, max steps, timeout, and
  environment facts when available.
- Full or summarized trajectory: LLM calls, tool calls, handoffs, guardrail
  events, custom events, observations, retries, and final answer.
- Tool call details: tool name, call timing, arguments, raw or summarized
  result, error state, whether the result was used, and recovery behavior.
- Evidence artifacts: files, retrieved sources, code snippets, tables, judge
  output, screenshots, or execution logs that support the verdict.
- Cost facts: total latency, steps, LLM calls, tool calls, tokens, repeated
  calls, invalid steps, budget/timeout status.
- Safety facts: risky commands, permission prompts, sensitive data, production
  side effects, prompt injection exposure, and guardrail behavior.

If the trace or expected result is missing, ask for it unless the user clearly
wants a best-effort report from partial evidence. In a best-effort report, mark
unsupported fields as unavailable and keep root-cause claims conservative.

## Canvas Structure

Lead with diagnosis, then evidence. The first screen should answer: did the
case pass, what was the decisive step, and what should change next.

1. **Case header**: Case ID, task type, verdict, score, run configuration, and
   a one-sentence conclusion.
2. **Diagnostic summary**: final outcome, critical failure or success step,
   primary root cause, downstream propagation, confidence, and top repair.
3. **Trajectory timeline**: compact step table or swimlane showing behavior
   type, agent action, input/observation, reasonableness, tag, and issue.
4. **Step-level evaluation**: per-step cards or rows with labels:
   `OK`, `WARN`, `ERROR`, `ROOT_CAUSE`, and `PROPAGATED`.
5. **Critical step deep dive**: why this step changed the run, what evidence
   proves it, why later errors are downstream, and what should have happened.
6. **Root cause and propagation**: category breakdown plus a clear chain from
   first error to final result.
7. **Tool and evidence analysis**: tool choice, parameters, results, result
   use, recovery behavior, unsupported claims, stale evidence, and source fit.
8. **Cost, efficiency, and safety**: latency, tokens, duplicate work, invalid
   steps, risky actions, sensitive data, and confirmation/guardrail handling.
9. **Repair plan and regression**: fixes grouped by Prompt, Planner, Tool
   schema, Tool result, Memory/RAG, Runtime, Evaluator, and Guardrail; include
   next-run pass criteria and steps to watch.

For successful cases, replace the failure emphasis with the **critical success
step**: the moment the agent made the run robust, such as verifying evidence,
recovering from an empty result, narrowing a query, or correcting a plan.

## Diagnostic Rules

- Separate **root-cause errors** from **propagated errors**. A later bad answer
  is not the root cause if it merely follows from earlier wrong evidence.
- Do not write "model hallucination" as the only cause. Classify concrete
  failure modes: task understanding, planning, tool choice, tool parameters,
  observation understanding, retrieval, evidence use, memory contamination,
  reflection/recovery, output format, resource limit, environment, or safety.
- Tie every critical claim to trace evidence. If the trace does not support a
  root cause, label it as a hypothesis and show what evidence is missing.
- Preserve the user's wording for the original request. Redact secrets, access
  tokens, private personal data, and credentials before rendering.
- Treat tool errors carefully: distinguish wrong tool choice, wrong arguments,
  noisy/empty results, tool/runtime failure, and correct result misread by the
  agent.
- Include "what should have happened" for the critical step. Make it concrete:
  expected constraint extraction, query rewrite, retry policy, validation,
  human confirmation, or final evidence check.
- For partial-success cases, show which user requirements passed, which failed,
  and whether remaining failures are independent or downstream of one root
  cause.

## Useful Representations

Pick the layout that makes the diagnosis fastest to understand:

- A verdict strip with score, pass/fail, confidence, critical step, and primary
  failure category.
- A horizontal timeline or vertical trace table for step-by-step behavior.
- A dependency or propagation graph: `Step 3 -> Step 4 -> Step 6 -> Final`.
- A root-cause matrix separating primary cause, secondary contributors, and
  downstream symptoms.
- A tool-call audit table with arguments, result quality, result use, and
  recovery.
- An evidence ledger mapping final claims to observations, with unsupported or
  stale claims highlighted.
- A repair backlog grouped by system layer and tagged with expected impact.

Use `computeDAGLayout` plus inline SVG for propagation graphs when it is clearer
than text. Use `DiffView` only when the case is code-review-like and the exact
hunk is part of the evidence.

## Report Template

Cover these fields when the evidence supports them:

```text
Case information:
- Case ID
- Task type
- Original user request
- Expected result
- Actual result
- Verdict: success / partial success / failure / timeout
- Agent configuration
- Overall score

One-sentence conclusion:
- Direct statement of why the case passed or failed.

Trajectory summary:
- Step
- Behavior type
- Agent behavior
- Tool / observation result
- Judgment
- Issue

Critical step:
- Critical failure step / critical success step
- Failure or success type
- Concrete behavior
- Why this step is decisive
- What should have happened

Root cause:
- Primary category
- Secondary category
- Root cause vs propagated symptom
- Confidence and missing evidence

Propagation:
- Step X -> Step Y -> Step Z -> Final

Tool analysis:
- Selection
- Timing
- Parameters
- Return quality
- Observation use
- Retry or fallback

Evidence analysis:
- Supported claims
- Unsupported claims
- Misread, stale, irrelevant, or missing evidence

Cost and safety:
- Latency, steps, calls, tokens, duplicate work, invalid steps
- Risky actions, sensitive data, prompt injection, guardrails, confirmations

Repair and regression:
- Prompt
- Planner
- Tool schema / result
- Memory / RAG
- Runtime
- Evaluator
- Guardrail
- Regression inclusion and pass criteria
```

## Do Not

- Do not turn a single case into an aggregate benchmark dashboard.
- Do not paste the complete raw trace. Summarize the trajectory and quote only
  the spans needed to justify the diagnosis.
- Do not invent missing metrics, traces, CI status, tool results, or model
  configuration. Mark unavailable facts as unavailable.
- Do not bury the verdict under metadata. The first screen must show outcome,
  critical step, root cause, and next fix.
- Do not make every section a uniform card stack. Use cards for high-signal
  summaries and deep dives; use open sections, tables, compact callouts, and
  diagrams for the rest.
- Do not expose credentials, secrets, private data, or unnecessarily verbose
  user content in the canvas.

## Output Self-Check

Before calling `CreateCanvas`, verify:

- The report answers the five core questions: outcome, trajectory, critical
  step, root cause category, and next repair.
- The critical step is justified by trace evidence, not only final outcome.
- Root-cause and propagated errors are separated.
- Tool, evidence, cost, and safety sections are present when relevant.
- The repair plan is specific enough to implement or test.
- The first screen is diagnostic, not just descriptive metadata.
- The design passes the `bitfun-canvas` slop-pattern check.

## Output

Call `CreateCanvas` with a concise title and the complete TSX source. In the
final response, give the returned `bitfun-canvas://...` artifact reference and
mention the trace or case source used.
