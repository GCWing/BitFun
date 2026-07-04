# Security Policy

[中文版](./SECURITY_CN.md)

BitFun is a desktop-grade Agent runtime (Rust core + Tauri shell) that runs on your own machine with broad capabilities—filesystem, terminal, Git, MCP/LSP, and remote control. Because of this reach, we take security reports seriously and appreciate the community's help in keeping users safe.

## Supported Versions

BitFun is currently in active `0.x` development and ships as a rolling release. Security fixes land on the latest release; older versions are not patched separately.

| Version | Supported |
| ------- | --------- |
| Latest release (`main`) | ✅ |
| Older releases | ❌ |

Please upgrade to the latest [release](https://github.com/GCWing/BitFun/releases) before reporting an issue to confirm it still reproduces.

## Reporting a Vulnerability

**Please do not open a public issue, discussion, or pull request for security vulnerabilities.** Public disclosure before a fix is available puts users at risk.

Instead, report privately through GitHub Security Advisories:

➡️ **[Report a vulnerability](https://github.com/GCWing/BitFun/security/advisories/new)**

This opens a private channel visible only to the maintainers. If you are unable to use GitHub Security Advisories, open a minimal public issue that says only "I'd like to report a security issue privately"—without any details—and a maintainer will follow up with a private channel.

To help us triage quickly, please include where you can:

- A clear description of the vulnerability and its impact
- The affected component (Rust core, desktop/Tauri, web UI, mobile-web pairing, server/relay, CLI, installer, etc.)
- Step-by-step reproduction instructions or a proof of concept
- Affected version(s), operating system, and configuration
- Any suggested mitigation or fix, if you have one

## Disclosure Process

- We aim to acknowledge new reports within **5 business days**.
- We will work with you to confirm the issue, assess severity, and determine a fix timeline, keeping you updated on progress.
- Once a fix is released, we will publish a security advisory and credit the reporter unless you prefer to remain anonymous.
- We follow coordinated disclosure: please give us a reasonable window to ship a fix before any public disclosure.

## Scope

In scope:

- The BitFun runtime, official Agents, desktop/CLI/server apps, web UI, and the mobile-web pairing/remote-control flow in this repository.

Out of scope:

- Issues in third-party dependencies (please report those upstream; let us know if a BitFun update is needed).
- Vulnerabilities that require a pre-compromised machine, physical access, or already-elevated privileges.
- Risks inherent to running an autonomous Agent with capabilities you explicitly grant it (e.g., a tool you authorized acting within its granted permissions).

## Safe Harbor

We will not pursue or support legal action against researchers who, in good faith, discover and report vulnerabilities in accordance with this policy and who avoid privacy violations, data destruction, and service disruption during testing.

Thank you for helping keep BitFun and its users safe.
