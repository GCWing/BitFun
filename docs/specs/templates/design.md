# <Title> Design

Date: <YYYY-MM-DD>
Status: draft | in-progress | stable
Related spec: <path or none>
Scope: <affected paths and layers>
Authority language: <zh-CN | en>

## Problem

## Confirmed approach

Data flow, command/response shapes, port vs implementation ownership.

## Layer ownership

Which Product Surface / Assembly / Feature / Kernel / Adapter / Service / Execution / Contracts own what. Cite [`docs/architecture/product-architecture.md`](../../architecture/product-architecture.md) when relevant.

## State model

| State | Meaning | Owner | Independence notes |
|---|---|---|---|

## Risk scan

| Dimension | Affected? | Risk / evidence | Mitigation or N/A reason |
|---|---|---|---|
| Security / permissions | | | |
| Credentials / privacy | | | |
| Network / external systems | | | |
| Data or state migration | | | |
| Release / packaging / rollout | | | |
| Remote / multi-host | | | |
| i18n | | | |
| Theme / interaction | | | |

## Migration and compatibility

Existing data/config/protocol behavior, compatibility window, and migration failure behavior. Use `N/A` with a reason when no persisted or versioned state changes.

## Remote compatibility

Local-only? Remote workspace? New network/SSH/agent-loop round-trips?

## Failure / cancel / partial success

## i18n and theme

New locale keys (namespace / shared terms). Prefer existing semantic/component/domain tokens; new color tokens need an owner contract.

## Security

Execution location, sandbox, side effects, auth scope. Unknown capability defaults to restricted.

## Release and rollout

Feature gates, staged rollout, packaging/deployment impact, monitoring, and stop conditions. Use `N/A` with a reason when release behavior is unchanged.

## Rollback

How to restore the previous behavior and data/state interpretation; identify irreversible steps.

## Alternatives considered

## Architecture alignment

Conflicts with architecture docs? If yes, justification.

## Test approach

Unit / contract / focused E2E paths.
