export const GITHUB_STAR_CTA_DISMISSED_KEY = 'bitfun:nav:github-star-cta:dismissed:v1';

/** GitHub renders the Star control on the repository page itself — no dedicated star route exists. */
export const GITHUB_STAR_URL = 'https://github.com/GCWing/BitFun';

export const isGithubStarCtaDismissed = (): boolean => {
  try {
    return localStorage.getItem(GITHUB_STAR_CTA_DISMISSED_KEY) === 'true';
  } catch {
    return false;
  }
};

export const setGithubStarCtaDismissed = (): void => {
  try {
    localStorage.setItem(GITHUB_STAR_CTA_DISMISSED_KEY, 'true');
  } catch {
    // Ignore storage failures — the button still stays hidden for this session.
  }
};
