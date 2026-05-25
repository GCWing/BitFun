# Error Classification Rules

When verification fails in Intent Coding, classify the failure before attempting repair. The goal is to make repair behavior auditable and prepare for future routing.

## Failure Classes

Use one or more classes:

- `syntax_error`: parser, formatter, invalid JSON, malformed config, or invalid markup.
- `type_error`: TypeScript, Rust, schema, or API contract mismatch.
- `test_failure`: automated test assertion failure.
- `lint_failure`: lint, style, formatting, or static check failure.
- `runtime_error`: command exits from runtime exception, panic, crash, or unhandled rejection.
- `missing_dependency`: missing package, binary, tool, feature flag, or generated artifact.
- `environment_failure`: sandbox, network, permission, filesystem, platform, or unavailable service issue.
- `behavior_mismatch`: output does not satisfy an Accepted Check/Test even if commands pass.
- `security_violation`: secret exposure, unsafe permission broadening, injection risk, or policy violation.
- `unknown`: insufficient evidence to classify.

## Repair Attempt Record

For each failed verification, record:

- Command or check that failed.
- Failure class.
- Short evidence summary.
- Repair action taken.
- Whether the same failure repeated.

## Escalation

Escalate to the user instead of continuing blind repair when:

- The same failure class repeats without new evidence.
- The fix would broaden scope beyond the Intent Record.
- The repair requires a new dependency or risky file category.
- The failure appears to be environmental and cannot be resolved locally.
- The repair path conflicts with accepted intent.

## Evidence Requirement

Every Evidence Package should include repair-loop data when any verification fails:

- Failure classes observed.
- Repair attempts count.
- Final repair status: `not_needed`, `repaired`, `blocked`, or `deferred`.
- Remaining verification gaps.

