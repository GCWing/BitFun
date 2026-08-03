# BitFun Skin Market Web

Public, read-only catalog for reviewed BitFun Appearance packages.

- Web base: `/skin/`
- API base: `/skin/api/v1`
- Routes: `/skin/` and `/skin/appearances/:slug`
- Features: search, light/dark filter, newest/download sorting, cursor pagination, release and compatibility details, package downloads
- Installation remains a BitFun Desktop action through Settings > Appearance.

The site is self-contained and does not import the main Web UI locale or theme catalogs.
GitHub identity is shared with the MiniApp market through its same-origin auth
broker. Local development proxies `/miniapp/api` to `127.0.0.1:9710`; set
`MINIAPP_MARKET_DEV_API` when the broker runs elsewhere.

```bash
pnpm --dir src/skin-market-web dev
pnpm --dir src/skin-market-web type-check
pnpm --dir src/skin-market-web test
pnpm --dir src/skin-market-web build
```
