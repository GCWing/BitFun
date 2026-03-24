import { browser, expect } from '@wdio/globals';

const DRIVER_HOST = '127.0.0.1';
const DRIVER_PORT = Number(process.env.BITFUN_E2E_WEBDRIVER_PORT || 4445);
const ELEMENT_KEY = 'element-6066-11e4-a52e-4f735466cecf';
const SHADOW_KEY = 'shadow-6066-11e4-a52e-4f735466cecf';

type DriverResponse<T> = {
  value: T;
};

async function driverRequest<T>(path: string, init?: RequestInit): Promise<DriverResponse<T>> {
  const response = await fetch(`http://${DRIVER_HOST}:${DRIVER_PORT}${path}`, {
    headers: {
      'content-type': 'application/json',
      ...(init?.headers ?? {}),
    },
    ...init,
  });

  const payload = await response.json() as DriverResponse<T>;
  if (!response.ok) {
    throw new Error(`WebDriver request failed: ${response.status} ${JSON.stringify(payload)}`);
  }
  return payload;
}

function pngDimensions(base64: string): { width: number; height: number } {
  const buffer = Buffer.from(base64, 'base64');
  const signature = buffer.subarray(0, 8).toString('hex');
  expect(signature).toBe('89504e470d0a1a0a');
  return {
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
  };
}

