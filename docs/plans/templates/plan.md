# <Topic> Implementation Plan

**Goal:** <one-line product goal>.
**Architecture:** <layer ownership and boundaries>.
**Tech stack:** <frameworks / libs / test tools>.
**Status:** draft | in-progress | completed
**Related spec/design:** <paths>
**Authority language:** <zh-CN | en>

## Verification baseline

Use the smallest matching rows in [`docs/guideline/verification.md`](../../guideline/verification.md).

## Risk scan

| Dimension | Affected? | Risk / evidence | Mitigation or N/A reason |
|---|---|---|---|
| Security / credentials | | | |
| Network / external systems | | | |
| Data or state migration | | | |
| Release / packaging / rollout | | | |
| Remote / multi-host | | | |
| i18n / theme / interaction | | | |

## Milestone 1: <slice>

Risk: <Low|Medium|High>. <one-line reason>.

### Task 1: <independently deliverable change>

- [ ] Change: <files / behavior / boundary>.
- Risk: <Low|Medium|High>. <one-line reason>.
- Verify: <focused command or review evidence>.
- Rollback: <how to undo or fall back; use N/A only with a reason>.

## Milestone 2: <slice>

Risk: <Low|Medium|High>. <one-line reason>.

- Repeat the Task block above. Every task must be independently verifiable and reversible.

## Migration and compatibility

Ordering, persisted state/protocol compatibility, and failure handling. Use `N/A` with a reason when unchanged.

## Release and rollout

Feature gates, staged rollout, packaging/deployment impact, monitoring, and stop conditions. Use `N/A` with a reason when unchanged.

## Rollback

Cross-task rollback order, compatibility fallback, and irreversible steps. Per-task rollback belongs in every Task block, not only high-risk tasks.

## Risks

| Risk | Mitigation |
|---|---|

## Closeout checklist

- [ ] Matching verification green
- [ ] Stable conclusions moved to architecture / feature authority (or N/A noted)
- [ ] Spec renamed / status set to `completed` if closing
