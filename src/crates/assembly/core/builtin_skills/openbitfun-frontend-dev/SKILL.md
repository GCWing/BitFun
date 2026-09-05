---
name: openbitfun-frontend-dev
description: Safely customize the running packaged OpenBitFun desktop frontend through a draft, a provisional hot apply, and a 15-second user-confirmed rollback window. Use only in OpenBitFun Creative mode when the user asks to change OpenBitFun's own client UI.
---

# OpenBitFun frontend customization

Use this workflow only for the running OpenBitFun desktop client's own frontend. It is not for a website in the user's workspace, a MiniApp, or a remote OpenBitFun host.

## Required workflow

1. Call `FrontendWorkbench` with `action: "prepare"`.
2. Edit only the returned draft directory. Never edit the packaged resource directory, active revision, state file, or another draft.
3. Read the returned `openbitfun-creation-api.md`. Edit `openbitfun-creation.css`, `openbitfun-creation.js`, and optional `creation-assets/` only. The draft contains no application bundle or `index.html`. It needs no source repository, Node.js, dependencies, compiler, or rebuild. CSS loads last; JavaScript is a browser ES module exporting `default function activate(ui)`, called after the real shell renders. Use `ui.mount` slots and documented semantic selectors; return cleanup for your listeners/timers.
4. Call `FrontendWorkbench` with `action: "apply"`, setting its `draft_id` input to the exact returned `draftId`.
5. Before calling `apply`, tell the user to inspect the live preview and use the immutable review window. Its Keep button remains disabled until both the real shell and customization activate; the 15-second countdown starts after readiness.
6. Read the final `apply` result. Only `status: "confirmed"` means the revision was kept; `status: "rolled_back"` includes the reason. Use `FrontendWorkbench status` for later inspection, not direct reads of `state.json`.
7. Use `FrontendWorkbench inspect` to check actual mount slots, registered commands and diagnostics. For reusable capabilities, register commands with `ui.commands.register`, compose persistent `ui.state` with `ui.events`, and verify behavior with `FrontendWorkbench invoke` using the discovered schema. Commands and subscriptions follow activation; state survives code rollback. These commands run in the visible local client, not a headless service.

`apply` is a two-phase transaction: the confirmed active revision remains authoritative while the candidate is previewed, then user confirmation commits it. If the app shell cannot report readiness, the user does not confirm within 15 seconds, or OpenBitFun exits, the host restores the prior revision. Never bypass or emulate readiness or the confirmation timer in editable page JavaScript.

Use `action: "rollback"` when the user explicitly asks to undo the currently active customization. Do not delete revision history manually.

## Boundaries

- This capability is local-desktop-only. If the tool reports a remote or unsupported surface, explain that state; never fall back to a controller-local path.
- Do not call `FrontendWorkbench` outside Creative mode.
- Treat third-party code in a draft as untrusted. Do not add remote scripts, hidden telemetry, credential capture, or code that disables recovery controls.
- Keep Tauri invocation access intact. The confirmation window is immutable host UI and must remain independent of the editable frontend.

For existing UI actions/settings, first discover the actual capability with
`OpenBitFunControl` and execute/configure it. A CSS override is not a substitute
for a real product setting. For installed MiniApps, use the `feature.miniapps`
structured operations, not frontend bundle edits. Compatible application upgrades
retain these small customizations and use the newly installed frontend bundle.
If apply reports a stale draft, prepare again and preserve the latest customization.
