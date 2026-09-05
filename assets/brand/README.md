# OpenBitFun application brand assets

`source/` contains the two transparent Logo masters supplied for the product:

- `openbitfun-mark-dark.png` is the dark mark for light surfaces.
- `openbitfun-mark-light.png` is the light mark for dark surfaces.

The application icon is generated from the light mark. It uses the supplied
black-and-white application treatment, removes the construction grid from the
reference, and applies a transparent rounded-corner mask. The construction
grid is not a shippable asset.

Run `pnpm run generate-brand-assets` after changing a source master. The
generator is the single owner of the derived desktop, web, installer, Android,
iOS, and HarmonyOS files.
