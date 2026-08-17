import { $, $$ } from '@wdio/globals';

export interface RenderedSubagentIdentity {
  avatarId: string;
  nameId: string;
  name: string;
  imageSource: string;
}

export class SessionTree {
  private readonly triggerSelector = '[data-testid="flowchat-header-session-tree"]';
  private readonly panelSelector = '[data-bf-part="sessionTreePanel"]';

  async open(): Promise<void> {
    const trigger = await $(this.triggerSelector);
    await trigger.waitForClickable({ timeout: 15000 });
    await trigger.click();
    const panel = await $(this.panelSelector);
    await panel.waitForDisplayed({ timeout: 15000 });
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
}
