# Clear CJK Source Candidates — Implementation Plan

> **For AI agents:** Required sub-skill: use `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task by task. Steps use checkbox
> (`- [ ]`) syntax for progress tracking.

**Goal:** Remove the three CJK source candidates reported by `pnpm run i18n:audit` for the
`web-ui-source` budget, by deleting dead code, internationalizing a TTS string, and fixing a
duplicate-`id` bug in a `<Select>` config component.

**Architecture:** Three independent single-file (or two-file, in the case of the i18n task) edits
that each clear one CJK candidate. The TTS welcome message is moved into the `common` bootstrap
namespace as `app.ttsWelcome`, preserving the synchronous-call pattern that `useWelcomeVoice`
needs (it fires inside `useEffect`). The `<反思>` strip rule is dead code and is deleted. The
`VoiceConfig.tsx` change fixes both the CJK candidate and a latent duplicate-`id` bug at the same
time.

**Tech stack:** TypeScript, React, i18next, Web UI locale resource JSON files. No Rust, no
test-framework changes, no schema changes.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `src/web-ui/src/flow_chat/hooks/useTtsPlayback.ts` | modify | Delete the unreachable `<反思>` strip rule on line 36 |
| `src/web-ui/src/flow_chat/hooks/useWelcomeVoice.ts` | modify | Replace the hard-coded `'你好，主人'` literal with `i18nService.t('app.ttsWelcome', { ns: 'common' })` |
| `src/web-ui/src/locales/zh-CN/common.json` | modify | Add `app.ttsWelcome: '你好，主人'` |
| `src/web-ui/src/locales/zh-TW/common.json` | modify | Add `app.ttsWelcome: '你好，主人'` |
| `src/web-ui/src/locales/en-US/common.json` | modify | Add `app.ttsWelcome: 'Hello'` |
| `src/web-ui/src/infrastructure/config/components/VoiceConfig.tsx` | modify | Change `'中文 (简体)'` to `'Chinese (Simplified)'`; collapse the duplicate `id: 'zh-CN'` row |

No new files. No new tests (the existing `I18nService.test.ts` already covers the bootstrap
synchronous-call path that the new `app.ttsWelcome` key depends on).

---

## Task 1: Delete the `<反思>` strip rule

**Files:**
- Modify: `src/web-ui/src/flow_chat/hooks/useTtsPlayback.ts` (line 36)

- [ ] **Step 1.1: Read the current state of the file**

Read `F:\git-base\BitFun\src\web-ui\src\flow_chat\hooks\useTtsPlayback.ts` lines 32-46. Confirm
line 36 is exactly:

```ts
    .replace(/<反思>[\s\S]*?<\/反思>/gi, '')
```

and that the surrounding lines (32-46, the `stripMarkdown` function body) are the other eleven
English strip rules. The exact current state of the surrounding context is documented in the
spec; if anything has shifted (line numbers, additional CJK, etc.), STOP and report.

- [ ] **Step 1.2: Delete line 36**

In `F:\git-base\BitFun\src\web-ui\src\flow_chat\hooks/useTtsPlayback.ts`, delete the line that
reads:

```ts
    .replace(/<反思>[\s\S]*?<\/反思>/gi, '')
