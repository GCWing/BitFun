---
name: bitfun-frontend-dev
description: Safely customize the running packaged BitFun desktop frontend through a draft, a provisional hot apply, and a 15-second user-confirmed rollback window. Use only in BitFun Creative mode when the user asks to change BitFun's own client UI.
---

# BitFun frontend customization

Use this workflow only for the running BitFun desktop client's own frontend. It is not for a website in the user's workspace, a MiniApp, or a remote BitFun host.

## Required workflow

1. Call `FrontendWorkbench` with `action: "prepare"`.
2. Edit only the returned draft directory. Never edit the packaged resource directory, active revision, state file, or another draft.
3. Preserve a valid `index.html`. It already loads `bitfun-creation.css` and `bitfun-creation.js`; do not edit `index.html` merely to link them again. Prefer CSS-only overrides for visual changes and keep JavaScript changes small and reversible.
4. Call `FrontendWorkbench` with `action: "apply"`, setting its `draft_id` input to the exact returned `draftId`.
5. While `apply` is running, tell the user to inspect the live preview and use the immutable review window. Its Keep button remains disabled until the real app shell renders; the authoritative 15-second countdown starts only after readiness.
6. Read the final `apply` result. Only `status: "confirmed"` means the revision was kept; `status: "rolled_back"` includes the reason. Use `FrontendWorkbench status` for later inspection, not direct reads of `state.json`.

`apply` is a two-phase transaction: the confirmed active revision remains authoritative while the candidate is previewed, then user confirmation commits it. If the app shell cannot report readiness, the user does not confirm within 15 seconds, or BitFun exits, the host restores the prior revision. Never bypass or emulate readiness or the confirmation timer in editable page JavaScript.

Use `action: "rollback"` when the user explicitly asks to undo the currently active customization. Do not delete revision history manually.

## Boundaries

- This capability is local-desktop-only. If the tool reports a remote or unsupported surface, explain that state; never fall back to a controller-local path.
- Do not call `FrontendWorkbench` outside Creative mode.
- Treat third-party code in a draft as untrusted. Do not add remote scripts, hidden telemetry, credential capture, or code that disables recovery controls.
- Keep Tauri invocation access intact. The confirmation window is immutable host UI and must remain independent of the editable frontend.
