# BitFun Appearance package contract

## Package shape

- Archive extension: `.bitfun-appearance`
- Archive format: ZIP
- Root manifest: `appearance.json`
- `schema`: `bitfun.appearance`
- `schemaVersion`: `2`
- Image formats: PNG, JPEG, WebP, or GIF
- Background video formats: MP4 or WebM
- Imported packages are sparse overlays merged with the built-in light or dark Appearance package.

The package is declarative. It cannot carry CSS, selectors, markup, scripts, SVG, fonts, external URLs, or executable content. Video is accepted only through top-level `backgroundMedia` and never through component, scene, or material Style IR.

## Top-level model

An Appearance manifest may define:

- package identity and mode;
- an explicit package preview asset reference;
- an optional host-managed background video and poster;
- required capabilities;
- typed global values;
- reusable materials;
- registered component definitions;
- registered scene definitions;
- renderer settings;
- declared package assets;
- SHA-256 integrity hashes.

`preview` uses `{ "kind": "asset", "assetId": "..." }` and must reference a declared image asset when present.

`backgroundMedia` uses this closed shape:

```json
{
  "kind": "video",
  "assetId": "background-video",
  "posterAssetId": "background-poster",
  "fit": "cover",
  "position": "center"
}
```

The video asset must be `video/mp4` or `video/webm`; the poster must be an image. Declare `background-media.v1`. Playback is host-owned: muted, looping, non-interactive, paused while hidden, and replaced by the poster for reduced-motion users.

Use `scripts/bitfun_appearance.py init` for a valid starting structure. Use its `contract` commands for the exact standalone registry.

## Style IR

Values are typed records, not raw CSS. Common kinds include:

- colors: `hex`, `rgb`, `transparent`, `ref`;
- lengths: `px`, `percent`, `zero`, `lengthKeyword`, `ref`;
- assets: `asset` with `assetId`;
- compound values: shadow, transform, background size, background position.

Materials are definitions with `style` and an optional `visualRole`. Scene and component Part rules compose one to eight material ids through `materials: string[]`, then add `base`, `facets`, `states`, and `contexts` styling. The singular `material` field is unsupported.

```json
{
  "materials": {
    "quiet-card": {
      "visualRole": "card",
      "style": {
        "backgroundColor": { "kind": "ref", "path": "globals.colors.surface" }
      }
    }
  },
  "components": {
    "skill-card": {
      "parts": {
        "root": {
          "materials": ["quiet-card"],
          "decorationIntent": "framed"
        }
      }
    }
  }
}
```

`decorationIntent` accepts `flat`, `separator`, or `framed`.

Part descriptors may define `propertyProfile`, `allowedProperties`, `forceableProperties`, `visualRole`, and `continuityGroup`. Query the registry instead of assuming every Part accepts every Style IR property. `cascade: "override"` only makes descriptor-approved forceable paint properties important.

Only registered properties are accepted. Part descriptors may further narrow `allowedProperties`.

## Background layers

For multiple layers, provide arrays of equal length:

- `backgroundImages`
- `backgroundSizes`
- `backgroundPositions`
- `backgroundRepeats`
- `backgroundBlendModes`

The first image is painted topmost.

## Limits

- compressed archive: 96 MiB;
- expanded content: 128 MiB;
- manifest: 256 KiB;
- one image: 16 MiB;
- one background video: 64 MiB;
- preview image: 4 MiB;
- files: 64 maximum;
- image side: 16,384 pixels maximum;
- image pixels: 50 million maximum.

The production host additionally limits video to 60 seconds, 4,096 displayed pixels per side, and 9 million displayed pixels, and verifies that the current WebView can decode the container and codec. Pixel aspect ratio affects the displayed dimensions reported by the browser; a coded 3840x2160 stream can still exceed the limit when its sample aspect ratio is not 1:1. The standalone Python validator verifies structure, file size, and container signature; `verify_host.py` additionally probes packaged video display dimensions, while final import remains the codec compatibility check.

The package rejects unsafe ZIP paths, symlinks, undeclared files, unsupported media, invalid references, and integrity mismatches.

## Registry provenance

`appearance-registry.json` is a self-contained production registry snapshot. Consumers of this skill do not need BitFun source code for offline authoring. Query the snapshot through the script instead of editing it by hand. When a checkout is available, run `python scripts/sync_registry.py <bitfun-repo> --check` before relying on the snapshot. The check compares exported contract content and ignores provenance-only commit or timestamp differences.

Maintainers synchronize it with `scripts/sync_registry.py`. Provenance includes the source revision, dirty state, tree evidence hash, and generation timestamp; a matching commit hash alone does not prove that a dirty working tree is synchronized.

Published snapshots must come from a clean checkout. `--allow-dirty` exists only for local investigation.

## Validation diagnostics

The standalone validator and production host report stable issue codes and manifest paths. The standalone validator also reports non-blocking normalization, continuity, and cascade warnings. The production host additionally attaches component or scene context; unknown-Part issues include the surface's currently allowed Part ids. Product UI groups these issues by surface or package section instead of flattening them into one message.

When import fails, use the reported path and allowed-Part list to update the manifest, then confirm the current descriptor with `contract show`. Do not rename or map an unknown Part based on another package.
