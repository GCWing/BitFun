import { RemoteSession } from '../../model/RemoteModels';
import { ChatTimelineItem } from '../../model/ChatTimelineModels';
import { ChatTimelineState } from '../../services/ChatTimelineStore';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteLogger } from '../../services/RemoteLogger';
import { RemoteUiState } from '../../services/RemoteUiState';
import {
  GeneralChatServiceState,
  GeneralChatServiceStatus
} from '../../services/general-chat/GeneralChatServiceState';
import { AppRoute } from '../navigation/AppRouteContract';
import { GeneralChatPageState } from '../state/GeneralChatPageState';
import { RemotePageState } from '../state/RemotePageState';
import {
  GENERAL_CHAT_HOME_DRAFT_ID,
  RemoteConversationDependencies,
  requireRemoteRuntime
} from './ConversationRuntime';
import { RemoteCreateFlowController } from './RemoteCreateFlowController';
import { RemoteTranscriptController } from './RemoteTranscriptController';

export class VisibleConversationController {
  private readonly general: GeneralChatPageState;
  private readonly remote: RemotePageState;
  private readonly remoteRuntime?: RemoteConversationDependencies;
  private readonly transcript: RemoteTranscriptController;
  private readonly createFlow: RemoteCreateFlowController;
  private readonly setChatInputRef: (route: AppRoute, value: string) => void;
  private readonly setVisibleStatusTextRef: (statusText: string) => void;
  private readonly isGeneralComposerRouteRef: (route: AppRoute) => boolean;
  private readonly notify: (message: string) => void;

  constructor(
    general: GeneralChatPageState,
    remote: RemotePageState,
    remoteRuntime: RemoteConversationDependencies | undefined,
    transcript: RemoteTranscriptController,
    createFlow: RemoteCreateFlowController,
    setChatInputRef: (route: AppRoute, value: string) => void,
    setVisibleStatusTextRef: (statusText: string) => void,
    isGeneralComposerRouteRef: (route: AppRoute) => boolean,
    notify: (message: string) => void
  ) {
    this.general = general;
    this.remote = remote;
    this.remoteRuntime = remoteRuntime;
    this.transcript = transcript;
    this.createFlow = createFlow;
    this.setChatInputRef = setChatInputRef;
    this.setVisibleStatusTextRef = setVisibleStatusTextRef;
    this.isGeneralComposerRouteRef = isGeneralComposerRouteRef;
    this.notify = notify;
  }

  openHomeSession(session: RemoteSession, inPlace: boolean = false): void {
    requireRemoteRuntime(this.remoteRuntime).filePreview.close();
    if (session.agentType === 'chat') {
      this.openGeneralSession(session);
      return;
    }
    void this.createFlow.openRemoteSession(session, inPlace).catch((err: Object) => {
      const message = err instanceof Error ? err.message : String(err);
      this.notify(message);
    });
  }

  async deleteHomeSession(session: RemoteSession): Promise<void> {
    if (session.agentType !== 'chat') {
      await this.createFlow.deleteRemoteSession(session);
      return;
    }
    await requireRemoteRuntime(this.remoteRuntime).generalCommands.deleteSession(session, this.general.isBusy);
  }

  activeGeneralChatAsRemoteSession(): RemoteSession {
    const active = this.general.activeSession;
    return {
      id: active.sessionId,
      title: active.title,
      agentType: 'chat',
      status: 'ready',
      updatedAt: '',
      createdAt: '',
      messageCount: this.general.timelineItems.length,
      workspacePath: active.workspacePath
    };
  }

  activeGeneralUploadedFileCount(): number {
    let count = 0;
    this.general.timelineItems.forEach((item: ChatTimelineItem) => {
      if (item.message && item.message.images) {
        count += item.message.images.length;
      }
    });
    return count;
  }

  async archiveHomeSession(session: RemoteSession, archived: boolean): Promise<void> {
    await requireRemoteRuntime(this.remoteRuntime).generalCommands.archiveSession(session, archived, this.general.isBusy);
  }

