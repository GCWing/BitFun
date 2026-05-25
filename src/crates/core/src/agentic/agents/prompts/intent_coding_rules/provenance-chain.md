# Provenance Chain Rules

Intent Coding tasks should preserve a compact chain of custody from request to delivery. The chain should be useful for review without copying full logs or large outputs.

## Minimum Chain

Record these anchors when applicable:

- Original request: the user request or a concise reference to it.
- Context inputs: key `AGENTS.md`, built-in intent coding rules, or module docs used.
- Intent Record: path to the accepted Intent Record.
- Acceptance: accepted checks/tests and user decisions.
- Execution: files changed and major implementation decisions.
- Verification: commands/checks run and results.
- Repair loop: failure classes and repair attempt count when verification fails.
- Review escalation: Deep Review or equivalent review status for L3/L4.
- Evidence Package: path to the final Evidence Package.

## What Not To Store

Do not include:

- Secrets, tokens, credentials, customer data, or private local configuration.
- Full command logs when a short summary is enough.
- Large diffs already available through Git.
- Tool outputs that include sensitive or irrelevant data.

## Evidence Requirement

Every Evidence Package should include:

- A `Provenance Chain` section.
- Links or paths to Intent Record and Evidence Package.
- Key context inputs.
- Verification and repair anchors.
- Human decisions that changed scope, risk, or acceptance.

