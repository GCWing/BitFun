# Skin market service

This crate owns the concrete HTTP, SQLite, artifact, validation, review and
retention behavior for the public Skin catalog. “Skin” is product copy only;
the package/runtime contract remains Appearance (`.openbitfun-appearance`,
`appearance.json`, `openbitfun.appearance`). Stable DTOs and pure state policy live
in `openbitfun-product-domains::appearance_market`.

The service is isolated from the MiniApp market database and artifacts. It
does not own OAuth credentials: authenticated Desktop requests carry the
existing MiniApp market Bearer token, which is forwarded only to the configured
MiniApp `/me` endpoint. Browser contribution and review routes use the
MiniApp broker's `/skin`-scoped session aliases. Unsafe requests are verified
with the matching CSRF cookie and header through `POST /me`; unrelated browser
cookies are never forwarded.

Key invariants:

- approved releases are immutable; yank/unpublish are explicit moderation
  state, never in-place artifact rewrites;
- package SHA, canonical review metadata and normalized preview SHA bind the
  review bundle hash;
- only declared package-local raster/video assets are accepted; preview output
  is normalized to same-origin WebP;
- the no-query preview URL remains the normalized original; only the bounded
  `compact-v1` (640px) and `large-v1` (1280px) query variants are accepted,
  generated lazily beside the original and removed with it;
- listing slugs and package IDs cannot be transferred between owners through
  an update submission;
- upload size, expansion, entry count, media dimensions and MIME are bounded
  before publication;
- retention removes only unreferenced, expired draft artifacts.

Focused verification:

```bash
cargo test -p openbitfun-product-domains --no-default-features --features appearance-market
cargo test -p openbitfun-skin-market-service
cargo check -p openbitfun-skin-market-server
```

Production deployment, backup and rollback are documented in
`deploy/skin-market/README.md`.
