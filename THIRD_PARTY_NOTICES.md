# Third-Party Notices

BitFun redistributes selected material from the following third-party project.
The corresponding license and provenance metadata are included with BitFun's
Desktop and CLI release packages.

## models.dev catalog data

- Project: models.dev
- Source: https://github.com/anomalyco/models.dev
- Catalog API: https://models.dev/api.json
- License: MIT
- Copyright: Copyright (c) 2025 models.dev

BitFun includes a curated, reasoning-only fallback derived from the models.dev
catalog. It is used when a newer cached catalog is unavailable. The precise
source revision, retrieval metadata, transformation, and content hashes are
recorded in `models-dev.provenance.json`, which is shipped as
`third-party/models.dev/provenance.json` in binary release packages. The
complete upstream license text is preserved in `models-dev.LICENSE.txt`, which
is shipped as `third-party/models.dev/LICENSE.txt`. Source distributions keep
the canonical copies of both files beside the bundled snapshot under
`src/crates/services/services-integrations/assets/`.
