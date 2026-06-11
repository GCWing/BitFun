# Spec: Lower i18n Governance Baseline to Reflect Removed Debt

## Context

The `pnpm run i18n:audit` command on `dev` reports 7 pre-existing errors.
Three of them are all caused by the same drift in the
`sharedTermDuplicates` budget of `scripts/i18n-governance-baseline.json`:

```
[i18n:audit] ERROR sharedTermDuplicates has 184 candidate(s), below baseline 185; lower scripts/i18n-governance-baseline.json.
[i18n:audit] ERROR sharedTermDuplicates web-ui has 169 candidate(s), below baseline 170; lower scripts/i18n-governance-baseline.json.
[i18n:audit] ERROR sharedTermDuplicates sharedKey features.deepReview has 1 candidate(s), below baseline 2; lower scripts/i18n-governance-baseline.json.
```

The audit script (in `scripts/i18n-audit.mjs:1622-1623, 1646, 1678`) treats
the baseline as a **no-growth upper bound**: it errors when the actual count
is strictly greater OR strictly less than the baseline. The intent is to
force a conscious review whenever debt changes, so removing one byte of debt
is just as visible as adding one.

The `description` field of the baseline file says:

> "No-growth baseline for i18n governance candidates. Lower counts when
> shared-term or l10n debt is removed; do not raise without review."

That is exactly the situation: a candidate was removed (the actual count
dropped by 1, from 2 to 1 for `features.deepReview` in the web-ui surface,
which is the only shared term duplicate that actually existed in the
change). The other two errors (maxTotal 185→184, web-ui 170→169) are the
upstream roll-ups of that same single removal.

## Root cause

Some past change (likely a refactor that removed a `t('deepReviewConsent.windowTitle')`
call site or removed a `t(...)` reference whose value happened to equal
the shared term value "Deep Review") dropped the candidate count. The
baseline was not lowered at the time. This is a known pre-existing drift
on the `dev` branch, not caused by any single recent commit.

## Goal

Reflect the current actual state in the baseline so the audit passes
cleanly on the three `sharedTermDuplicates` errors. Do not change any
other budget category, do not change any locale resource, do not change
any source code.

## Design

### Files changed

| File | Change |
|---|---|
| `scripts/i18n-governance-baseline.json` | Lower three numbers by exactly 1 each |

The other four pre-existing errors (sessionFilesBadge drift ×2, CJK source
×1) are out of scope and will be addressed in separate spec/plan
iterations.

### Exact edits

In `scripts/i18n-governance-baseline.json`:

1. Line 9: `"maxTotal": 185` → `"maxTotal": 184`
2. Line 15: `"web-ui": 170` → `"web-ui": 169`
3. Line 25: `"features.deepReview": 2` → `"features.deepReview": 1`

### What stays the same

- `version: 1` (no schema change).
- `description` (the rule that motivates the change is unchanged).
- `confirmedUnusedKeys` and `l10nQualityCandidates` budgets (no errors
  there, no changes needed).
- `sharedTermDuplicates.bySurface` entries for `core`, `installer`,
  `mobile-web`, `relay-static-homepage` (no errors, no changes).
- All other `bySharedKey` entries under `sharedTermDuplicates` (no
  errors, no changes).
- All locale resources, source code, contract files.

## Verification

1. After the change, `pnpm run i18n:audit` should report **only** the
   four errors that are out of scope (sessionFilesBadge ×2, CJK source
   ×1). The three `sharedTermDuplicates` errors must be gone.
2. `pnpm run i18n:contract:test` should still pass (it reads the same
   baseline file for parity checks; the numbers are still consistent
   with the actual count).
3. `pnpm run type-check:web` should pass (no source changes, but it
   confirms the JSON file did not break anything in node resolution).

## Out of scope (future work)

These are tracked separately and are NOT part of this spec:

- The `deepReviewConsent.windowTitle` dead key in en-US/ja-CN/zh-TW
  locale resources (3 lines, 0 source references). It is the actual
  source of the `features.deepReview` shared-term duplicate and removing
  it is the right way to permanently remove the debt. However, removing
  a locale key is a subsystem-A (locale resource) change and will be
  planned in a separate spec.
- The 33 unused `sessionFilesBadge.*` keys in zh-CN and zh-TW
  (`flow-chat.json`).
- The 46 unknown static Web UI i18n keys (mostly in
  `SessionFileModificationsBar.tsx` and `SessionFilesBadge.tsx`).
- The 3 CJK source candidates in `useTtsPlayback.ts`, `useWelcomeVoice.ts`,
  `VoiceConfig.tsx`.

After this spec is applied, audit will go from 7 errors to 4. Subsystem
B (CJK) and subsystem A (sessionFilesBadge + dead keys) remain.

## Risk

Very low. This is a pure numeric adjustment to a configuration file
that already encodes the rule that says "lower this when debt is
removed". No user-visible behavior changes. No locale text changes. No
source code changes.

The only risk is changing the wrong number. The three numbers (maxTotal,
web-ui surface, features.deepReview sharedKey) are all derived from the
same single underlying candidate removal, so they must all be lowered
by exactly 1. No other number in the file is anywhere near the boundary
in the audit output.
