# Spec: Clear Pre-existing CJK Source Candidates in Web UI

## Context

`pnpm run i18n:audit` on `dev` reports a pre-existing
`web-ui-source` budget error:

```
[i18n:audit] ERROR web-ui-source has 3 CJK source candidate line(s), budget is 0.
First entries:
  src/web-ui/src/flow_chat/hooks/useTtsPlayback.ts:36
  src/web-ui/src/flow_chat/hooks/useWelcomeVoice.ts:60
  src/web-ui/src/infrastructure/config/components/VoiceConfig.tsx:41
```

`scripts/i18n-hardcoded-baseline.json:6` declares
`web-ui-source.maxCjkLines: 0` because `AGENTS.md` says the Web UI source
must not contain CJK (CJK in product UI text is a locale-strings debt; CJK
in logs is forbidden by `src/web-ui/LOGGING.md`). The audit script
(`scripts/i18n-audit.mjs:2300-2304`) reports an error whenever the actual
count exceeds the budget. With three real candidates and a budget of zero,
the only correct response is to remove the three lines.

The three candidates are qualitatively different and require three
different kinds of fix:

1. **`<反思>` strip rule in `useTtsPlayback.ts:36`** — defensive dead
   code. A full-repo grep for `<反思>` matches only this one line. Git
   history (`git log -S "反思"`) shows the string was introduced in
   commit `c2f205e3` ("添加语音播报功能") and never used anywhere else.
   The eleven other strip rules in the same `stripMarkdown` function
   cover English-only model-output conventions (`<scratchpad>`,
   `<thinking>`, `<search_plan>`, `<execution_summary>`, `<file_plan>`,
   `<viewplan>`, `<CodeOutputBlock>`, etc.); `<反思>` is the lone
   non-English entry. There is no model prompt or template that emits
   `<反思>`, so this strip rule can never match anything in production.
   The cleanest fix is to delete the line.

2. **TTS welcome message in `useWelcomeVoice.ts:60`** — user-facing
   audio. The string `'你好，主人'` is spoken by the TTS engine when the
   main window opens. This is a real product string (not a debug log,
   not a defensive strip rule), so it should be internationalized
   through the standard i18n path rather than deleted or grandfathered.
   The component already knows the user's TTS language
   (`cfg.language`); the TTS engine itself is configured to that
   language. Localizing the spoken message through the i18n service
   aligns the audio with the engine.

3. **Voice-language label in `VoiceConfig.tsx:41`** — UI string in a
   `<Select>` language picker. The label `'中文 (简体)'` is hard-coded
   in a `LANGUAGES` constant. The other six labels in the same array
   are all in English; the only CJK entry is the one for `zh-CN`. The
   simplest correct fix is to use the English label `'Chinese
   (Simplified)'` (matching the duplicate entry on line 44) and
   collapse the duplicate `id: 'zh-CN'` rows into a single row (the
   array currently has two entries with `id: 'zh-CN'`, which is a
   latent bug — React would warn and the second value would shadow the
   first). This fix also addresses a bug independent of the CJK
   question.

## Goal

Remove all three CJK source candidates so that
`pnpm run i18n:audit` no longer reports the `web-ui-source` error, while:

- Not regressing TTS playback or startup behavior.
- Not changing the visible UI text in any locale other than the one
  already-broken case in `VoiceConfig.tsx` (and even that change
  preserves the user's existing understanding: every other entry in
  the picker is already shown in English, so the user already sees
  English in this control for `en-US` / `en-GB` / `ja-JP` / `ko-KR`).
- Not introducing new shared-term or locale-format debt.
- Not changing the i18n governance baseline (`maxCjkLines: 0` stays
  at zero; we are clearing debt, not moving it into a grandfathered
  bucket).

## Design

### Files changed

| File | Change |
|---|---|
| `src/web-ui/src/flow_chat/hooks/useTtsPlayback.ts` | Delete line 36 (the `<反思>` strip rule) |
| `src/web-ui/src/flow_chat/hooks/useWelcomeVoice.ts` | Replace the hard-coded `'你好，主人'` literal with a call to `i18nService.t('app.ttsWelcome', { ns: 'common' })` |
| `src/web-ui/src/locales/zh-CN/common.json` | Add `app.ttsWelcome: '你好，主人'` |
| `src/web-ui/src/locales/zh-TW/common.json` | Add `app.ttsWelcome: '你好，主人'` |
| `src/web-ui/src/locales/en-US/common.json` | Add `app.ttsWelcome: 'Hello'` |
| `src/web-ui/src/infrastructure/config/components/VoiceConfig.tsx` | Change `'中文 (简体)'` to `'Chinese (Simplified)'`; collapse the duplicate `id: 'zh-CN'` row so each id appears exactly once |

No new files. No new namespaces. No test files added or modified (the
existing `I18nService.test.ts` already covers the bootstrap
synchronous-call path; the change is a single new key in the existing
`common` namespace, which is loaded synchronously at startup).

### Key design decisions

#### Why `app.ttsWelcome` and not a new namespace

- The `common` namespace is already in the bootstrap set
  (`scripts/i18n-audit.mjs:45`), so `i18nService.t('app.ttsWelcome',
  { ns: 'common' })` is synchronous — necessary because
  `useWelcomeVoice` fires inside `useEffect` and we do not want to add
  an async wait before speaking the welcome message.
- The existing `app.welcome: '欢迎使用 BitFun'` key (in all three
  locales) is already a UI welcome string; placing the audio welcome
  string next to it under `app.ttsWelcome` keeps the two related
  strings adjacent and signals that they are both "welcome" copy
  (audio vs. visual).
