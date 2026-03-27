const LAST_DAILY_PROMPT_DATE_KEY = 'bitfun:update:lastDailyPromptDate';
const LAST_PROMPTED_LATEST_KEY = 'bitfun:update:lastPromptedLatestVersion';
const SKIPPED_VERSION_KEY = 'bitfun:update:skippedVersion';

function todayLocalDateString(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

export function shouldShowDailyUpdatePrompt(latestVersion: string): boolean {
  try {
    const skipped = localStorage.getItem(SKIPPED_VERSION_KEY);
    if (skipped && skipped === latestVersion) {
      return false;
    }
    const today = todayLocalDateString();
    const lastDate = localStorage.getItem(LAST_DAILY_PROMPT_DATE_KEY);
    const lastPromptedLatest = localStorage.getItem(LAST_PROMPTED_LATEST_KEY);
    if (lastDate === today && lastPromptedLatest === latestVersion) {
      return false;
    }
    return true;
  } catch {
    return true;
  }
}

/** After showing the daily prompt (Later / Install / Skip) for this remote version. */
export function recordDailyPromptDismissed(latestVersion: string): void {
  try {
    localStorage.setItem(LAST_DAILY_PROMPT_DATE_KEY, todayLocalDateString());
    localStorage.setItem(LAST_PROMPTED_LATEST_KEY, latestVersion);
  } catch {
    /* ignore */
  }
}

export function recordSkipThisVersion(latestVersion: string): void {
  try {
    localStorage.setItem(SKIPPED_VERSION_KEY, latestVersion);
    recordDailyPromptDismissed(latestVersion);
  } catch {
    /* ignore */
  }
}