```

The line above (line 35) and the line below (line 37) both end with `,` and join cleanly. After
the deletion, line 35 will be followed directly by what was line 37. No other line is modified.

- [ ] **Step 1.3: Run type-check**

Run from `F:\git-base\BitFun`:

```bash
pnpm --dir src/web-ui run type-check
```

Expected: PASS, exit code 0. If it fails, the most likely cause is a trailing comma or stray
whitespace; fix and re-run.

- [ ] **Step 1.4: Run the focused test file for the hook**

Run from `F:\git-base\BitFun`:

```bash
pnpm --dir src/web-ui run test:run -- src/flow_chat/hooks/useTtsPlayback
```

Expected: 0 fail. If a test breaks, the most likely cause is a test that was asserting on the
strip rule list; STOP and report.

- [ ] **Step 1.5: Run the i18n audit and confirm the candidate count drops from 3 to 2**

Run from `F:\git-base\BitFun`:

```bash
pnpm run i18n:audit 2>&1 | grep -E "CJK source candidate|First entries:" | head -20
```

Expected: a single `web-ui-source has 2 CJK source candidate line(s)` line, with first entries
that are now only the two remaining lines (`useWelcomeVoice.ts:60` and `VoiceConfig.tsx:41`).
Audit still exits with code 1 (because the two remaining CJK candidates and the four
pre-existing subsystem-A errors are still present), but the spec's success criterion is
"`<反思>` is no longer in the first-entries list".

- [ ] **Step 1.6: Commit**

```bash
git add src/web-ui/src/flow_chat/hooks/useTtsPlayback.ts
git commit -m "refactor(flowchat): drop unreachable <反思> TTS strip rule"
```

- [ ] **Step 1.7: Self-review**

Before reporting, confirm:

- `git show HEAD --stat` shows only `src/web-ui/src/flow_chat/hooks/useTtsPlayback.ts` was
  touched.
- `git diff HEAD~1 HEAD -- src/web-ui/src/flow_chat/hooks/useTtsPlayback.ts` shows exactly
  one line deleted, zero lines added.
- No other file was touched (no locale resource, no test file, no other source file).
- The audit output is consistent with the expected 2 remaining candidates.

---

## Task 2: Internationalize the TTS welcome message

**Files:**
- Modify: `src/web-ui/src/flow_chat/hooks/useWelcomeVoice.ts` (line 60)
- Modify: `src/web-ui/src/locales/zh-CN/common.json` (add one key under `app`)
- Modify: `src/web-ui/src/locales/zh-TW/common.json` (add one key under `app`)
- Modify: `src/web-ui/src/locales/en-US/common.json` (add one key under `app`)

- [ ] **Step 2.1: Read the current state of all four files**

Read:
- `F:\git-base\BitFun\src\web-ui\src\flow_chat\hooks\useWelcomeVoice.ts` lines 1-15 and
  lines 50-65.
- `F:\git-base\BitFun\src\web-ui\src\locales\zh-CN\common.json` — confirm there is an
  existing `app` object with `welcome`, `version`, `loading` keys (top of file).
- `F:\git-base\BitFun\src\web-ui\src\locales\zh-TW\common.json` — same as above.
- `F:\git-base\BitFun\src\web-ui\src\locales\en-US\common.json` — same as above.

Confirm `useWelcomeVoice.ts` line 60 is exactly:

```ts
        await provider.speak('你好，主人');
```

and the file currently has no `useTranslation` or `i18nService` import. If the file already
imports `i18nService` (because someone else added it between the spec being written and you
running this task), adapt the import line as needed; the call pattern is the same. If the
file has changed in a way that affects the fix, STOP and report.

- [ ] **Step 2.2: Add the `i18nService` import to `useWelcomeVoice.ts`**

In `F:\git-base\BitFun\src\web-ui\src\flow_chat\hooks\useWelcomeVoice.ts`, find the existing
import block (lines 7-11):

```ts
import { useEffect, useRef } from 'react';
import { createTtsProvider } from '@/infrastructure/voice/TtsProviderFactory';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { createLogger } from '@/shared/utils/logger';
```

Add a new import line after the `createLogger` import (or in the appropriate alphabetical
position — match the project's existing import-ordering convention; check a few neighboring
files if uncertain):

```ts
import { i18nService } from '@/infrastructure/i18n';
```

- [ ] **Step 2.3: Replace the hard-coded literal on line 60**

In `F:\git-base\BitFun\src\web-ui\src\flow_chat\hooks\useWelcomeVoice.ts`, find line 60:

```ts
        await provider.speak('你好，主人');
```

Replace with:

```ts
        await provider.speak(i18nService.t('app.ttsWelcome', { ns: 'common' }));
```

- [ ] **Step 2.4: Add `app.ttsWelcome` to `zh-CN/common.json`**

In `F:\git-base\BitFun\src\web-ui\src\locales\zh-CN\common.json`, find the `app` object at the
top of the file (it currently has `version`, `loading`, `welcome`). Add a new key
`"ttsWelcome"` to it. The result should look like:

```json
  "app": {
    "version": "版本",
    "loading": "加载中...",
    "welcome": "欢迎使用 BitFun",
    "ttsWelcome": "你好，主人"
  },
```

If the existing `app` object has a different shape (additional keys, different key order), add
`ttsWelcome` in the same position. Do not reorder existing keys.

- [ ] **Step 2.5: Add `app.ttsWelcome` to `zh-TW/common.json`**

In `F:\git-base\BitFun\src\web-ui\src\locales\zh-TW\common.json`, find the `app` object and add
`ttsWelcome` to it. The result should look like:

```json
  "app": {
    "version": "版本",
    "loading": "載入中...",
    "welcome": "歡迎使用 BitFun",
    "ttsWelcome": "你好，主人"
  },
