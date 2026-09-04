# OpenBitFun production host

This directory is the restore entry for the current production origin
(`ssh lwb`). Read [README.md](README.md) before changing cron, Nginx, the
release mirror, Relay, the website, New API, or either market.

This is a **move of an existing origin**, not a green-field install. Do not
re-run the export/import steps against the live `lwb` host. Do not invent a
second copy of `scripts/openbitfun-release-sync.sh`. Cron must run the in-repo
script from the OpenBitFun checkout.

MiniApp and Skin markets have their own runbooks. Do not mix their checkouts,
secrets, or Compose projects with this host index. Do not generate new market
secrets when the old `market.env` files still exist.
