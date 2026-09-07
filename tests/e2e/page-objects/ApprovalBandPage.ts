import { $, browser, expect } from '@wdio/globals';

export class ApprovalBandPage {
  get band() { return $('[data-testid="chat-input-approval-band"]'); }
  get allow() { return $('[data-testid="chat-input-approval-allow"]'); }
  get reject() { return $('[data-testid="chat-input-approval-reject"]'); }
  get resource() { return $('.openbitfun-chat-input-approval__resource'); }

  async assertContained() {
    await this.band.waitForDisplayed({ timeout: 45000 });
    const layout = await browser.execute(() => {
      const band = document.querySelector<HTMLElement>('[data-testid="chat-input-approval-band"]')!;
      const resource = band.querySelector<HTMLElement>('.openbitfun-chat-input-approval__resource')!;
      const container = band.parentElement!;
      const rect = band.getBoundingClientRect();
      const parent = container.getBoundingClientRect();
      const buttons = ['allow', 'reject'].map(action => {
        const button = band.querySelector<HTMLElement>(`[data-testid="chat-input-approval-${action}"]`)!;
        const box = button.getBoundingClientRect();
        const hit = document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2);
        return {
          action, rect: { x: box.x, y: box.y, width: box.width, height: box.height }, visible: box.left >= 0 && box.right <= innerWidth && box.top >= 0 && box.bottom <= innerHeight,
          hit: !!hit && button.contains(hit),
          outsideResource: !resource.contains(button),
        };
      });
      return {
        viewportWidth: innerWidth,
        bounded: rect.left >= parent.left - 1 && rect.right <= parent.right + 1,
        noHorizontalOverflow: document.documentElement.scrollWidth <= innerWidth + 1,
        scrollable: resource.scrollHeight > resource.clientHeight,
        resourceHeight: resource.clientHeight,
        viewportHeight: innerHeight,
        buttons,
      };
    });
    expect(layout.bounded).toBe(true);
    expect(layout.noHorizontalOverflow).toBe(true);
    expect(layout.scrollable).toBe(true);
    expect(layout.resourceHeight).toBeLessThanOrEqual(layout.viewportHeight * .25 + 32);
    for (const button of layout.buttons) {
      expect(button.visible).toBe(true);
      expect(button.hit).toBe(true);
      expect(button.outsideResource).toBe(true);
    }
    return layout;
  }
}
