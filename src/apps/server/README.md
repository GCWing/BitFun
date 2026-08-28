# BitFun Server (Web App Backend)

This directory contains the `bitfun-server` application, which serves the web backend runtime for BitFun.

> **Deprecated:** This Web Server was already deprecated before the current App Server refactor. Changes made here
> during that refactor are intended to validate protocol and host boundaries; they do not promise feature completeness,
> Desktop parity, backward compatibility, or production readiness.

If you are looking for **Remote Connect self-hosted relay deployment**, use:

- [Relay Server README](../relay-server/README.md)
- [deploy.sh](../relay-server/deploy.sh)

`src/apps/server` and `src/apps/relay-server` are different components. `src/apps/server` is the main web app backend, while `src/apps/relay-server` is the relay service used by Remote Connect.

When exercising configured OpenCode plugins, use `pnpm run server:dev` or
`pnpm run server:build`. These commands prepare the extension Host first; a
compatible `bun` command (or `BITFUN_BUN_COMMAND`) is required at runtime. The
release build stages `resources/ext-host` beside the server binary; deploy that
directory with the binary.
