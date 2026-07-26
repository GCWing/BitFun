import { argon2idAsync } from '@noble/hashes/argon2.js';

interface LoginChallenge {
  kdf_salt: string;
  argon2_params: string;
}

interface KdfParams {
  m: number;
  t: number;
  p: number;
}

interface ErrorBody {
  error?: string;
  retry_after_secs?: number;
}

const form = document.querySelector<HTMLFormElement>('[data-page-login-form]');
const usernameInput = document.querySelector<HTMLInputElement>('[data-page-login-username]');
const passwordInput = document.querySelector<HTMLInputElement>('[data-page-login-password]');
const submitButton = document.querySelector<HTMLButtonElement>('[data-page-login-submit]');
const errorElement = document.querySelector<HTMLElement>('[data-page-login-error]');
const toggleButton = document.querySelector<HTMLButtonElement>('[data-page-login-toggle-password]');
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

function decodeBase64(value: string): Uint8Array {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function encodeBase64(value: Uint8Array): string {
  let binary = '';
  for (let offset = 0; offset < value.length; offset += 0x8000) {
    binary += String.fromCharCode(...value.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
}

function parseKdfParams(raw: string): KdfParams {
  const value = JSON.parse(raw) as Partial<KdfParams>;
  if (!Number.isInteger(value.m)
    || !Number.isInteger(value.t)
    || !Number.isInteger(value.p)
    || value.m! < 8 * 1024
    || value.m! > 256 * 1024
    || value.t! < 1
    || value.t! > 10
    || value.p! < 1
    || value.p! > 16) {
    throw new Error(message('登录参数无效。', 'The sign-in parameters are invalid.'));
  }
  return value as KdfParams;
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
    if (body.error === 'invalid username or password') {
      return message('用户名或密码不正确。', 'Incorrect username or password.');
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

toggleButton?.addEventListener('click', () => {
  if (!passwordInput) return;
  const reveal = passwordInput.type === 'password';
  passwordInput.type = reveal ? 'text' : 'password';
  toggleButton.textContent = reveal
    ? message('隐藏', 'Hide')
    : message('显示', 'Show');
  toggleButton.setAttribute('aria-pressed', String(reveal));
});

form?.addEventListener('submit', async (event) => {
  event.preventDefault();
  if (!usernameInput || !passwordInput || !submitButton) return;

  const username = usernameInput.value.trim();
  const password = passwordInput.value;
  if (!username || username.length > 128 || !password || password.length > 1024) {
    showError(message('请输入有效的用户名和密码。', 'Enter a valid username and password.'));
    return;
  }

  showError('');
  submitButton.disabled = true;
  submitButton.textContent = message('正在验证…', 'Signing in…');

  let passwordBytes: Uint8Array | undefined;
  let passwordHash: Uint8Array | undefined;
  try {
    const challenge = await postJson<LoginChallenge>(
      relayApiPath('/api/auth/login/challenge'),
      { username },
    );
    const salt = decodeBase64(challenge.kdf_salt);
    if (salt.length !== 16) {
      throw new Error(message('登录参数无效。', 'The sign-in parameters are invalid.'));
    }
    const params = parseKdfParams(challenge.argon2_params);
    passwordBytes = new TextEncoder().encode(password);
    passwordInput.value = '';
    passwordHash = await argon2idAsync(passwordBytes, salt, {
      m: params.m,
      t: params.t,
      p: params.p,
      dkLen: 32,
      version: 0x13,
      asyncTick: 16,
    });
    const loginBody: Record<string, unknown> = {
      username,
      password_hash: encodeBase64(passwordHash),
    };
    if (loginState) {
      loginBody.state = loginState;
    } else {
      loginBody.return_to = currentPageReturnPath();
      loginBody.path_prefix = relayPathPrefix();
    }
    const result = await postJson<{ redirect_to: string }>(
      relayApiPath('/api/page-auth/login'),
      loginBody,
    );
    window.location.replace(externalRedirectTarget(result.redirect_to));
  } catch (error) {
    showError(error instanceof Error
      ? error.message
      : message('登录失败，请重试。', 'Sign-in failed. Try again.'));
    passwordInput.focus();
  } finally {
    passwordBytes?.fill(0);
    passwordHash?.fill(0);
    submitButton.disabled = false;
    submitButton.textContent = message('登录并访问', 'Sign in and continue');
  }
});
