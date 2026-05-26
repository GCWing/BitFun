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

## Baseline Gates

Use the smallest relevant set. Prefer these gate identifiers:

- `scope`: Changes stayed within the accepted Intent Record.
- `verification`: Required verification commands were run or explicitly skipped.
- `security`: No secrets, credentials, unsafe auth changes, or malicious behavior were introduced.
- `risk_review`: L3/L4 review routing was completed, skipped, or blocked with evidence.
- `dependencies`: New dependencies were not introduced without approval.
- `platform_boundary`: Platform-specific behavior stayed behind adapters.
- `remote_compatibility`: Remote workspace impact was considered when relevant.

## Future Upgrade Path

A later policy-as-code layer can evaluate these gates automatically. It should
preserve the same reviewable output shape so Evidence Packages remain useful to
humans.
