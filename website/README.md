# BitFun Playbook

BitFun Playbook is the public, bilingual manual for BitFun's user-facing
features and settings. It does not maintain its own list. The build consumes
`../docs/interactive-capabilities/capabilities.json`, generated from the single
semantic source at `../src/shared/interactive-capabilities/catalog.json`.

Desktop Tauri commands are audited separately as implementation coverage. They
never become website pages or user-visible search entries.

```bash
pnpm run capabilities:generate
pnpm run website:dev
pnpm run website:test
pnpm run website:build
```

The color theme defaults to the operating-system preference. The header control
lets readers explicitly choose System, Light, or Dark; an explicit choice is
stored only in that browser.

Every build writes `dist/release.json`. Deploy using its `releaseId`, which
changes for either catalog or website-source changes, so a presentation-only
update never mutates an older release directory.

The production origin is <https://playbook.openbitfun.com>.
