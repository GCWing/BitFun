# Policy Gate Rules

Intent Coding uses lightweight policy gates for this MVP. These gates are not an
OPA/Rego engine yet; they are a machine-checkable checklist that records which
governance checks were considered before delivery.

## Evidence Requirement

Every Evidence Package must include a `Policy Gates` section with one or more
gate lines:

```text
- [passed] gate_id: result summary
- [not_applicable] gate_id: reason summary
- [skipped] gate_id: reason: explicit reason
- [blocked] gate_id: reason: explicit blocker
```

Valid statuses:

- `passed`
- `failed`
- `skipped`
- `blocked`
- `not_applicable`

`failed` gates fail the local workflow checker. `skipped` and `blocked` gates
must include `reason: <reason>`.

## Required Gate Profile

The workflow checker derives a lightweight required gate profile from the
Evidence Package.

Every Evidence Package must include:

- `scope`: Changes stayed within the accepted Intent Record.
- `verification`: Required verification commands were run or explicitly skipped.
- `security`: No secrets, credentials, unsafe auth changes, or malicious behavior were introduced.

Additional required gates:

- `risk_review`: Required for L3/L4 tasks.
- `dependencies`: Required when dependency manifest or lock files are changed.
- `platform_boundary`: Required when platform adapter, desktop-only, Tauri, or adapter surfaces are touched.
- `remote_compatibility`: Required when remote workspace, synchronization, transport, or websocket behavior is touched.

## Optional Policy Config

The checker can load additional gate requirements from:

- `.agent/policy.json`
- `.bitfun/intent-coding-policy.json`

Supported shape:

```json
{
  "required_gates": ["team_review"],
  "risk_gates": {
    "L3": ["risk_review"],
    "L4": ["security_review"]
  },
  "path_gates": [
    { "contains": "src/crates/core/src/agentic/tools/", "gate": "tool_contract" }
  ],
  "text_gates": [
    { "contains": "data deletion", "gate": "data_safety" }
  ]
}
```

Configured gates are additive. They cannot remove built-in required gates.

Optional gates can still be included when useful. Prefer these gate identifiers:

- `risk_review`: L3/L4 review routing was completed, skipped, or blocked with evidence.
- `dependencies`: New dependencies were not introduced without approval.
- `platform_boundary`: Platform-specific behavior stayed behind adapters.
- `remote_compatibility`: Remote workspace impact was considered when relevant.

## Future Upgrade Path

A later policy-as-code layer can evaluate these gates automatically. It should
preserve the same reviewable output shape so Evidence Packages remain useful to
humans.
