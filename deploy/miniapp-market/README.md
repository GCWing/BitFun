# BitFun MiniApp Market deployment

This directory deploys only the MiniApp market. It must not alter the existing
Relay, New API, website, or their Nginx virtual hosts.

## Production paths

- Application checkout: `/srv/bitfun-miniapp-market/app`
- SQLite: `/srv/bitfun-miniapp-market/data/market.sqlite`
- Content-addressed packages/screenshots: `/srv/bitfun-miniapp-market/artifacts`
- Backups: `/srv/bitfun-miniapp-market/backups`
- Secrets: `/etc/bitfun-miniapp-market/market.env` (`root:root`, mode `0600`)
- Origin listener: `127.0.0.1:9710`
- Public URL: `https://market.openbitfun.com/miniapp/`

The GitHub OAuth callback must be exactly:

`https://market.openbitfun.com/miniapp/api/v1/auth/github/callback`

## First deployment

1. Check out the exact reviewed Git commit under the application path.
2. Create the three persistent directories and assign `data` and `artifacts`
   to UID/GID `10001`; keep `backups` readable only by root.
3. Copy `market.env.example` to the secret path, generate a random session
   secret, and add the GitHub OAuth client credentials. Keep
   `MARKET_PUBLIC_BROWSE=false`.
4. Export `MARKET_GIT_COMMIT="$(git rev-parse HEAD)"` and run
   `docker compose -f deploy/miniapp-market/docker-compose.yml up -d --build`.
5. Install `nginx-log-format.conf` under Nginx's `http` context and the
   dedicated vhost, run `nginx -t`, then reload Nginx. The market access log
   deliberately omits IP addresses, query strings, cookies, and request bodies.
6. Install `backup.sh` and the systemd service/timer. Run one backup and
   `restore-drill.sh` before enabling the timer.
7. Sign in as GitHub ID `24753352`, submit and approve one sample MiniApp,
   verify a clean desktop install and manual update, then set
   `MARKET_PUBLIC_BROWSE=true` and recreate the container.

The production container runs read-only as non-root with all Linux capabilities
dropped. Backups stored on the same server do not provide off-site disaster
recovery.
