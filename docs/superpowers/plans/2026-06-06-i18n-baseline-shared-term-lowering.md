# i18n Governance Baseline Lowering — Implementation Plan

> **For AI agents:** Required sub-skill: use `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task by task. Steps use checkbox
> (`- [ ]`) syntax for progress tracking.

**Goal:** Lower three numbers in `scripts/i18n-governance-baseline.json` so the
`pnpm run i18n:audit` command stops reporting the three `sharedTermDuplicates`
errors that flag a previously-removed candidate.

**Architecture:** Single-file, three-line numeric edit. The audit script
(`scripts/i18n-audit.mjs:1620-1623, 1646, 1678`) treats each baseline number
as a strict upper bound: actual count must equal the baseline exactly, or it
errors. The `description` field of the baseline file explicitly says
"Lower counts when shared-term or l10n debt is removed; do not raise without
review." We are removing debt (a candidate was already removed in some past
change), so we lower the baselines to match the new actual state.

**Tech stack:** JSON configuration editing, no test framework changes, no Rust,
no source code.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `scripts/i18n-governance-baseline.json` | modify | Lower three numbers by 1 each so the audit no longer reports the candidate-removal drift |

No new files. No tests. No source changes.

---

## Task 1: Lower the three baseline numbers and verify

**Files:**
- Modify: `scripts/i18n-governance-baseline.json` (lines 9, 15, 25)

- [ ] **Step 1.1: Read the current state of the baseline file**

Read `F:\git-base\BitFun\scripts\i18n-governance-baseline.json` in full and
confirm the file matches what the spec describes (3 numeric edits, no schema
change). Expected current values:
- Line 9: `"maxTotal": 185`
- Line 15: `"web-ui": 170`
- Line 25: `"features.deepReview": 2`

If any of these values is already different (because someone else lowered it
between the spec being written and you running this task), STOP and report
the actual current values. Do not blindly lower.

- [ ] **Step 1.2: Edit line 9 — `maxTotal` from 185 to 184**

In `F:\git-base\BitFun\scripts\i18n-governance-baseline.json`, find this line
at line 9:

```json
      "maxTotal": 185,
```

Replace with:

```json
      "maxTotal": 184,
```

- [ ] **Step 1.3: Edit line 15 — `web-ui` surface from 170 to 169**

Find this line at line 15:

```json
        "web-ui": 170
```

Replace with:

```json
        "web-ui": 169
```

- [ ] **Step 1.4: Edit line 25 — `features.deepReview` sharedKey from 2 to 1**

Find this line at line 25:

```json
        "features.deepReview": 2,
```

Replace with:

```json
        "features.deepReview": 1,
```

- [ ] **Step 1.5: Validate the file is still valid JSON**

Read the entire file back and confirm:
- Top-level keys are still `"version"`, `"description"`, `"budgets"`.
- `budgets.sharedTermDuplicates.maxTotal` is `184`.
- `budgets.sharedTermDuplicates.bySurface["web-ui"]` is `169`.
- `budgets.sharedTermDuplicates.bySharedKey["features.deepReview"]` is `1`.
- All other numbers are unchanged from the read in Step 1.1.
- No trailing commas, no missing commas.

A quick way to validate: from `F:\git-base\BitFun` run
`node -e "JSON.parse(require('fs').readFileSync('scripts/i18n-governance-baseline.json','utf-8'));console.log('ok')"`.
Expected output: `ok`. Exit code: 0.

- [ ] **Step 1.6: Run `pnpm run i18n:audit` and confirm the three `sharedTermDuplicates` errors are gone**

Run from `F:\git-base\BitFun`:

```bash
pnpm run i18n:audit
```

Expected output: 4 errors remaining, all out of scope of this spec:
- 2 errors mentioning `sessionFilesBadge` extra keys (zh-CN and zh-TW).
- 1 error mentioning 46 unknown static Web UI i18n keys.
- 1 error mentioning 3 CJK source candidates.

The three `sharedTermDuplicates` errors from the audit must be **absent** from
the output. The audit may still exit with code 1 (because there are 4 other
errors), but the spec's success criterion is that our three specific errors
are gone.

- [ ] **Step 1.7: Run `pnpm run i18n:contract:test` and confirm it still passes**

Run from `F:\git-base\BitFun`:

```bash
pnpm run i18n:contract:test
```

Expected: PASS, exit code 0. If it fails, the baseline numbers are
inconsistent with what the contract tests expect; STOP and report the failure.
Do not blindly adjust other numbers.

- [ ] **Step 1.8: Commit**

```bash
git add scripts/i18n-governance-baseline.json
git commit -m "chore(i18n): lower sharedTermDuplicates baseline to current state"
```

- [ ] **Step 1.9: Self-review**

Before reporting, confirm:
- `git show HEAD --stat` shows only `scripts/i18n-governance-baseline.json` was touched.
- `git diff HEAD~1 HEAD` shows exactly three lines changed, each a single number.
- No locale resource, no source file, no test file was touched.
- The `description` field is unchanged.

---

## Self-Check

1. **Spec coverage:**
   - 3 numeric edits → Task 1.2, 1.3, 1.4
   - Description unchanged → Task 1.1 read + Task 1.9 review
   - Other budget categories unchanged → Step 1.1 read
   - Verification: `pnpm run i18n:audit` shows the three errors gone → Task 1.6
   - Verification: `pnpm run i18n:contract:test` passes → Task 1.7
   - No new files, no source changes, no locale changes → Task 1.9 self-review

2. **Placeholder scan:** No "TODO" / "TBD" / "similar to step X" markers.

3. **Type consistency:** No types defined or used; this is a pure config edit.
   The audit script reads the JSON via standard `JSON.parse`, so the only
   requirement is valid JSON and a numeric value at the three paths.

No gaps; proceed to execution.
