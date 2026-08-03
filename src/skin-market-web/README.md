# BitFun Skin Market Web

Public, read-only catalog for reviewed BitFun Appearance packages.

- Web base: `/skin/`
- API base: `/skin/api/v1`
- Routes: `/skin/` and `/skin/appearances/:slug`
- Features: search, light/dark filter, newest/download sorting, cursor pagination, release and compatibility details, package downloads
- Installation remains a BitFun Desktop action through Settings > Appearance.

The site is self-contained and does not import the main Web UI locale or theme catalogs.

```bash
pnpm --dir src/skin-market-web dev
pnpm --dir src/skin-market-web type-check
pnpm --dir src/skin-market-web test
pnpm --dir src/skin-market-web build
```