describe('L0 Embedded WebDriver Protocol', () => {
  it('supports alert lifecycle endpoints', async () => {
    const sessionId = browser.sessionId;
    expect(sessionId).toBeDefined();

    await driverRequest<null>(`/session/${sessionId}/execute/sync`, {
      method: 'POST',
      body: JSON.stringify({
        script: '() => { alert("embedded-alert"); return null; }',
        args: [],
      }),
    });

    const text = await driverRequest<string>(`/session/${sessionId}/alert/text`);
    expect(text.value).toBe('embedded-alert');

    await driverRequest<null>(`/session/${sessionId}/alert/accept`, {
      method: 'POST',
      body: '{}',
    });

    const dismissed = await fetch(`http://${DRIVER_HOST}:${DRIVER_PORT}/session/${sessionId}/alert/text`);
    expect(dismissed.status).toBe(404);
  });

  it('supports shadow root lookup endpoints', async () => {
    await browser.execute(() => {
      document.getElementById('wd-shadow-host')?.remove();
      const host = document.createElement('div');
      host.id = 'wd-shadow-host';
      const shadow = host.attachShadow({ mode: 'open' });
      const button = document.createElement('button');
      button.className = 'shadow-btn';
      button.textContent = 'shadow-ok';
      shadow.appendChild(button);
      document.body.appendChild(host);
    });

    const sessionId = browser.sessionId;
    expect(sessionId).toBeDefined();

    const hostElement = await driverRequest<Record<string, string>>(`/session/${sessionId}/element`, {
      method: 'POST',
      body: JSON.stringify({
        using: 'css selector',
        value: '#wd-shadow-host',
      }),
    });

    const hostId = hostElement.value[ELEMENT_KEY];
    expect(hostId).toBeDefined();

    const shadowRoot = await driverRequest<Record<string, string>>(
      `/session/${sessionId}/element/${hostId}/shadow`,
    );
    const shadowId = shadowRoot.value[SHADOW_KEY];
    expect(shadowId).toBeDefined();

    const shadowElement = await driverRequest<Record<string, string>>(
      `/session/${sessionId}/shadow/${shadowId}/element`,
      {
        method: 'POST',
        body: JSON.stringify({
          using: 'css selector',
          value: '.shadow-btn',
        }),
      },
    );

    expect(shadowElement.value[ELEMENT_KEY]).toBeDefined();
  });

  it('supports extended locator strategies', async () => {
    await browser.execute(() => {
      document.getElementById('wd-locator-host')?.remove();
      const host = document.createElement('div');
      host.id = 'wd-locator-host';

      const byId = document.createElement('div');
      byId.id = 'wd-by-id';

      const byName = document.createElement('input');
      byName.setAttribute('name', 'wd-by-name');

      const byClass = document.createElement('div');
      byClass.className = 'wd-by-class';

      host.append(byId, byName, byClass);
      document.body.appendChild(host);
    });

    const sessionId = browser.sessionId;

    const byId = await driverRequest<Record<string, string>>(`/session/${sessionId}/element`, {
      method: 'POST',
      body: JSON.stringify({ using: 'id', value: 'wd-by-id' }),
    });
    expect(byId.value[ELEMENT_KEY]).toBeDefined();

    const byName = await driverRequest<Record<string, string>>(`/session/${sessionId}/element`, {
      method: 'POST',
      body: JSON.stringify({ using: 'name', value: 'wd-by-name' }),
    });
    expect(byName.value[ELEMENT_KEY]).toBeDefined();

    const byClass = await driverRequest<Record<string, string>>(`/session/${sessionId}/element`, {
      method: 'POST',
      body: JSON.stringify({ using: 'class name', value: 'wd-by-class' }),
    });
    expect(byClass.value[ELEMENT_KEY]).toBeDefined();
  });

  it('uses the native cookie store endpoints', async () => {
    const sessionId = browser.sessionId;
    const cookieName = `wd-cookie-${Date.now()}`;

    await driverRequest<null>(`/session/${sessionId}/cookie`, {
      method: 'POST',
      body: JSON.stringify({
        cookie: {
          name: cookieName,
          value: 'cookie-value',
          path: '/',
          sameSite: 'Lax',
        },
      }),
    });

    const cookie = await driverRequest<{
      name: string;
      value: string;
      path?: string;
      sameSite?: string;
    }>(`/session/${sessionId}/cookie/${cookieName}`);
    expect(cookie.value.name).toBe(cookieName);
    expect(cookie.value.value).toBe('cookie-value');
    expect(cookie.value.path).toBe('/');
    expect(cookie.value.sameSite).toBe('Lax');

    const cookies = await driverRequest<Array<{ name: string }>>(`/session/${sessionId}/cookie`);
    expect(cookies.value.some((item) => item.name === cookieName)).toBe(true);

    await driverRequest<null>(`/session/${sessionId}/cookie/${cookieName}`, {
      method: 'DELETE',
    });

    const deleted = await fetch(`http://${DRIVER_HOST}:${DRIVER_PORT}/session/${sessionId}/cookie/${cookieName}`);
    expect(deleted.status).toBe(404);
  });

  it('appends text when using the element value endpoint', async () => {
    await browser.execute(() => {
      let input = document.getElementById('wd-send-keys-input') as HTMLInputElement | null;
      if (!input) {
        input = document.createElement('input');
        input.id = 'wd-send-keys-input';
        input.style.position = 'fixed';
        input.style.left = '24px';
        input.style.top = '144px';
        document.body.appendChild(input);
      }
      input.value = 'foo';
      input.focus();
      input.setSelectionRange(input.value.length, input.value.length);
    });

    const sessionId = browser.sessionId;
    const input = await driverRequest<Record<string, string>>(`/session/${sessionId}/element`, {
      method: 'POST',
      body: JSON.stringify({
        using: 'id',
        value: 'wd-send-keys-input',
      }),
    });
    const inputId = input.value[ELEMENT_KEY];

    await driverRequest<null>(`/session/${sessionId}/element/${inputId}/value`, {
      method: 'POST',
      body: JSON.stringify({
        text: 'bar',
      }),
    });

    await driverRequest<null>(`/session/${sessionId}/element/${inputId}/value`, {
      method: 'POST',
      body: JSON.stringify({
        value: ['!'],
      }),
    });

    const finalValue = await browser.execute(() => {
      const input = document.getElementById('wd-send-keys-input') as HTMLInputElement | null;
      return input?.value ?? '';
    });
    expect(finalValue).toBe('foobar!');
  });

  it('returns cropped element screenshots', async () => {
    const cssWidth = 96;
    const cssHeight = 48;

    await browser.execute(({ width, height }) => {
      document.getElementById('wd-screenshot-box')?.remove();
      const box = document.createElement('div');
      box.id = 'wd-screenshot-box';
      box.style.position = 'fixed';
      box.style.left = '24px';
      box.style.top = '24px';
      box.style.width = `${width}px`;
      box.style.height = `${height}px`;
      box.style.background = 'rgb(12, 112, 248)';
      box.style.zIndex = '2147483647';
      document.body.appendChild(box);
    }, { width: cssWidth, height: cssHeight });

    const sessionId = browser.sessionId;
    const dpr = await browser.execute(() => window.devicePixelRatio || 1);
    const box = await driverRequest<Record<string, string>>(`/session/${sessionId}/element`, {
      method: 'POST',
      body: JSON.stringify({ using: 'id', value: 'wd-screenshot-box' }),
    });
    const boxId = box.value[ELEMENT_KEY];

    const screenshot = await driverRequest<string>(
      `/session/${sessionId}/element/${boxId}/screenshot`,
    );
    const { width, height } = pngDimensions(screenshot.value);

    expect(width).toBeGreaterThanOrEqual(Math.floor(cssWidth * dpr) - 2);
    expect(width).toBeLessThanOrEqual(Math.ceil(cssWidth * dpr) + 2);
    expect(height).toBeGreaterThanOrEqual(Math.floor(cssHeight * dpr) - 2);
    expect(height).toBeLessThanOrEqual(Math.ceil(cssHeight * dpr) + 2);
  });

  it('supports print endpoint when the platform exposes it', async function () {
    const capabilities = browser.capabilities as Record<string, unknown>;
    if (!capabilities.printPage) {
      this.skip();
    }

    const sessionId = browser.sessionId;
    const pdf = await driverRequest<string>(`/session/${sessionId}/print`, {
      method: 'POST',
      body: JSON.stringify({
        orientation: 'portrait',
        marginTop: 1,
        marginBottom: 1,
        marginLeft: 1,
        marginRight: 1,
      }),
    });

    const buffer = Buffer.from(pdf.value, 'base64');
    expect(buffer.subarray(0, 4).toString('ascii')).toBe('%PDF');
  });

  it('supports wheel actions for viewport scrolling', async () => {
    await browser.execute(() => {
      window.scrollTo(0, 0);
      document.body.style.minHeight = '4000px';
      let marker = document.getElementById('wd-wheel-marker');
      if (!marker) {
        marker = document.createElement('div');
        marker.id = 'wd-wheel-marker';
        marker.style.position = 'absolute';
        marker.style.top = '3200px';
        marker.style.left = '24px';
        marker.textContent = 'wheel-marker';
        document.body.appendChild(marker);
      }
    });

    const sessionId = browser.sessionId;
    await driverRequest<null>(`/session/${sessionId}/actions`, {
      method: 'POST',
      body: JSON.stringify({
        actions: [
          {
            type: 'wheel',
            id: 'wheel',
            actions: [
              { type: 'scroll', x: 120, y: 120, deltaX: 0, deltaY: 600 },
            ],
          },
        ],
      }),
    });

    await browser.pause(100);
    const scrollY = await browser.execute(() => window.scrollY);
    expect(scrollY).toBeGreaterThan(0);
  });

  it('releases pressed keys when DELETE /actions is called', async () => {
    await browser.execute(() => {
      type ActionEventLog = Array<{ type: string; key: string }>;
      const wdWindow = window as typeof window & { __wdActionEvents: ActionEventLog };
      wdWindow.__wdActionEvents = [];
      let input = document.getElementById('wd-release-input') as HTMLInputElement | null;
      if (!input) {
        input = document.createElement('input');
        input.id = 'wd-release-input';
        input.style.position = 'fixed';
        input.style.left = '24px';
        input.style.top = '96px';
        document.body.appendChild(input);
      }
      input.value = '';
      input.focus();

      input.onkeydown = (event) => {
        wdWindow.__wdActionEvents.push({ type: 'keydown', key: event.key });
      };
      input.onkeyup = (event) => {
        wdWindow.__wdActionEvents.push({ type: 'keyup', key: event.key });
      };
    });

    const sessionId = browser.sessionId;
    await driverRequest<null>(`/session/${sessionId}/actions`, {
      method: 'POST',
      body: JSON.stringify({
        actions: [
          {
            type: 'key',
            id: 'keyboard',
            actions: [
              { type: 'keyDown', value: '\uE008' },
            ],
          },
        ],
      }),
    });

    await driverRequest<null>(`/session/${sessionId}/actions`, {
      method: 'DELETE',
      body: '{}',
    });

    const events = await browser.execute(() => {
      const wdWindow = window as typeof window & {
        __wdActionEvents: Array<{ type: string; key: string }>;
      };
      return wdWindow.__wdActionEvents;
    });

    expect(events.some((event) => event.type === 'keydown' && event.key === 'Shift')).toBe(true);
    expect(events.some((event) => event.type === 'keyup' && event.key === 'Shift')).toBe(true);
  });
});
