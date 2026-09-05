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
The beta release contains Desktop and Installer assets only. CLI and Relay
floating releases remain stable-only.

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
