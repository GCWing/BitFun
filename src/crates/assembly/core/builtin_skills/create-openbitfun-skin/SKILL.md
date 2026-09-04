---
name: create-openbitfun-skin
description: Inspect the current OpenBitFun Appearance contract and author, repair, package, or validate sparse .openbitfun-appearance skins. Use when Codex needs to discover registered scenes/components/parts/states/facets, check Style IR properties or renderer tokens, initialize or inspect a package, diagnose import failures, verify a skin against a OpenBitFun checkout, or choose a bundled style example for design-specific guidance.
---

# OpenBitFun Appearance Authoring

Use this Skill as the authority for what a OpenBitFun skin may change and how that change is represented. Let the user's assets, product direction, and the selected example determine the visual design.

The bundled registry and package references define capabilities. Files under `examples/` define optional style choices; they never expand the contract or prescribe coverage for another skin.

## Route the task

- For a new or repaired package, read [authoring-workflow.md](references/authoring-workflow.md) and [package-contract.md](references/package-contract.md).
- For renderer configuration, also read [renderer-contracts.md](references/renderer-contracts.md).
- For generated video or WebP assets, also read [media-quality-policy.md](references/media-quality-policy.md).
- For a style-specific design, load only the closest example after reading the parent workflow.
- For import failures, inspect the issue path, query that surface, and repair the sparse manifest instead of copying another package.
- For OpenBitFun contract changes, synchronize the registry before changing examples or skins.

## Discover the exposed surface

Run commands from this Skill directory:

```powershell
python scripts/openbitfun_appearance.py contract list components
python scripts/openbitfun_appearance.py contract list scenes
python scripts/openbitfun_appearance.py contract list renderers
python scripts/openbitfun_appearance.py contract show components <component-id>
python scripts/openbitfun_appearance.py contract show scenes <scene-id>
python scripts/openbitfun_appearance.py contract properties
python scripts/openbitfun_appearance.py contract tokens theme
python scripts/openbitfun_appearance.py contract tokens widget
```

Only author registered surfaces, Parts, facets, states, renderer adapters, tokens, and Style IR properties returned by these commands. A Part descriptor may further restrict its allowed properties.

A component owns only the Parts returned for that component. Nested visual owners expose their own component ids and must be styled through those ids. Do not infer Parts from DOM structure, another skin, or an example surface plan.

Surface-level states may use an `ancestorPart` selector rooted at `root`. Define the state rule using the registered state id; do not reproduce host selectors in the package.

Important independent owners include `toolbar-mode`, `floating-mini-chat`, `session-menu`, `composer-voice-input`, `miniapp-bubble-welcome`, `session-title-config`, `assistant-card`, `workspace-item`, `external-mcp-overview`, `miniapp-customize-panel`, `user-message-edit-composer`, `voice-input-diagnostics`, `flow-chat-turn-rail`, `copyable-text-preview`, `reasoning-preset-selector`, `reasoning-config-panel`, `reasoning-preset-editor`, `market-account-controls`, and `miniapp-market-view`. Query each owner before styling it. This list is a navigation aid, not an exhaustive contract; the registry is authoritative.

The following Parts are not registered and must not be used:

- `toolbar-mode.input`
- `toolbar-mode.sessionMenu`
- `floating-mini-chat.input`
- `floating-mini-chat.inputBar`
- `floating-mini-chat.sessionMenu`

## Author a sparse package

Initialize the smallest valid overlay:

```powershell
python scripts/openbitfun_appearance.py init <project-dir> --id <id> --name "<name>" --mode <light-or-dark>
```

Edit `<project-dir>/appearance.json` and declare only the values the skin needs to change. OpenBitFun merges the package over its built-in light or dark Appearance.

Packages may contain PNG, JPEG, WebP, and GIF images. MP4 or WebM is allowed only through top-level `backgroundMedia`, and every video requires an image poster. CSS files, selectors, markup, scripts, fonts, remote URLs, SVG, and executable content are unsupported.

## Select a style example

Currently bundled:

| Example | Load when | Do not inherit blindly |
| --- | --- | --- |
| [cinematic-animated-wallpaper](examples/cinematic-animated-wallpaper/SKILL.md) | Animated character artwork, source-derived glass materials, image-led cards, or illustrated dialogs | Its asset roles, crop defaults, palette, and selected surface plan |

Read [style-example-contract.md](references/style-example-contract.md) before adding or restructuring an example. An example is a design recipe validated against the registry, not a second contract snapshot.

## Validate importability

```powershell
python scripts/openbitfun_appearance.py validate <project-dir>
python scripts/openbitfun_appearance.py build <project-dir> --output <skin>.openbitfun-appearance
python scripts/openbitfun_appearance.py validate <skin>.openbitfun-appearance
python scripts/openbitfun_appearance.py inspect <project-or-archive>
```

When a OpenBitFun checkout is available, verify with the production registry, Validator, Compiler, and packaged-video display-dimension probe:

```powershell
python scripts/verify_host.py <openbitfun-repo> <skin>.openbitfun-appearance --report <host-verification.json> --strict-warnings
```

Standalone validation proves package and bundled-registry compatibility. Host verification proves the selected checkout accepts and compiles the package. Neither proves runtime visual quality; import the skin and inspect the actual surfaces.

## Reuse deterministic support

Example authors may import:

- `scripts/build_support.py` for JSON, hashing, package initialization, validation, archive creation, host verification, build-record checks, and runtime checklist creation.
- `scripts/media_support.py` for video probing, contact sheets, robust frame extraction, normalized crops, WebP output, VP9 encoding, preview sheets, and host video limits.

Keep style-specific asset roles, crop decisions, palettes, material construction, renderer choices, surface selection, and runtime checks inside the example.

## Maintain the registry

Treat the bundled registry as the offline authority. Confirm it before targeting a checkout:

```powershell
python scripts/sync_registry.py <openbitfun-repo> --check
```

The check compares the exported Appearance contract, not provenance-only commit or timestamp fields. Unrelated OpenBitFun commits do not invalidate a compatible snapshot.

Refresh it only from a clean checkout after OpenBitFun changes Appearance descriptors:

```powershell
python scripts/sync_registry.py <openbitfun-repo>
```

Do not hand-edit `references/appearance-registry.json`. Use `--allow-dirty` only for local investigation, never for a distributed snapshot.
