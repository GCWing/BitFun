# Appearance authoring workflow

Follow this workflow for any style. A style example may refine design decisions but must not replace these contract checks.

## 1. Define the intended delta

Identify the requested base mode, surfaces, renderers, assets, motion behavior, and accessibility requirements. Prefer a sparse overlay. Leave an owner unchanged when the built-in Appearance already satisfies the design.

Separate four kinds of decisions:

- Contract facts: registered surfaces, Parts, states, facets, contexts, properties, tokens, and limits.
- Style choices: color, opacity, material, imagery, crop, emphasis, and coverage.
- Build facts: inputs, hashes, generated assets, archive, and host revision.
- Runtime evidence: imported screenshots or observations across required states.

## 2. Query before authoring

Use `contract show` for every component or scene being changed. Confirm each Part's property profile and allowed properties. Query renderer tokens before adding CSS or Widget values.

Do not use an example's surface plan to infer current host capabilities. The registry is the authority; the example only records a previously validated selection.

## 3. Initialize and author sparsely

Run `openbitfun_appearance.py init`, then add only required globals, materials, component Parts, scenes, renderers, and assets.

Use typed Style IR values. Compose Parts through `materials: []`; the singular `material` field is unsupported. Keep image references in Style IR image-only and put video only in top-level `backgroundMedia`.

## 4. Inspect assets before packaging

For source artwork, inspect contact sheets or the original image before selecting crops. Generate role-specific previews when one source is reused across different aspect ratios. Check bright and calm frames when a background moves.

Use the default adaptive policy in [media-quality-policy.md](media-quality-policy.md) unless the user explicitly requests a fixed quality mode. Preserve both the requested policy and the resolved encoding in the build record.

Do not infer visual quality from successful encoding. Verify subject placement, text readability, contrast, and reduced-motion fallback separately.

## 5. Validate in layers

Run project validation, build the archive, and validate the archive. Resolve errors by manifest path and issue code. Treat warnings as design or continuity debt and require zero host warnings for finished examples.

When a checkout is available, first check registry synchronization and then run strict host verification. Do not update an example's validated revision until both pass against a clean checkout.

## 6. Record reproducibility

For generated skins, record source paths and hashes, style input hashes, build parameters, output hashes, registry provenance, host verification, and runtime inspection state. Rebuild from the record and provide a read-only drift check.

Treat the manifest `version` as the release identity of the importable package. Bump it whenever the manifest, declared assets, renderer settings, selected surfaces, or host-compatibility migration changes. Use a patch bump for compatible corrections and a minor bump for new visual coverage or capabilities. Preserve the version only for a byte-equivalent rebuild of the same package content or when changing reports and other files outside `package/`.

Keep reports, source files, and contact sheets outside the importable `package/` directory.

## 7. Inspect at runtime

Import the archive into OpenBitFun and inspect every intentionally styled owner plus representative unchanged owners. Include detached windows, long content, dialogs, renderer surfaces, interaction states, reduced motion, and bright/dark background frames where relevant.

Compiler validation and runtime visual inspection are separate completion conditions.
