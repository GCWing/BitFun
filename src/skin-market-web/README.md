# OpenBitFun Skin Market Web

Public catalog and authenticated contribution workflow for reviewed OpenBitFun
Appearance packages.

- Web base: `/skin/`
- API base: `/skin/api/v1`
- Routes: `/skin/`, `/skin/appearances/:slug`, `/skin/submissions`, and `/skin/admin`
- Features: catalog search and filters, release details and downloads, personal submission status and withdrawal, plus administrator review and publishing
- Installation remains a OpenBitFun Desktop action through Settings > Appearance.
- Visitors without the desktop client reach `https://openbitfun.com/download`
  from the catalog masthead and from every appearance page (`src/GetOpenBitFunCta.tsx`,
  `src/links.ts`).

The site is self-contained and does not import the main Web UI locale or theme catalogs.
GitHub identity is shared with the MiniApp market through its same-origin auth
broker. Skin writes use the `/skin`-scoped CSRF alias issued by that broker.
Local development proxies `/miniapp/api` to `127.0.0.1:9710`; set
`MINIAPP_MARKET_DEV_API` when the broker runs elsewhere.

Preview endpoints retain the normalized original when no query is present and
offer bounded, versioned WebP variants: `?variant=compact-v1` (640px maximum
edge) and `?variant=large-v1` (1280px maximum edge). Catalog and embedded-client
cards use compact previews; larger web/detail surfaces use responsive candidates.
The server generates variants lazily, stores them beside the content-addressed
original, and serves public variants with immutable cache headers.

```bash
pnpm --dir src/skin-market-web dev
pnpm --dir src/skin-market-web type-check
pnpm --dir src/skin-market-web test
pnpm --dir src/skin-market-web build
```
