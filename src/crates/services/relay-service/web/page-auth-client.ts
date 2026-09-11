interface ErrorBody {
  error?: string;
  retry_after_secs?: number;
}

const form = document.querySelector<HTMLFormElement>('[data-page-login-form]');
const submitButton = document.querySelector<HTMLButtonElement>('[data-page-login-submit]');
const errorElement = document.querySelector<HTMLElement>('[data-page-login-error]');
const loginState = form?.dataset.pageLoginState;

function relayPathPrefix(): string {
  const authRouteIndex = window.location.pathname.indexOf('/api/page-auth/');
  if (authRouteIndex >= 0) {
    return window.location.pathname.slice(0, authRouteIndex);
  }
  const pageRouteIndex = window.location.pathname.indexOf('/p/');
  if (pageRouteIndex >= 0) {
    return window.location.pathname.slice(0, pageRouteIndex);
  }
  return '';
}

function relayApiPath(path: string): string {
  return `${relayPathPrefix()}${path}`;
}

function currentPageReturnPath(): string {
  const pageRouteIndex = window.location.pathname.indexOf('/p/');
  const pagePath = pageRouteIndex >= 0
    ? window.location.pathname.slice(pageRouteIndex)
    : window.location.pathname;
  return `${pagePath}${window.location.search}`;
}

function externalRedirectTarget(target: string): string {
  if (/^https?:\/\//i.test(target)) return target;
  return `${relayPathPrefix()}${target}`;
}

function isChineseLocale(): boolean {
  return navigator.language.toLowerCase().startsWith('zh');
}

function message(zh: string, en: string): string {
  return isChineseLocale() ? zh : en;
}

function showError(value: string): void {
  if (!errorElement) return;
  errorElement.textContent = value;
  errorElement.hidden = value.length === 0;
}

async function readError(response: Response): Promise<string> {
  const fallback = message('登录失败，请重试。', 'Sign-in failed. Try again.');
  try {
    const body = await response.json() as ErrorBody;
    if (body.retry_after_secs && body.retry_after_secs > 0) {
      return message(
        `尝试次数过多，请在 ${body.retry_after_secs} 秒后重试。`,
        `Too many attempts. Try again in ${body.retry_after_secs} seconds.`,
      );
    }
    if (body.error === 'account does not have access to this Page') {
      return message('该账号没有此页面的访问权限。', 'This account cannot access the Page.');
    }
    return body.error || fallback;
  } catch {
    return fallback;
  }
}

async function postJson<T>(path: string, body: Record<string, unknown>): Promise<T> {
  const response = await fetch(path, {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw new Error(await readError(response));
  }
  return response.json() as Promise<T>;
}

interface AuthStart {
  transactionId: string;
  transactionSecret: string;
  authorizationUrl: string;
  expiresAt: number;
  pollIntervalSeconds: number;
}

form?.addEventListener('submit', async (event) => {
  event.preventDefault();
  if (!submitButton || submitButton.disabled) return;
  // Open synchronously during the click so browser popup protection permits it.
  const popup = window.open('about:blank', '_blank');
  if (!popup) {
    showError(message('请允许登录弹窗后重试。', 'Allow the sign-in popup and try again.'));
    return;
  }
  popup.opener = null;
  showError('');
  submitButton.disabled = true;
  submitButton.textContent = message('等待 GitHub 授权…', 'Waiting for GitHub…');
  try {
    const start = await postJson<AuthStart>(relayApiPath('/api/auth/github/start'), {});
    const authorization = new URL(start.authorizationUrl);
    if (authorization.protocol !== 'https:' || authorization.hostname !== 'github.com') {
      throw new Error(message('登录地址无效。', 'The sign-in URL is invalid.'));
    }
    popup.location.replace(authorization.href);
    let accessToken: string | undefined;
    while (Date.now() < start.expiresAt * 1000) {
      await new Promise((resolve) => setTimeout(resolve, Math.max(1, start.pollIntervalSeconds) * 1000));
      const result = await postJson<{ status: string; tokens?: { accessToken: string } }>(
        relayApiPath('/api/auth/github/poll'),
        { transactionId: start.transactionId, transactionSecret: start.transactionSecret },
      );
      if (result.tokens?.accessToken) {
        accessToken = result.tokens.accessToken;
        break;
      }
      if (result.status !== 'pending') {
        throw new Error(message('授权未完成，请重新登录。', 'Authorization did not complete. Sign in again.'));
      }
    }
    if (!accessToken) throw new Error(message('登录已过期，请重试。', 'Sign-in expired. Try again.'));
    const loginBody: Record<string, unknown> = { access_token: accessToken };
    if (loginState) {
      loginBody.state = loginState;
    } else {
      loginBody.return_to = currentPageReturnPath();
      loginBody.path_prefix = relayPathPrefix();
    }
    const result = await postJson<{ redirect_to: string }>(relayApiPath('/api/page-auth/login'), loginBody);
    window.location.replace(externalRedirectTarget(result.redirect_to));
  } catch (error) {
    showError(error instanceof Error ? error.message : message('登录失败，请重试。', 'Sign-in failed. Try again.'));
  } finally {
    popup.close();
    submitButton.disabled = false;
    submitButton.textContent = message('使用 GitHub 登录', 'Sign in with GitHub');
  }
});
