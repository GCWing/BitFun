# Third-Party Notices

BitFun redistributes selected material and links libraries from the following
third-party projects. These notices are included with BitFun's Desktop and CLI
release packages.

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

## anydoc

- Project: anydoc
- Source: https://github.com/firecrawl/anydoc
- Version: 0.1.6
- License: MIT
- Copyright: Copyright (c) 2026 Sideguide Technologies Inc.

BitFun links anydoc to convert supported office documents, OpenDocument files,
RTF, EPUB, CSV, and PDFs into Markdown for the Agent Read tool.

MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
