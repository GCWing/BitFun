// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { RemotePairingCard } from './RemotePairingCard';

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

function renderCard(props: Partial<React.ComponentProps<typeof RemotePairingCard>> = {}) {
  const container = document.createElement('div');
  container.innerHTML = renderToStaticMarkup(
    <RemotePairingCard owner="bot" copied={false} onCopyUrl={() => {}} {...props} />,
  );
  return container;
}

describe('RemotePairingCard pending states', () => {
  it.each(['bot', 'network'] as const)('renders one waiting status for a %s pairing code', owner => {
    const view = renderCard({ owner, pairingCode: '123456' });
    expect(view.querySelectorAll('[role="status"]')).toHaveLength(1);
    expect(view.querySelectorAll('[data-openbitfun-component="status-pill"]')).toHaveLength(1);
    expect(view.querySelector('.openbitfun-remote-connect__pairing-code')?.textContent).toBe('123456');
    expect(view.textContent?.match(/remoteConnect.stateWaiting(?:Bot)?/g)).toHaveLength(1);
  });

  it.each(['bot', 'network'] as const)('restores %s waiting state without instructions for a missing code', owner => {
    const view = renderCard({ owner });
    expect(view.querySelectorAll('[role="status"]')).toHaveLength(1);
    expect(view.querySelector('.openbitfun-remote-connect__pairing-visual')).toBeNull();
    expect(view.textContent).not.toContain('remoteConnect.botHint');
    expect(view.textContent).not.toContain('remoteConnect.scanHint');
  });

  it('retains the QR link, copy controls and instructions with a single status', () => {
    const view = renderCard({ owner: 'network', qrUrl: 'https://example.test/pair' });
    expect(view.querySelector('svg')).not.toBeNull();
    expect(view.textContent).toContain('https://example.test/pair');
    expect(view.textContent).toContain('remoteConnect.scanHint');
    expect(view.querySelectorAll('[role="status"]')).toHaveLength(1);
    expect(view.querySelectorAll('button[aria-label="remoteConnect.copyUrl"]')).toHaveLength(2);
  });

  it('shows copy feedback only while a URL is present', () => {
    const copied = renderCard({ qrUrl: 'https://example.test/pair', copied: true });
    expect(copied.querySelector('[role="status"]')?.textContent).toBe('remoteConnect.urlCopied');
    expect(copied.textContent).not.toContain('remoteConnect.stateWaiting');
    const codeOnly = renderCard({ pairingCode: '123456', copied: true });
    expect(codeOnly.querySelector('[role="status"]')?.textContent).toBe('remoteConnect.stateWaitingBot');
  });

  it('keeps one status when a provider returns both a URL and a pairing code', () => {
    const view = renderCard({ pairingCode: '123456', qrUrl: 'https://example.test/pair' });
    expect(view.querySelector('.openbitfun-remote-connect__pairing-code')).not.toBeNull();
    expect(view.querySelectorAll('[role="status"]')).toHaveLength(1);
  });

  it('delegates both QR and copy-button clicks without initiating a connection', async () => {
    const onCopyUrl = vi.fn();
    const container = document.createElement('div');
    const root = createRoot(container);
    try {
      await act(async () => {
        root.render(<RemotePairingCard owner="network" copied={false} qrUrl="https://example.test/pair" onCopyUrl={onCopyUrl} />);
      });
      for (const button of container.querySelectorAll('button')) {
        await act(async () => { button.click(); });
      }
      expect(onCopyUrl).toHaveBeenCalledTimes(2);
    } finally {
      await act(async () => root.unmount());
    }
  });
});
