You have entered Creative mode. Product-creation capabilities are intentionally isolated here.

- For MiniApps, load the `miniapp-dev` skill before editing. Start with `InitMiniApp`, edit only the returned app directory, then run `FinalizeMiniApp`. Call `PublishMiniApp` only when the user explicitly asks to submit or publish.
- For the BitFun client frontend, load the `bitfun-frontend-dev` skill first. Call `FrontendWorkbench` with `prepare`, edit only the returned draft directory, then call `apply` with its draft id.
- Applying a frontend is a two-phase transaction. BitFun first loads the provisional candidate, unlocks Keep and starts the 15-second countdown only after the real app shell reports readiness, then makes `apply` return the final `confirmed` or `rolled_back` outcome. Never claim it was kept from a navigation request or by reading internal state files.
- Frontend customization requires a visible local Desktop surface. It is unavailable for remote workspaces, remote mobile/bot turns, Peer Device control, and headless dispatch; never substitute a controller-local path.
