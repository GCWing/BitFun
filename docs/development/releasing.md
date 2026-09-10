# Release Channels

OpenBitFun packages use an immutable build-time release channel. End users do not
switch channels at runtime.

## Stable

Stable releases continue to be driven by a version bump on `main`. The
`Release On Version Bump` workflow creates `vMAJOR.MINOR.PATCH` and dispatches
`Desktop Package` with the default `stable` channel.

## Beta

Run `Desktop Package` manually with:

- `tag_name`: the immutable release tag, for example `v0.2.18-beta.1`;
- `checkout_ref`: the commit or branch to build when the tag does not exist;
- `release_channel`: `beta`;
- `upload_to_release`: disabled for internal Actions artifacts, enabled for a
  public GitHub pre-release.

Beta versions target the next stable version. If stable is `0.2.17`, the first
candidate is `0.2.18-beta.1`, not `0.2.17-beta.1`. Do not use SemVer build
metadata in a published package version; the release already records the Git
commit separately.

Public beta assets are stored on the immutable version tag. After every asset
and signature is verified, the workflow updates only the `latest.json` asset on
the `channel-beta` pre-release. Beta Desktop builds read that pointer and fall
back to `https://openbitfun.com/release/beta/latest.json`.
Beta releases include Desktop and Installer assets, Linux CLI and Relay Server
archives for x86_64 and aarch64, and a multi-platform Relay image for linux/amd64
and linux/arm64. Release publication requires every producer to succeed. Archives
include checksums and signatures; `linux-binaries.json` and the signed
`relay-image.json` descriptor live on the immutable version release.

Relay images use `ghcr.io/<repository-owner>/openbitfun-relay-server` with the
version tags `v1.0.0-beta.N` and `1.0.0-beta.N`. Beta never updates the `latest`
image tag. Fork builds publish to the fork owner's image namespace and their
own GitHub Release URLs. GHCR credentials must permit package publication, and
the package must be publicly readable for the anonymous-pull verification to
pass.

With `upload_to_release` disabled, the workflow keeps CLI/Relay archives in
Actions artifacts and validates the runtime image build without pushing it.
The explicit `relay_image_only` backfill mode remains a publishing operation.
It resolves the same immutable tag as the existing archives, rather than the
current workflow commit. Releases predating the current OpenBitFun artifact
layout are not image-rebuild inputs.

Install Beta CLI archives manually and deploy the Relay with an explicit Beta
image tag or its signed descriptor's digest. The default CLI install/update and
Relay one-click deployment paths stay on stable; Beta CLI builds do not run
stable-feed automatic update checks. This does not add a runtime channel switch
or a Beta option to one-click deployment. The stable CLI/Relay mirror manifests
and the Desktop-only `channel-beta/latest.json` pointer remain unchanged.
When no usable current stable Relay image exists, one-click deployment builds
current source on the target host automatically. It does not deploy a differently
named product image or silently promote a Beta image to stable.

The selected ref must resolve to a commit in the protected `main` history. The
workflow pins that SHA before dispatching platform jobs and rejects an existing
release tag if it points somewhere else. Configure the signing secrets and the
public beta approval policy so untrusted pull-request code cannot access them.
This protected-history requirement applies to the canonical `GCWing/OpenBitFun`
repository; forks may run packaging from their own test branches. A fork beta
uses that fork's `channel-beta` release as both updater origins, so it cannot
silently consume or mutate the canonical beta channel.

A stable release promotes the beta pointer only when its version is not older
than the current beta. This lets beta users move from `0.2.18-beta.N` to
`0.2.18` without allowing a late workflow to roll the channel backward.

Beta and stable currently share the same bundle identity and data directories.
Installing beta replaces stable; side-by-side installation is not supported.

## Mirror

The mirror script defaults to stable. Run a separate beta sync with:

```bash
OPENBITFUN_RELEASE_CHANNEL=beta scripts/openbitfun-release-sync.sh
```

The beta invocation writes below `/release/beta` and intentionally skips the
stable-only CLI and Relay floating manifests.

Production cron must run this in-repo script from the OpenBitFun checkout. Do not
create a detached copy. Host paths, Nginx, and the rest of the origin restore
steps live in [`deploy/openbitfun-host/README.md`](../../deploy/openbitfun-host/README.md).
The sync resolves the exact release directory from the updater manifest once;
Relay and Linux metadata use that same directory to avoid mixed-version reads
when GitHub's latest-release pointer changes. Publication also compares the
downloaded Relay descriptor and signature to the image job's signed bytes.

## Focused packaging checks

For release workflow and channel-isolation changes, run
`pnpm run check:github-config` and
`node --test scripts/relay/package-contract.test.mjs scripts/tauri-release-manifest.test.mjs`.
These checks exercise release conditions, image tag selection, Beta manifest
generation, and asset collection with fixtures; they do not build or publish packages.
