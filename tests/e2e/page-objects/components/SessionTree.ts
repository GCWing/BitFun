import { $, $$, browser } from '@wdio/globals';

export interface RenderedSubagentIdentity {
  avatarId: string;
  nameId: string;
  name: string;
  imageSource: string;
}

export class SessionTree {
  private readonly overviewTriggerSelector = '[data-testid="flowchat-header-session-overview"]';
  private readonly overviewPanelSelector = '[data-testid="flowchat-header-session-overview-panel"]';
  private readonly panelSelector = '[data-testid="flowchat-header-session-tree-content"]';
  private readonly rightPanelTriggerSelector = '[data-testid="flowchat-header-right-panel"]';
  private readonly rightPanelSelector = '[data-testid="session-aux-pane"]';

  async openOverview() {
    await browser.waitUntil(async () => {
      try {
        const trigger = await $(this.overviewTriggerSelector);
        if (!(await trigger.isClickable())) return false;
        await trigger.click();
        return true;
      } catch {
        // Workspace/session hydration can replace the header node between lookup
        // and click. Re-query the selector instead of retaining a stale handle.
        return false;
      }
    }, {
      timeout: 15000,
      interval: 200,
      timeoutMsg: 'The session overview trigger did not become clickable',
    });
    await browser.waitUntil(async () => {
      try {
        return await $(this.overviewPanelSelector).isDisplayed();
      } catch {
        return false;
      }
    }, {
      timeout: 15000,
      interval: 200,
      timeoutMsg: 'The session overview panel did not become visible',
    });
    return $(this.overviewPanelSelector);
  }

  async open(): Promise<void> {
    await this.openOverview();
    const panel = await $(this.panelSelector);
    await panel.waitForDisplayed({ timeout: 15000 });
  }

  async closeOverview(): Promise<void> {
    const panel = await $(this.overviewPanelSelector);
    if (!(await panel.isExisting())) return;

    const trigger = await $(this.overviewTriggerSelector);
    await trigger.waitForClickable({ timeout: 5000 });
    await trigger.click();
    await browser.waitUntil(async () => {
      try {
        return !(await $(this.overviewPanelSelector).isExisting());
      } catch {
        return true;
      }
    }, {
      timeout: 5000,
      interval: 100,
      timeoutMsg: 'The session overview panel did not close',
    });
  }

  async setRightPanelOpen(open: boolean): Promise<void> {
    await browser.waitUntil(async () => {
      try {
        return await $(this.rightPanelTriggerSelector).isClickable();
      } catch {
        return false;
      }
    }, {
      timeout: 15000,
      interval: 200,
      timeoutMsg: 'The session right-panel trigger did not become clickable',
    });

    const trigger = await $(this.rightPanelTriggerSelector);
    const isOpen = (await trigger.getAttribute('aria-pressed')) === 'true';
    if (isOpen !== open) {
      await trigger.click();
    }

    await browser.waitUntil(async () => {
      try {
        const currentTrigger = await $(this.rightPanelTriggerSelector);
        const panel = await $(this.rightPanelSelector);
        const isPressed = (await currentTrigger.getAttribute('aria-pressed')) === 'true';
        const isCollapsed = (await panel.getAttribute('class'))
          ?.split(/\s+/)
          .includes('bitfun-session-scene__aux-pane--collapsed') ?? false;
        return isPressed === open && isCollapsed === !open;
      } catch {
        return false;
      }
    }, {
      timeout: 10000,
      interval: 100,
      timeoutMsg: `The session right panel did not become ${open ? 'open' : 'collapsed'}`,
    });
  }

  async getRightPanelControlState(): Promise<{
    open: boolean;
    state: string | null;
    hasOpenIcon: boolean;
    hasCloseIcon: boolean;
  }> {
    const trigger = await $(this.rightPanelTriggerSelector);
    return {
      open: (await trigger.getAttribute('aria-pressed')) === 'true',
      state: await trigger.getAttribute('data-bf-state'),
      hasOpenIcon: await trigger.$('.lucide-panel-right-open').isExisting(),
      hasCloseIcon: await trigger.$('.lucide-panel-right-close').isExisting(),
    };
  }

  async getSubagentIdentities(): Promise<RenderedSubagentIdentity[]> {
    const avatars = await $$(`${this.panelSelector} [data-bf-component="subagent-avatar"]`);
    const identities: RenderedSubagentIdentity[] = [];

    for (const avatar of avatars) {
      const nodeMain = await avatar.$('..');
      const name = await nodeMain.$('[data-bf-part="subagentName"]');
      const image = await avatar.$('img');
      identities.push({
        avatarId: (await avatar.getAttribute('data-bf-avatar-id')) ?? '',
        nameId: (await avatar.getAttribute('data-bf-name-id')) ?? '',
        name: (await name.getText()).trim(),
        imageSource: (await image.getAttribute('src')) ?? '',
      });
    }

    return identities;
  }

  async getBackgroundEmptyStateText(): Promise<string> {
    const emptyState = await $(`${this.overviewPanelSelector} [data-testid="flowchat-header-background-empty"]`);
    await emptyState.waitForDisplayed({ timeout: 15000 });
    return (await emptyState.getText()).trim();
  }

  async waitForPullRequestOverviewState(): Promise<'loaded' | 'unavailable' | 'error'> {
    const section = await $(`${this.overviewPanelSelector} [data-testid="flowchat-header-pull-requests"]`);
    await browser.waitUntil(async () => {
      const state = await section.getAttribute('data-bf-state');
      return state === 'loaded' || state === 'unavailable' || state === 'error';
    }, {
      timeout: 30000,
      interval: 250,
      timeoutMsg: 'The pull request overview did not leave its loading state',
    });
    return await section.getAttribute('data-bf-state') as 'loaded' | 'unavailable' | 'error';
  }

  async hasPullRequestEmptyState(): Promise<boolean> {
    return $(
      `${this.overviewPanelSelector} [data-testid="flowchat-header-pull-requests-empty"]`,
    ).isExisting();
  }

  async getPullRequestUnavailableStateText(): Promise<string> {
    const unavailableState = await $(
      `${this.overviewPanelSelector} [data-testid="flowchat-header-pull-requests-unavailable"]`,
    );
    await unavailableState.waitForDisplayed({ timeout: 15000 });
    return (await unavailableState.getText()).trim();
  }

  async getPullRequestItemCount(): Promise<number> {
    return (await $$(
      `${this.overviewPanelSelector} [data-testid="flowchat-header-pull-request-item"]`,
    )).length;
  }

  async pullRequestItemsHaveDetailIndicators(): Promise<boolean> {
    const items = await $$(
      `${this.overviewPanelSelector} [data-testid="flowchat-header-pull-request-item"]`,
    );
    for (const item of items) {
      if (!(await item.$('.lucide-chevron-right').isExisting())) return false;
    }
    return (await items.length) > 0;
  }
}
