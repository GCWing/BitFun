# BitFun Skin Market Web

Public catalog and authenticated contribution workflow for reviewed BitFun
Appearance packages.

- Web base: `/skin/`
- API base: `/skin/api/v1`
- Routes: `/skin/`, `/skin/appearances/:slug`, `/skin/submissions`, and `/skin/admin`
- Features: catalog search and filters, release details and downloads, personal submission status and withdrawal, plus administrator review and publishing
- Installation remains a BitFun Desktop action through Settings > Appearance.

The site is self-contained and does not import the main Web UI locale or theme catalogs.
GitHub identity is shared with the MiniApp market through its same-origin auth
broker. Skin writes use the `/skin`-scoped CSRF alias issued by that broker.
Local development proxies `/miniapp/api` to `127.0.0.1:9710`; set
`MINIAPP_MARKET_DEV_API` when the broker runs elsewhere.

```bash
pnpm --dir src/skin-market-web dev
pnpm --dir src/skin-market-web type-check
pnpm --dir src/skin-market-web test
pnpm --dir src/skin-market-web build
```