  async exportHomeSession(session: RemoteSession): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    await runtime.generalCommands.exportSession(
      session,
      this.general.isBusy,
      async (text: string): Promise<void> => runtime.clipboard.writeText(text)
    );
  }

  async openGeneralSession(item: RemoteSession): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    if (this.general.isBusy) {
      return;
    }
    runtime.polling.stop();
    runtime.generalConversation.stop(false);
    await runtime.generalCommands.openSession(
      item,
      this.general.isBusy,
      async (sessionId: string): Promise<string> => runtime.generalDrafts.restore(sessionId),
      (_sessionId: string): void => runtime.appShell.replaceRouteWithoutAnimation(AppRoute.ChatHome)
    );
  }

  async startGeneralChat(text: string): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    const trimmed = text.trim();
    if (trimmed.length === 0 || this.general.isBusy) {
      return;
    }
    runtime.polling.stop();
    runtime.generalConversation.stop(false);
    runtime.generalDrafts.cancel();
    const created = await runtime.generalCommands.createSession(
      trimmed,
      this.general.isBusy,
      async (): Promise<void> => runtime.generalDrafts.clearHomeNow(),
      (_sessionId: string): void => runtime.appShell.replaceRouteWithoutAnimation(AppRoute.ChatHome)
    );
    if (created) {
      await runtime.generalConversation.sendMessage();
    }
  }

  async sendVisibleMessage(): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    if (runtime.appShell.isGeneralChatVisible()) {
      if ((this.general.activeSession.sessionId || '').length === 0) {
        this.startVisibleGeneralChat();
        return;
      }
      await runtime.generalConversation.sendMessage();
      return;
    }
    await this.transcript.sendRemoteMessage();
  }

  async stopVisibleTask(): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    if (runtime.appShell.isGeneralChatVisible()) {
      runtime.generalConversation.stop(true);
      return;
    }
    await this.transcript.stopRemoteTask();
  }

  closeActiveChat(): void {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    runtime.filePreview.close();
    runtime.hooks.stopVoiceInput();
    if (runtime.appShell.isRoute(AppRoute.GeneralChat)) {
      runtime.generalDrafts.persistVisible(this.general.chatInput);
      runtime.generalConversation.stop(true);
      runtime.appShell.popRoute(AppRoute.ChatHome);
      this.restoreGeneralChatDraft(GENERAL_CHAT_HOME_DRAFT_ID);
      return;
    }
    runtime.polling.stop();
    this.remote.setConversationDismissed(true);
    runtime.appShell.replaceRouteWithoutAnimation(AppRoute.RemoteHome);
  }

  async renameVisibleSession(title: string): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    if (runtime.appShell.isGeneralChatVisible()) {
      await runtime.generalCommands.renameActiveSession(this.general.activeSession, title);
      return;
    }
    await this.transcript.renameRemoteSession(title);
  }

  async retryVisibleMessage(text: string): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    if (runtime.appShell.isGeneralChatVisible()) {
      const prepared = await runtime.generalCommands.retryMessage(
        this.general.activeSession.sessionId || '', text, this.general.isBusy
      );
      if (prepared) {
        await runtime.generalConversation.sendMessage();
      }
      return;
    }
    this.transcript.retryRemoteMessage(text);
  }

  downloadVisibleFile(path: string): void {
    if (requireRemoteRuntime(this.remoteRuntime).appShell.isGeneralChatVisible()) {
      this.general.setStatus(RemoteI18n.t('generalChat.fileDownloadMock'));
      return;
    }
    this.transcript.downloadRemoteFile(path);
  }

  async selectVisibleModel(modelId: string): Promise<void> {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    if (runtime.appShell.isGeneralChatVisible()) {
      await runtime.settings.selectModel(modelId);
      return;
    }
    await this.transcript.selectRemoteModel(modelId);
  }

  startVisibleGeneralChat(): void {
    const rawText = this.general.chatInput.trim();
    const text = rawText.length > 0 ? rawText :
      (this.general.selectedImages.length > 0 ? RemoteI18n.t('chat.analyzeImageDefault') : '');
    if (text.length === 0 || this.general.isBusy) {
      return;
    }
    if (this.general.serviceState === GeneralChatServiceState.Unconfigured) {
      const statusText = GeneralChatServiceStatus.userMessage(this.general.serviceState);
      this.general.setStatus(statusText);
      this.notify(statusText);
      return;
    }
    this.startGeneralChat(text);
  }

  generalChatHomeStatusText(): string {
    if (this.general.serviceState === GeneralChatServiceState.Ready ||
      this.general.serviceState === GeneralChatServiceState.Sending ||
      this.general.serviceState === GeneralChatServiceState.Streaming) {
      return '';
    }
    return GeneralChatServiceStatus.userMessage(this.general.serviceState, this.general.statusText);
  }

  prepareNewGeneralChat(): void {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    runtime.hooks.stopVoiceInput();
    runtime.generalConversation.stop(true);
    runtime.generalDrafts.clearHome();
    this.general.clearComposer();
    this.general.clearActiveSession();
    this.resetGeneralTimeline('');
    runtime.appShell.replaceRouteWithoutAnimation(AppRoute.ChatHome);
  }

  onVisibleChatInputChange(route: AppRoute, value: string): void {
    this.setChatInputRef(route, value);
    if (this.isGeneralComposerRouteRef(route)) {
      requireRemoteRuntime(this.remoteRuntime).generalDrafts.scheduleVisible(value);
    }
  }

  visibleGeneralChatDraftId(): string {
    return requireRemoteRuntime(this.remoteRuntime).appShell.isGeneralChatVisible() ?
      this.general.activeSession.sessionId || GENERAL_CHAT_HOME_DRAFT_ID : '';
  }

  async restoreGeneralChatDraft(draftId: string): Promise<void> {
    this.general.setChatInput(await requireRemoteRuntime(this.remoteRuntime).generalDrafts.restore(draftId));
  }

  latestUserMessageText(): string {
    if (requireRemoteRuntime(this.remoteRuntime).appShell.isGeneralChatVisible()) {
      return this.general.latestUserMessageText();
    }
    const candidates = this.remote.persistedMessages.concat(this.remote.optimisticMessages);
    for (let index = candidates.length - 1; index >= 0; index--) {
      if (candidates[index].role === 'user' && candidates[index].text.trim().length > 0) {
        return candidates[index].text;
      }
    }
    return '';
  }

  resetGeneralTimeline(sessionId: string): void {
    requireRemoteRuntime(this.remoteRuntime).timeline.reset(sessionId);
    this.syncGeneralTimeline();
  }

  syncGeneralTimeline(): void {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    const state: ChatTimelineState = runtime.timeline.snapshotState();
    const projectedItems = runtime.timeline.viewState(false);
    this.general.setTimelineProjection(
      state.persistedMessages,
      state.optimisticMessages,
      state.activeTurn || RemoteUiState.emptyActiveTurn(),
      false,
      projectedItems
    );
    const itemSummary = projectedItems.map((item: ChatTimelineItem) => {
      const message = item.message;
      return `${item.type}:${item.id}:${message ? message.status : ''}:${message ? message.text.length : 0}`;
    }).join(',');
    RemoteLogger.info(`general chat projection revision=${this.general.timelineRevision} persisted=${state.persistedMessages.length} active=${state.activeTurn ? state.activeTurn.id : 'none'} items=${itemSummary}`);
  }

  showHomeToast(message: string): void {
    const runtime = requireRemoteRuntime(this.remoteRuntime);
    if (!runtime.hooks.showToast(message)) {
      this.setVisibleStatusTextRef(message);
    }
  }
}
