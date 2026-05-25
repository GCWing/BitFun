# Security Rules

These rules define repository-wide security constraints for Coding Agent tasks.

## Secrets

- Do not commit secrets, tokens, certificates, private keys, or sensitive local configuration.
- Do not print secrets in logs, test output, screenshots, or evidence packages.

## Sensitive Areas

- Do not change authentication, authorization, billing, deployment, release signing, or database migration files unless the Intent Record explicitly includes that scope.
- Do not broaden permissions, network access, filesystem access, or desktop automation capabilities without explicit approval.

## Dependencies

- Do not add dependencies without approval.
- When a dependency is approved, document its purpose and check license compatibility.

## Agent Loop Safety

- Do not address looping behavior first with hard-coded string, pattern, or count blockers.
- Investigate tool behavior, model interaction, context packaging, prompt/tool schema design, and state synchronization before adding loop controls.

