import { GeneralChatConfigSnapshot, GeneralChatConfigStore } from './GeneralChatConfigStore';
import { GeneralChatCommandController } from './GeneralChatCommandController';
import { GeneralChatDraftLifecycleController } from './GeneralChatDraftLifecycleController';
import { ConnectionErrorPolicy } from '../ConnectionErrorPolicy';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteLogger } from '../RemoteLogger';

export interface GeneralChatBootstrapCallbacks {
  onConfigRestored: (snapshot: GeneralChatConfigSnapshot) => void;
  onHomeDraftRestored: (text: string) => void;
  onStatusText: (statusText: string) => void;
}

export class GeneralChatBootstrapController {
  private readonly configStore: GeneralChatConfigStore;
  private readonly commandController: GeneralChatCommandController;
  private readonly draftLifecycleController: GeneralChatDraftLifecycleController;
  private readonly callbacks: GeneralChatBootstrapCallbacks;

  constructor(
    configStore: GeneralChatConfigStore,
    commandController: GeneralChatCommandController,
    draftLifecycleController: GeneralChatDraftLifecycleController,
    callbacks: GeneralChatBootstrapCallbacks
  ) {
    this.configStore = configStore;
    this.commandController = commandController;
    this.draftLifecycleController = draftLifecycleController;
    this.callbacks = callbacks;
  }

  async restore(context: Context): Promise<void> {
    await this.restoreConfig(context);
    await this.restoreLocalChat(context);
  }

  private async restoreConfig(context: Context): Promise<void> {
    try {
      const snapshot = await this.configStore.init(context);
      this.callbacks.onConfigRestored(snapshot);
    } catch (err) {
      RemoteLogger.warn(`general chat configuration restore failed: ${ConnectionErrorPolicy.errorText(err)}`);
    }
  }

  private async restoreLocalChat(context: Context): Promise<void> {
    try {
      await this.commandController.init(context);
      this.commandController.refreshSessions();
      this.callbacks.onHomeDraftRestored(await this.draftLifecycleController.restoreHome());
    } catch (err) {
      RemoteLogger.warn(`general chat restore failed: ${ConnectionErrorPolicy.errorText(err)}`);
      this.callbacks.onStatusText(RemoteI18n.t('generalChat.localRestoreFailed'));
    }
  }
}
