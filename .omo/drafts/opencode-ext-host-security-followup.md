---
slug: opencode-ext-host-security-followup
status: drafting
intent: clear
review_required: false
pending-action: write .omo/plans/opencode-ext-host-security-followup.md
approach: Revise only the ext-host IPC design so every claimed security/reliability property has an explicit protocol state, owner, failure transition, and fixture; preserve the shared RuntimeServices-to-Host default 1:1 model and existing owner chain.
---

# Draft: opencode-ext-host-security-followup

## Components (topology ledger)
<!-- Lock the SHAPE before depth. One row per top-level component that can succeed or fail independently. -->
<!-- id | outcome (one line) | status: active|deferred | evidence path -->
gateway-auth | Per-instance gateway authenticates a generation-bound caller before accepting a body or allocating streams | active | docs/architecture/extensions/opencode-ext-host-ipc-design.md:338
prepared-target | P0 states an implementable cross-platform target-integrity property without claiming unavailable OS immutability | active | docs/architecture/extensions/opencode-ext-host-ipc-design.md:109
cancel-terminal | Cancellation distinguishes signal delivery from invocation termination and recycles an unproven Host generation | active | docs/architecture/extensions/opencode-ext-host-ipc-design.md:315
post-import-gate | Contribution expansion after import enters a separate approval/stop/restart state before publication | active | docs/architecture/extensions/opencode-ext-host-ipc-design.md:79
liveness | An out-of-band watchdog detects a blocked main event loop even without business requests | active | docs/architecture/extensions/opencode-ext-host-ipc-design.md:355
version-and-qa | Audit pins, protocol exit conditions, fixtures, and document verification align with OpenCode v1.18.9 | active | docs/architecture/extensions/opencode-ext-host-ipc-design.md:440

## Open assumptions (announced defaults)
<!-- Record any default you adopt instead of asking, so the user can veto it at the gate. -->
<!-- assumption | adopted default | rationale | reversible? -->
gateway transport | retain the loopback HTTP compatibility gateway and authenticate a high-entropy capability URL/token bound to instanceID + connection_generation; reject before body/stream work | preserves OpenCode serverUrl compatibility while closing unauthenticated port enumeration | yes, protocol can later move to a private pipe
gateway residual risk | explicitly retain shared-Host non-isolation as residual risk; token authentication protects other local processes and blind cross-instance probing, not a plugin that has compromised the shared Host process | consistent with plugin-runtime-design.md shared-process threat model | yes
gateway P0 scope | define and fixture the authenticated gateway protocol now, but do not enable Client/serverUrl gateway behavior in the first Tool/Hook vertical slice | preserves the minimal P0 while preventing the unauthenticated protocol from becoming a future compatibility dependency | yes
cancel state | frozen `{cancelled:true}` means signal_delivered only; ordinary Cancelled requires a later terminal invocation event, otherwise timeout => poison/recycle + OutcomeUnknown | matches upstream implementation behavior | yes
liveness | use a dedicated supervisor control channel handled outside the plugin/main JS event queue and report main-loop progress generations; a normal ping on the business queue is forbidden | required by upper plugin-runtime design | yes
post-import expansion | publish no expanded contribution; stop the shared Host, enter AwaitingApproval, then restart the same target group if approved or only compliant targets if rejected | existing adapter guardrail requires separate pre/post import gates | yes
test strategy | no TDD for documentation-only work; agent-executed links, diff, hygiene, boundary checks, plus review of exact protocol/fixture acceptance clauses | no executable bridge exists in this PR | yes

