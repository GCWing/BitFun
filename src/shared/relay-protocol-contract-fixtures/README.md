# Relay protocol contract fixtures

The relay wire format is currently hand-decoded in four places: the HarmonyOS
app (`src/apps/mobile/harmonyos`), the mobile web client (`src/mobile-web`), the
Kotlin shared core (`src/apps/mobile/shared`), and the desktop peer that
produces the payloads. Nothing but review has kept those four in agreement.

These fixtures are the shared reference. Each file pairs a captured payload with
the normalized result every client must produce from it:

```json
{
  "description": "why this payload exists",
  "cases": [
    { "name": "...", "wire": { ... }, "expected": { ... } }
  ]
}
```

`wire` is what arrives on the socket. `expected` is the decoded form *before*
product defaults are applied — alias resolution and type coercion only, no
"Session ab12cd" placeholder titles and no `"code"` agent-type fallback. Those
are policy and differ by surface, so pinning them here would make the fixture
wrong for whichever client changes first.

## Consumers

| Client | Test |
|---|---|
| Kotlin shared core | `core-protocol` `commonTest`, via generated `RelayContractFixtures` |

HarmonyOS and mobile-web adopt these incrementally; a client is listed here only
once its tests actually read the files.

## Adding a case

Prefer capturing a real payload over inventing one — the value of these fixtures
is that they record what peers actually send, including spellings that look like
mistakes. When a peer is found emitting a new alias, add a case rather than only
extending the resolution list in code.
