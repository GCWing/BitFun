---
name: bitfun-frontend-dev
description: Safely customize the running packaged BitFun desktop frontend through a draft, a provisional hot apply, and a 15-second user-confirmed rollback window. Use only in BitFun Creative mode when the user asks to change BitFun's own client UI.
---

# BitFun frontend customization

Use this workflow only for the running BitFun desktop client's own frontend. It is not for a website in the user's workspace, a MiniApp, or a remote BitFun host.

## Required workflow

1. Call `FrontendWorkbench` with `action: "prepare"`.
2. Edit only the returned draft directory. Never edit the packaged resource directory, active revision, state file, or another draft.
3. Preserve a valid `index.html`. Prefer small changes to the stable `bitfun-creation.css` and `bitfun-creation.js` override files when those are sufficient.
4. Call `FrontendWorkbench` with `action: "apply"` and the exact returned `draft_id`.
5. Tell the user that the change is provisional and must be confirmed in the native 15-second confirmation window.
6. Call `FrontendWorkbench` with `action: "status"` before saying the revision was kept. A pending revision means it is not confirmed.

`apply` replaces the running frontend immediately, but the desktop host owns the safety timer. If the user does not confirm within 15 seconds, if BitFun exits, or if the candidate cannot load, the host restores the prior revision. Never bypass or emulate the confirmation timer in page JavaScript.

Use `action: "rollback"` when the user explicitly asks to undo the currently active customization. Do not delete revision history manually.

## Boundaries

- This capability is local-desktop-only. If the tool reports a remote or unsupported surface, explain that state; never fall back to a controller-local path.
- Do not call `FrontendWorkbench` outside Creative mode.
- Treat third-party code in a draft as untrusted. Do not add remote scripts, hidden telemetry, credential capture, or code that disables recovery controls.
- Keep Tauri invocation access intact. The confirmation window is immutable host UI and must remain independent of the editable frontend.