## Findings (cited - path:lines)
1. Gateway requests currently carry instance context but no caller credential: docs/architecture/extensions/opencode-ext-host-ipc-design.md:338-353. Upstream e084c921 gateway.ts:16-51 is loopback HTTP without token/auth.
2. The document claims read-only/OS-enforced immutability at docs/architecture/extensions/opencode-ext-host-ipc-design.md:117-121, while repository process_tree explicitly is not a sandbox and no ACL/mount/principal/loader mechanism exists: src/crates/services/services-core/src/process_tree.rs:1-7.
3. docs/architecture/extensions/opencode-ext-host-ipc-design.md:315-324 treats cancel confirmation as terminal; upstream e084c921 host.ts:348-408 only calls AbortController.abort and immediately acknowledges.
4. Candidate validation exists at docs/architecture/extensions/opencode-ext-host-ipc-design.md:289-290, but the required distinct post-import expansion gate exists only in src/crates/adapters/opencode-adapter/AGENTS.md:23-27 and docs/architecture/extensions/opencode-plugin-runtime-adapter-design.md:134-138.
5. Event-loop stall is named at docs/architecture/extensions/opencode-ext-host-ipc-design.md:363 without a heartbeat/watchdog in services/protocol/fixtures; docs/architecture/extensions/plugin-runtime-design.md:140-142 forbids relying on the blocked business queue.
6. docs/architecture/extensions/opencode-ext-host-ipc-design.md:448 calls plugin 1.17.18 the current compatibility target, while docs/architecture/extensions/opencode-extension-compatibility.md:24-33 fixes current stable compatibility at v1.18.9.
7. Reviewer main commit 8f9d4945 is not present in the local object database, so its 11-commit divergence cannot be independently verified in this planning workspace; do not treat it as a document-edit requirement.

## Decisions (with rationale)
1. Do not reintroduce bridge, Session holder, workspace Host owner, or per-plugin processes; all five changes extend the existing shared Host and owner chain.
2. Gateway authentication must be part of the new public protocol commit, schema snapshot, and cross-language fixtures, not a Rust-only convention.
3. Cancellation response taxonomy must separate signal_delivered, invocation_terminal, not_found, and connection_lost/unknown; only a terminal fact permits ordinary Cancelled.
4. Post-import capability expansion is a process-level safety transition because import may already have started timers or side effects; rejecting registration alone is insufficient.
5. Liveness must observe main-event-loop progress from outside the plugin event queue and bind every observation to connection_generation.
6. Mark @opencode-ai/plugin@1.17.18 as the frozen ext-host audit dependency; require a new fixture aligned with OpenCode stable v1.18.9 before P0 exit.
7. Owner decision accepted: prepared-target storage is tamper-evident in P0, using canonical attestation, content addressing, atomic staging, pre-import revalidation, and generation fencing. It is not called immutable or OS-enforced read-only; post-import mutation and delayed dynamic import remain explicit residual risks.
8. Owner decision accepted: retain loopback HTTP gateway compatibility, authenticated by a high-entropy capability bound to instanceID + connection_generation and rejected before request-body or stream allocation. Gateway activation remains outside the first Tool/Hook vertical slice.

## Scope IN
1. One documentation file: docs/architecture/extensions/opencode-ext-host-ipc-design.md.
2. Protocol shape, lifecycle transitions, owner boundaries, error taxonomy, implementation checklist, verification fixtures, audit/version wording, and P0 exit conditions for all five findings.
3. Internal Markdown-link validation and repository documentation checks.

## Scope OUT (Must NOT have)
1. No Rust/TypeScript implementation, protocol snapshot, fixture files, dependency installation, rebase, commit, or PR metadata edit.
2. No changes to plugin-runtime-design.md, opencode-plugin-runtime-adapter-design.md, opencode-adapter AGENTS.md, or current shared Host default 1:1 ownership.
3. No claim that Job Objects, process groups, chmod, ordinary ACLs, or content hashes alone provide an OS sandbox or immutable filesystem.
4. No claim that current upstream e084c921 already implements the target protocol.

## Open questions
None. Test strategy: documentation-only, no TDD; all QA is agent-executed.

## Approval gate
status: awaiting-approval
approach: Update the one IPC design document in six coordinated sections: authenticated future gateway; tamper-evident target contract; cancellation terminal-state protocol; post-import expansion stop/approve/restart gate; out-of-band liveness watchdog; version/fixture/exit-condition alignment. Preserve shared Host default 1:1 and all current owners.
next workflow action: After explicit approval, create .omo/plans/opencode-ext-host-security-followup.md, run mandatory Metis gap analysis, and write the decision-complete documentation work plan. Do not implement.
<!-- When exploration is exhausted and unknowns are answered, set status: awaiting-approval. -->
<!-- That durable record is the loop guard: on a later turn read it and resume at the gate instead of re-running exploration. -->