```

(Adjust the existing `version` / `loading` / `welcome` values to whatever the current
zh-TW file actually contains. Do not change those values; only add `ttsWelcome`.)

- [ ] **Step 2.6: Add `app.ttsWelcome` to `en-US/common.json`**

In `F:\git-base\BitFun\src\web-ui\src\locales\en-US\common.json`, find the `app` object and add
`ttsWelcome` to it. The result should look like:

```json
  "app": {
    "version": "Version",
    "loading": "Loading...",
    "welcome": "Welcome to BitFun",
    "ttsWelcome": "Hello"
  },
```

(Adjust the existing values to whatever the current en-US file actually contains. Do not
change those values; only add `ttsWelcome`.)

- [ ] **Step 2.7: Validate all four JSON files are still valid**

Run from `F:\git-base\BitFun`:

```bash
node -e "['src/web-ui/src/locales/zh-CN/common.json','src/web-ui/src/locales/zh-TW/common.json','src/web-ui/src/locales/en-US/common.json'].forEach(f=>JSON.parse(require('fs').readFileSync(f,'utf-8')));console.log('ok')"
```

Expected: prints `ok`, exit code 0. If any file is invalid, fix and re-run.

- [ ] **Step 2.8: Run type-check**

Run from `F:\git-base\BitFun`:

```bash
pnpm --dir src/web-ui run type-check
```

Expected: PASS, exit code 0. If it fails, the most likely cause is the import path; the
canonical import path is `@/infrastructure/i18n` (re-export surface; verify by reading
`F:\git-base\BitFun\src\web-ui\src\infrastructure\i18n\index.ts` if uncertain).

- [ ] **Step 2.9: Run the i18n contract test and the I18nService test**

Run from `F:\git-base\BitFun`:

```bash
pnpm run i18n:contract:test 2>&1 | tail -20
```

Expected: 31 pass, 6 fail (same 6 pre-existing failures from subsystem A; this task does not
change any of those). The 5 tests that assert `runI18nAudit().status === 0` will still fail
because audit still has 4 pre-existing subsystem-A errors. The 1 test that asserts an
array-namespace fixture will still fail for its own pre-existing reason. No new failures.

Then run:

```bash
pnpm --dir src/web-ui run test:run -- src/infrastructure/i18n/core/I18nService.test.ts
```

Expected: 0 fail.

- [ ] **Step 2.10: Run the i18n audit and confirm the candidate count drops from 2 to 1**

Run from `F:\git-base\BitFun`:

```bash
pnpm run i18n:audit 2>&1 | grep -E "CJK source candidate|First entries:" | head -20
```

Expected: a single `web-ui-source has 1 CJK source candidate line(s)` line, with first entries
listing only `VoiceConfig.tsx:41`. The two previous candidates (`useTtsPlayback.ts:36` and
`useWelcomeVoice.ts:60`) are gone from the list.

- [ ] **Step 2.11: Commit**

```bash
git add src/web-ui/src/flow_chat/hooks/useWelcomeVoice.ts src/web-ui/src/locales/zh-CN/common.json src/web-ui/src/locales/zh-TW/common.json src/web-ui/src/locales/en-US/common.json
git commit -m "feat(flowchat): internationalize TTS welcome message"
```

- [ ] **Step 2.12: Self-review**

Before reporting, confirm:

- `git show HEAD --stat` shows exactly 4 files touched: the hook file and the three locale
  files. No test files, no other source files.
- `git diff HEAD~1 HEAD -- src/web-ui/src/locales` shows 3 separate single-line additions
  (one per locale), each inserting `ttsWelcome` under the `app` object.
- `git diff HEAD~1 HEAD -- src/web-ui/src/flow_chat/hooks/useWelcomeVoice.ts` shows two
  changes: the new import line and the new `provider.speak(...)` call, with the literal
  `'你好，主人'` removed.
- Audit output is consistent with the expected 1 remaining candidate.

---

## Task 3: Fix the VoiceConfig language label and the duplicate-`id` bug

**Files:**
- Modify: `src/web-ui/src/infrastructure/config/components/VoiceConfig.tsx` (lines 40-48)

- [ ] **Step 3.1: Read the current state of the file**

Read `F:\git-base\BitFun\src\web-ui\src\infrastructure\config\components\VoiceConfig.tsx`
lines 40-48. Confirm the exact current state is:

```ts
const LANGUAGES = [
  { id: 'zh-CN', label: '中文 (简体)' },
  { id: 'en-US', label: 'English (US)' },
  { id: 'en-GB', label: 'English (UK)' },
  { id: 'zh-CN', label: 'Chinese Simplified' },
  { id: 'zh-TW', label: 'Chinese Traditional' },
  { id: 'ja-JP', label: 'Japanese' },
  { id: 'ko-KR', label: 'Korean' },
];
```

If anything has shifted (an entry was added/removed, the order changed, etc.), STOP and
report.

- [ ] **Step 3.2: Replace lines 40-48 with the corrected version**

Replace the entire `LANGUAGES` constant in
`F:\git-base\BitFun\src\web-ui\src\infrastructure\config\components\VoiceConfig.tsx` with:

```ts
const LANGUAGES = [
  { id: 'zh-CN', label: 'Chinese (Simplified)' },
  { id: 'en-US', label: 'English (US)' },
  { id: 'en-GB', label: 'English (UK)' },
  { id: 'zh-TW', label: 'Chinese (Traditional)' },
  { id: 'ja-JP', label: 'Japanese' },
  { id: 'ko-KR', label: 'Korean' },
];
```

Two changes:

1. Line 1's `label: '中文 (简体)'` becomes `label: 'Chinese (Simplified)'`.
2. The duplicate `id: 'zh-CN'` row (the one that was line 4) is deleted. The remaining six
   rows are reordered so the list is unique by `id` and the `zh-TW` entry directly follows
   the surviving `zh-CN` entry (preserving the relative order of the rest of the array).

If the array is referenced in any way that depends on its current order, the reorder may
need adjustment. Search the rest of the file for any reference to a `LANGUAGES` element by
index. If you find such a reference, STOP and report.

- [ ] **Step 3.3: Run type-check**

Run from `F:\git-base\BitFun`:

```bash
pnpm --dir src/web-ui run type-check
```

Expected: PASS, exit code 0.

- [ ] **Step 3.4: Run any focused test file for `VoiceConfig`**

Run from `F:\git-base\BitFun`:

```bash
pnpm --dir src/web-ui run test:run -- src/infrastructure/config/components/VoiceConfig
```

Expected: 0 fail. If no test file exists for `VoiceConfig`, skip this step and confirm in
the report that there is no test file to run.

- [ ] **Step 3.5: Run the i18n audit and confirm the `web-ui-source` error is gone**

Run from `F:\git-base\BitFun`:

```bash
pnpm run i18n:audit 2>&1 | grep -E "CJK source candidate|First entries:|Failed with" | head -20
```

Expected: no `CJK source candidate` line. The `Failed with` line should show
`Failed with 3 error(s)` (down from 4 in the previous task). The three remaining errors are
the two `sessionFilesBadge extra keys` errors and the one `46 unknown static Web UI i18n
key(s)` error — all out of scope (subsystem A).

- [ ] **Step 3.6: Commit**

```bash
git add src/web-ui/src/infrastructure/config/components/VoiceConfig.tsx
git commit -m "fix(voice-config): align zh-CN label to English and drop duplicate id"
```

- [ ] **Step 3.7: Self-review**

Before reporting, confirm:

- `git show HEAD --stat` shows only
  `src/web-ui/src/infrastructure/config/components/VoiceConfig.tsx` was touched.
- `git diff HEAD~1 HEAD -- src/web-ui/src/infrastructure/config/components/VoiceConfig.tsx`
  shows the full old `LANGUAGES` constant removed and the new constant added. No other lines
  changed.
- Audit output shows zero `web-ui-source` errors.

---

## Self-Check

1. **Spec coverage:**
   - Delete `<反思>` strip rule → Task 1
   - Internationalize TTS welcome via `i18nService.t('app.ttsWelcome', { ns: 'common' })` →
     Task 2.2 + 2.3
   - Add `app.ttsWelcome` to zh-CN, zh-TW, en-US → Task 2.4 + 2.5 + 2.6
   - Change `'中文 (简体)'` to `'Chinese (Simplified)'` → Task 3.2
   - Collapse the duplicate `id: 'zh-CN'` row → Task 3.2
   - Verification: audit no longer reports `web-ui-source` → Task 1.5, 2.10, 3.5
   - Verification: type-check passes → Task 1.3, 2.8, 3.3
   - Verification: existing tests still pass → Task 1.4, 2.9, 3.4

2. **Placeholder scan:** No "TODO" / "TBD" / "similar to step X" markers. Each step has either
   exact code to write, an exact command to run, or an exact commit message.

3. **Type consistency:** The new `i18nService.t('app.ttsWelcome', { ns: 'common' })` call
   matches the i18next signature used elsewhere in the codebase. The `LANGUAGES` constant's
   inferred type stays the same (a `const`-asserted array of `{ id: string; label: string }`).

No gaps; proceed to execution.
