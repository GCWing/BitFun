---
name: create-openbitfun-cinematic-wallpaper-skin
description: Apply the cinematic animated-wallpaper style recipe to a OpenBitFun Appearance skin. Use after loading the parent create-openbitfun-skin Skill when the user provides animated character artwork and wants a host-managed video background, source-derived glass materials, image-led cards, illustrated dialogs, reproducible builds, or cinematic drift checks.
---

# Cinematic animated-wallpaper recipe

Load the parent `create-openbitfun-skin` Skill first. The parent registry and package references define what can be changed; this example defines one optional visual strategy.

Do not reuse this example's asset roles, crop defaults, palette, opacity, or material choices unless they fit the user's artwork and intent. Its [surface-plan.json](references/surface-plan.json) is an example style selection validated against one registry revision, not a list of all current Appearance surfaces. Re-query the parent registry whenever the host checkout or bundled snapshot changes.

Read [style-playbook.md](references/style-playbook.md) for visual decisions and [palette-contract.md](references/palette-contract.md) before creating a custom palette.
Read the parent [media-quality-policy.md](../../references/media-quality-policy.md) before overriding video or WebP quality.

## Style strategy

This recipe uses:

- a host-managed VP9 WebM background with an image poster;
- source-derived artwork for floating surfaces, sidebars, cards, and dialogs;
- translucent cinematic materials over the artwork;
- a semantic palette shared by Style IR and renderer adapters;
- broad surface coverage suitable for an immersive character-led skin.

Keep the built-in Appearance for any owner that does not benefit from this treatment. Query the parent registry before adding, removing, or changing a selected surface.

## Output layout

Keep the importable project isolated from source and report files:

```text
<skin-root>/
  package/
    appearance.json
    assets/
      background.webm
      floating.webp
      sidebar.webp
      card-detail.webp
      card-portrait.webp
      dialog.webp
      preview.webp
  sources/
    palette.json
    surface-plan.json
  contact-sheet.png
  asset-preview-sheet.png
  <skin-id>.openbitfun-appearance
  host-verification.json
  skin-build.json
  runtime-checklist.json
```

Validate and build only `<skin-root>/package`. Reports and source files inside that directory invalidate the project.

## Choose a source frame

Generate only the timestamped contact sheet:

```powershell
python examples/cinematic-animated-wallpaper/scripts/build_cinematic_skin.py contact `
  --source <animated-wallpaper.mp4> `
  --output <skin-root>
```

Inspect `contact-sheet.png`, select a frame time, and decide normalized crops. Do not continue with blind crops.

## Create a palette

Copy [default-palette.json](references/default-palette.json) outside the Skill and tune it to the source artwork. Keep every required semantic key and six-digit hex value.

The recipe maps the palette into cinematic materials plus CSS, Widget, Monaco, xterm, Mermaid, and Canvas renderer settings. The parent Skill does not prescribe these mappings for other styles.

## Build the recipe

Run from the parent Skill directory:

```powershell
python examples/cinematic-animated-wallpaper/scripts/build_cinematic_skin.py build `
  --source <animated-wallpaper.mp4> `
  --output <skin-root> `
  --id <skin-id> `
  --name "<Skin name>" `
  --frame-time <seconds> `
  --palette <palette.json> `
  --openbitfun-repo <openbitfun-repo>
```

The thin recipe entrypoint combines cinematic asset generation and manifest construction with the parent's generic build support. It initializes the package, generates previews and assets, copies resolved recipe inputs, scaffolds the manifest, validates project and archive, runs strict host verification, and writes the reproducibility record.

Production host warnings fail by default. Use `--allow-warnings` only for diagnosis.

The default `--video-quality auto` tries codec-lossless VP9 only for small workloads, otherwise starts at CRF 20 and adapts only when the encoded file exceeds the host limit. The default `--static-quality-mode auto` tries lossless WebP before a quality fallback. Use `--video-quality lossless` only when exceeding 64 MiB should be treated as a hard failure; use `--video-crf` to reproduce a known fixed encoding.

Inspect `asset-preview-sheet.png`. Re-run `build --force` with adjusted crop arguments when any role is poorly framed.

## Rebuild and check

```powershell
python examples/cinematic-animated-wallpaper/scripts/build_cinematic_skin.py rebuild `
  --output <skin-root>

python examples/cinematic-animated-wallpaper/scripts/build_cinematic_skin.py check `
  --output <skin-root>
```

Build records use `openbitfun.appearance.recipe-build` and identify this recipe.

`skin-build.json` records the requested policy, every attempted encoding, and the selected codec-lossless/CRF or WebP quality result. `check` verifies source, palette, surface-plan, manifest and archive drift, package validity, and current host compatibility. Add `--skip-host-verify` only for an offline check.

## Recipe implementation

- [cinematic_recipe.py](scripts/cinematic_recipe.py) owns palette interpretation, materials, renderers, asset declarations, and manifest generation.
- [build_assets.py](scripts/build_assets.py) owns cinematic asset roles and their crop, tint, brightness, and blur decisions.
- [build_cinematic_skin.py](scripts/build_cinematic_skin.py) is the thin command entrypoint.
- Parent `scripts/build_support.py` owns generic package and verification lifecycle operations.
- Parent `scripts/media_support.py` owns generic probing, extraction, crop, encoding, and preview primitives.

Use [scaffold_cinematic.py](scripts/scaffold_cinematic.py) only when generating or checking the cinematic manifest without the full build.

## Runtime inspection

After importing the package, inspect at minimum:

- normal workbench and collapsed navigation;
- detached Toolbar Mode and in-app floating mini chat with long conversations;
- Skills, MiniApp gallery, Agents, and Insights views;
- Settings, archived sessions, and keyboard shortcuts;
- terminal navigation, bottom terminal, files, and auxiliary panels;
- generic and dedicated dialogs;
- text over calm and bright animation frames;
- reduced-motion poster fallback.

Record evidence in `runtime-checklist.json`. `skin-build.json` keeps `runtimeVisualInspection: false` until a real import is inspected; compiler validation does not replace runtime inspection.