- The alternative — a new `settings/voice` namespace — would require
  either (a) adding it to the bootstrap set, growing the startup
  resource bundle, or (b) an async `loadNamespace(...)` call inside
  `useWelcomeVoice`, complicating the speaking flow.

#### Why delete the `<反思>` strip rule rather than grandfather it

- The audit script already supports a grandfather mechanism
  (`maxCjkLines: 1` would report 3 as a warning, not an error), but
  using it for a defensive dead-code line would be mis-using the
  grandfather bucket: that bucket is documented as "stale strings
  waiting to be moved to owned locale resources", not "defensive
  regex that matches nothing".
- A full-repo grep confirms the line is unreachable: no prompt or
  template emits `<反思>`, no test references it, no documentation
  mentions it. Removing it is strictly safer than keeping it (if a
  future change ever did start emitting `<反思>`, the failure mode
  would be "TTS reads it aloud", which is the same failure mode as
  if the strip rule were silently missing for any other reason — and
  every other Chinese AI prompt convention is covered by either an
  English equivalent or the generic `<[^>]*think[^>]*>` /
  `<[^>]*reasoning[^>]*>` / `<[^>]*thought[^>]*>` catch-alls already
  present in the same function).

#### Why change `'中文 (简体)'` to `'Chinese (Simplified)'` and not i18n-ize the whole `LANGUAGES` array

- The `LANGUAGES` array is referenced from a `<Select>` whose other
  six entries are all already in English. The only CJK entry is the
  outlier. Changing it to English brings it in line with the rest
  of the array; users who currently see `'中文 (简体)'` will see
  `'Chinese (Simplified)'` instead, which is a one-line visual
  change in a single control.
- The alternative (full i18n-ization of `LANGUAGES`, `EDGE_TTS_VOICES`,
  and the surrounding settings UI) would require creating a
  `settings/voice` namespace, moving 15+ strings into three locale
  files, and changing the JSX to call `useTranslation` for each
  label. That is a much larger refactor than the CJK-cleanup
  question warrants, and it pulls in scope (a new namespace, a
  translation PR, label consistency across three locales) that is
  best handled in its own spec. This spec keeps the change to the
  single CJK line.
- The duplicate `id: 'zh-CN'` row (lines 41 and 44 in the current
  file) is a latent React warning and a logic bug: a `<Select>` with
  two options having the same key would log a warning at runtime and
  the second option would shadow the first. Collapsing the duplicate
  fixes that bug at the same time, at zero cost.

### What stays the same

- The `web-ui-source` budget in `scripts/i18n-hardcoded-baseline.json`
  stays at `0`. We are not grandfathering any of the three lines.
- The i18n governance baseline numbers from the prior spec (subsystem
  C) stay unchanged.
- `EDGE_TTS_VOICES` (8 entries, all English) is unchanged.
- All other components and locale keys are unchanged.
- The `app.welcome` UI string is unchanged; we are adding
  `app.ttsWelcome` as a separate, audio-specific key.

## Verification

1. `pnpm run i18n:audit` should report one fewer error (the
   `web-ui-source` line is gone). The remaining 3 pre-existing
   errors (sessionFilesBadge × 2, 46 unknown static Web UI i18n
   keys) are out of scope and remain.
2. `pnpm --dir src/web-ui run type-check` should pass.
3. `pnpm --dir src/web-ui run test:run src/infrastructure/i18n/core/I18nService.test.ts`
   should pass (covers the synchronous bootstrap-translation path
   that the new `app.ttsWelcome` key depends on).
4. Manual smoke (not part of CI):
   - Set the app UI to `en-US` and reopen the main window: the TTS
     engine should speak `'Hello'`.
   - Set the app UI to `zh-CN` and reopen the main window: the TTS
     engine should speak `'你好，主人'`.
   - Open the Settings → Voice page: the language picker should
     show `'Chinese (Simplified)'` for the `zh-CN` option, with no
     duplicate `zh-CN` entry in the picker.

## Out of scope (future work)

- Full i18n-ization of `LANGUAGES` and `EDGE_TTS_VOICES` and the
  rest of `VoiceConfig.tsx` (would be a separate spec, would
  introduce a `settings/voice` namespace, and would require
  translations across all three locales).
- The 2 `sessionFilesBadge extra keys` audit errors and the
  `46 unknown static Web UI i18n keys` error — these are subsystem A
  work and will be addressed in a separate spec.

After this spec is applied, audit will go from 4 errors to 3. Only
subsystem A will remain.

## Risk

Low.

- The `<反思>` strip-rule removal is a deletion of dead code. The
  worst-case behavior change is that a future prompt that emits
  `<反思>` would be read aloud by TTS, which is the same behavior
  the system would have if the file were ever corrupted, and which
  is acceptable given that no such prompt exists in the repo.
- The TTS welcome i18n-ization preserves the audio for `zh-CN`
  users verbatim. For `en-US` and `zh-TW` users, the audio changes
  from the current always-Chinese (driven by the
  hard-coded `'你好，主人'`) to language-appropriate greetings. This
  is a small behavior change, but it is the intended fix.
- The `VoiceConfig.tsx` label change is a one-line visible change
  in a single control and applies in all locales. It is consistent
  with the rest of the array.
- The `id: 'zh-CN'` duplicate removal is a bug fix; the visible
  effect is that the `<Select>` no longer logs a React duplicate-key
  warning and the two duplicate `zh-CN` rows collapse into one.

No user data is at risk. No persistence, network, or auth changes
are involved.
