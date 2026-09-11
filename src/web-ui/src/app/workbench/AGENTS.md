# Workbench

Follow the Web UI guide. Keep main tab/history ownership in sceneStore, resource
identity/origin in contentResourceStore, and document buffers in EditorDocument.
Do not reintroduce File View / Panel View containers, nested main content tab bars,
mount-dependent open queues, or a dependency on the active workspace during saves.

Focused verification from the repository root:

```bash
pnpm --dir src/web-ui run test:run src/app/workbench/contentResourceStore.test.ts src/app/workbench/workbenchContent.test.ts src/app/workbench/canvasTabTransfer.test.ts src/tools/editor/services/EditorDocument.test.ts src/app/components/panels/content-canvas/hooks/canvasPanelOwnership.test.tsx src/app/components/SceneBar/SceneBar.test.tsx src/app/components/panels/content-canvas/tab-bar/TabBar.test.tsx
pnpm run check:web
```

Use the parent guide's Session state checks when changing activation or lifetime,
and the editor guide's focused suite when changing document behavior. Do not use
mock-based visual verification or browser control for this module.
