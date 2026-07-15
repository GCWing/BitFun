# AGENTS.md

Native mobile applications are product entrypoints under `src/apps/mobile`.

## Boundaries

- Keep Android, iOS, and HarmonyOS lifecycle and platform API usage inside the
  corresponding platform directory.
- Keep reusable product logic platform-agnostic and expose it through stable
  contracts or adapters.
- Do not import implementation details from `src/apps/desktop` or
  `src/web-ui`.
- Treat remote workspace support as part of feature design. Gate unsupported
  behavior with a clear user-facing state.
- Keep credentials, signing files, provisioning profiles, device identifiers,
  and local SDK paths out of the repository.
- Add platform-specific build and verification commands here when a native
  toolchain is selected.

## Directory Ownership

| Directory | Ownership |
|---|---|
| `android/` | Android app, resources, lifecycle, and adapters |
| `ios/` | iOS app, resources, lifecycle, and adapters |
| `harmonyos/` | HarmonyOS app, resources, lifecycle, and adapters |
