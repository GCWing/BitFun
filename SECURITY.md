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

## Taiji Module Security

### 1. Taiji Module Security Boundaries

- **Data source security assumptions (CTP / Replay / TTS)**:
  - CTP data feeds are assumed to originate from authorized brokerage connections. The taiji module does not validate CTP credentials; it relies on the upstream CTP adapter for authentication.
  - Replay data is treated as trusted local input. Any replay file sourced from an untrusted origin must be sanitized before ingestion.
  - TTS (time-tick simulation) data is synthetic and carries no external trust dependency, but parameter tampering could skew backtest results.

- **Strategy engine sandbox boundaries**:
  - User-defined strategies run in-process and are not sandboxed. Strategy code has the same OS-level privileges as the taiji runtime. Review all third-party strategy code before execution.
  - Network access from strategies is not restricted by default. Strategies that require isolation should be executed in a dedicated container or VM.

- **Real-time data transport encryption**:
  - Real-time market data over TCP/UDP should be transported over encrypted channels (TLS) in production environments. Plaintext CTP connections are acceptable only in isolated test networks.

### 2. Taiji Dependency Security

- **Non-crates.io dependency audit**:
  - Any dependency sourced outside crates.io (Git repositories, local paths, vendor forks) must go through a manual audit before inclusion. Record the audit result in the dependency's commit history or a tracked decision log.
  - Pin non-crates.io dependencies to a specific commit hash; do not use floating branches.

- **ffmpeg-sidecar security notes**:
  - `ffmpeg-sidecar` downloads a prebuilt ffmpeg binary at runtime. Verify the binary's checksum against a trusted baseline before first use in production.
  - Bundle a known-good ffmpeg binary with the taiji distribution if the runtime download path is unacceptable for your environment.

### 3. Sensitive Information Handling

- **Strategy parameters and API keys**:
  - Always prefer environment variables for strategy parameters, API keys, and broker credentials. Do not hardcode secrets in strategy source files, YAML configuration, or commit them to version control.
  - Use a `.env` file (excluded from Git via `.gitignore`) for local development. In production, inject secrets through the platform's secret management mechanism.
  - Logging must never emit full API keys or credential strings. Mask sensitive fields before writing to log output.
