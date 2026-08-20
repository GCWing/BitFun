# Architecture index

Purpose: complete topic map for stable architecture docs under `docs/architecture/`.
Scope: product runtime boundaries and cross-cutting design authorities.
Status: stable
Authority language: Chinese for most design bodies; this index is English for AI / ops navigation.

Governance: [`docs/guideline/docs-governance.md`](../guideline/docs-governance.md).
Product norms entry: root [`AGENTS.md`](../../AGENTS.md). Specs / plans: [`docs/specs/README.md`](../specs/README.md).

This directory owns stable cross-module boundaries and accepted designs. It
must not contain implementation task lists, user setup guides, benchmark dumps,
temporary review notes, or module-local coding rules. Proposed targets must be
marked as proposed and must not be presented as shipped behavior.

## Start here

| Topic | Authority |
|---|---|
| Product runtime boundaries / decomposition | [`product-architecture.md`](product-architecture.md) |
| Rust build dependency boundaries | [`rust-build-dependency-boundaries.md`](rust-build-dependency-boundaries.md) |
| Agent Runtime deployment / multi-instance | [`agent-runtime-deployment-design.md`](agent-runtime-deployment-design.md) |
| Agent Runtime lifecycle sequence | [`agent-runtime-lifecycle-sequence.md`](agent-runtime-lifecycle-sequence.md) |
| Agent runtime & services | [`agent-runtime-services-design.md`](agent-runtime-services-design.md) |
| Agent SDK product shape | [`agent-sdk-product-architecture.md`](agent-sdk-product-architecture.md) |
| App Server architecture (proposed target) | [`app-server-architecture.md`](app-server-architecture.md) |
| CLI / TUI product line | [`cli-product-line-design.md`](cli-product-line-design.md) |
| HarmonyOS PC / portability | [`platform-portability-design.md`](platform-portability-design.md) |
| Product customization | [`product-customization-blueprint.md`](product-customization-blueprint.md) |
| Appearance package system | [`appearance-package-system.md`](appearance-package-system.md) |
| i18n | [`i18n.md`](i18n.md) |
| Theme / color tokens | [`theme-token-optimization.md`](theme-token-optimization.md) |
| Peer device mode | [`peer-device-mode.md`](peer-device-mode.md) |
| Remote workspace transport | [`remote-workspace-transport.md`](remote-workspace-transport.md) |
| Session projection | [`session-projection.md`](session-projection.md) |
| Review lifecycle | [`review-lifecycle.md`](review-lifecycle.md) |
| Deep Review | [`deep-review.md`](deep-review.md) |
| Detached task dispatch | [`detached-task-dispatch.md`](detached-task-dispatch.md) |
| Cache-friendly messages | [`cache-friendly-message-structure.md`](cache-friendly-message-structure.md) |
| Model request cache reuse | [`model-request-cache-reuse.md`](model-request-cache-reuse.md) |
| SDLC governance architecture | [`sdlc-governance-architecture.md`](sdlc-governance-architecture.md) |
| Security boundary | [`security-boundary.md`](security-boundary.md) |
| Quality data plane | [`quality-data-plane.md`](quality-data-plane.md) |
| Evidence pack | [`evidence-pack.md`](evidence-pack.md) |
| Artifact graph | [`artifact-graph.md`](artifact-graph.md) |
| Project profile integration | [`project-profile-integration.md`](project-profile-integration.md) |
| Agent workflow design | [`agent-workflow-design.md`](agent-workflow-design.md) |

## Extensions

| Topic | Authority |
|---|---|
| OpenCode compatibility | [`extensions/opencode-extension-compatibility.md`](extensions/opencode-extension-compatibility.md) |
| External AI work sources | [`extensions/external-ai-work-sources-design.md`](extensions/external-ai-work-sources-design.md) |
| Capability runtime integration | [`extensions/capability-runtime-integration-design.md`](extensions/capability-runtime-integration-design.md) |
| Plugin runtime and host | [`extensions/plugin-runtime-design.md`](extensions/plugin-runtime-design.md) |
| OpenCode config assets | [`extensions/opencode-config-assets-adapter-design.md`](extensions/opencode-config-assets-adapter-design.md) |
| OpenCode plugin runtime adapter | [`extensions/opencode-plugin-runtime-adapter-design.md`](extensions/opencode-plugin-runtime-adapter-design.md) |
| OpenCode TUI plugin adapter | [`extensions/opencode-tui-plugin-adapter-design.md`](extensions/opencode-tui-plugin-adapter-design.md) |
| OpenCode external integrations | [`extensions/opencode-external-integration-adapter-design.md`](extensions/opencode-external-integration-adapter-design.md) |
