import {
  ChatMessage,
  RemoteImageContext,
  SelectedImageAttachment,
  SteerTurnResult,
  SessionMessagesResult,
  SessionSummary
} from '../model/RemoteModels';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from './ConnectionErrorPolicy';
import { RemoteChatCache } from './RemoteChatCache';
import { RemoteLogger } from './RemoteLogger';
import { TranscriptIntegrityPolicy } from './TranscriptIntegrityPolicy';

export interface RemoteChatCommandClient {
  getSessionMessages(sessionId: string): Promise<SessionMessagesResult>;
  sendMessage(sessionId: string, text: string, agentType?: string, turnId?: string): Promise<string>;
  sendMessageWithImages(
    sessionId: string,
    text: string,
    agentType: string,
    imageContexts: RemoteImageContext[],
    turnId?: string
  ): Promise<string>;
  steerTurn(
    sessionId: string,
    turnId: string,
    text: string,
    displayContent: string,
    imageContexts: RemoteImageContext[]
  ): Promise<SteerTurnResult>;
  buildPlan(sessionId: string, planFilePath: string, planName: string, agentType: string): Promise<string>;
  cancelTask(sessionId: string, turnId?: string): Promise<void>;
  renameSession(sessionId: string, title: string): Promise<void>;
}

export interface RemoteChatCommandCallbacks {
  onMessagesLoaded: (messages: ChatMessage[], hasMoreMessages: boolean) => void;
  onMessageCountKnown: (pollVersion: number, knownMessageCount: number) => void;
  // Raised once the transcript is on screen, which for a cached session is
  // before any request has been sent. Without it the loading skeleton would
  // outlive the messages it is standing in for.
  onTimelineReady: () => void;
  onSendSucceeded: (turnId: string, pendingActiveId: string, localMessageId: string) => void;
  onSteerSucceeded?: (
    steeringId: string,
    turnId: string,
    displayText: string,
    images: SelectedImageAttachment[]
  ) => void;
  onSendFailed: (
    rawText: string,
    images: SelectedImageAttachment[],
    localMessageId: string,
    pendingActiveId: string
  ) => void;
  onActiveSession: (session: SessionSummary) => void;
  onSessionTitleChanged: (sessionId: string, title: string) => void;
  onStatusText: (statusText: string) => void;
  // The status line is only rendered while the link is down, so anything the
  // user asked for and did not get has to be said out loud instead.
  onToast: (message: string) => void;
  onBusy: (isBusy: boolean) => void;
  onPollRequested: () => void;
}

export class RemoteChatCommandController {
  private readonly client: RemoteChatCommandClient;
  private readonly callbacks: RemoteChatCommandCallbacks;
  private readonly cache: RemoteChatCache;
  // A transcript RPC already accepted by the desktop cannot be cancelled, and
  // device RPCs are handled one at a time. Keep at most that one unavoidable
  // request in flight; callers waiting behind it re-check the generation and
  // only the latest selected session is allowed to send the next request.
  private initialLoadGeneration: number = 0;
  private initialLoadInFlight?: Promise<void>;
  private initialLoadSessionId: string = '';

  constructor(
    client: RemoteChatCommandClient,
    callbacks: RemoteChatCommandCallbacks,
    cache: RemoteChatCache
  ) {
    this.client = client;
    this.callbacks = callbacks;
    this.cache = cache;
  }

  private static shortId(value: string): string {
    if (value.length <= 10) return value;
    return `${value.slice(0, 6)}...${value.slice(-4)}`;
  }

  /**
   * Puts a session's transcript on screen, from disk when there is one.
   *
   * A cache hit with complete rows sends nothing: the poller that starts right
   * after this asks for `known_msg_count` messages onwards, so whatever arrived
   * while the session was closed comes back as an ordinary tail rather than as
   * another copy of the whole transcript.
   *
   * Hollow assistants are a miss. The poller will not refill them — it skips
   * the rows the phone already counted — so the only way back to a body is a
   * full `get_session_messages`. The hollow rows still go on screen first so
   * the user messages do not wait on that fetch.
   */
  async loadMessages(
    sessionId: string,
    canApply: (sessionId: string) => boolean
  ): Promise<void> {
    if (sessionId.length === 0) {
      return;
    }
    const generation = ++this.initialLoadGeneration;
    const cached = await this.cache.load(sessionId);
    if (generation !== this.initialLoadGeneration) {
      RemoteLogger.info(
        `message load superseded after cache session=${RemoteChatCommandController.shortId(sessionId)}`
      );
      return;
    }
    if (cached.length > 0) {
      if (!canApply(sessionId)) {
        return;
      }
      RemoteLogger.info(`messages from cache session=${RemoteChatCommandController.shortId(sessionId)} count=${cached.length}`);
      this.applyMessages(cached);
      if (TranscriptIntegrityPolicy.hasHollowAssistants(cached)) {
        RemoteLogger.info(
          `hollow assistants in cache session=${RemoteChatCommandController.shortId(sessionId)} ` +
            `${TranscriptIntegrityPolicy.describeMessages(cached)}`
        );
        await this.cache.forget(sessionId);
        await this.reloadMessages(sessionId, canApply);
        return;
      }
      this.callbacks.onStatusText(RemoteI18n.t('status.messagesRestored'));
      return;
    }
    await this.loadLatestMessages(sessionId, canApply, generation);
  }

  /**
   * Serializes initial transcript misses without turning every rapid tap into a
   * desktop RPC. The request already on the wire is still cached when it lands;
   * all selections made while it is running race only in memory, and the last
   * one wins the right to issue the next request.
   */
  private async loadLatestMessages(
    sessionId: string,
    canApply: (sessionId: string) => boolean,
    generation: number
  ): Promise<void> {
    while (this.initialLoadInFlight !== undefined) {
      const inFlight = this.initialLoadInFlight;
      if (this.initialLoadSessionId === sessionId) {
        RemoteLogger.info(
          `message load joined session=${RemoteChatCommandController.shortId(sessionId)}`
        );
        await inFlight;
        return;
      }
      RemoteLogger.info(
        `message load waiting session=${RemoteChatCommandController.shortId(sessionId)} ` +
          `behind=${RemoteChatCommandController.shortId(this.initialLoadSessionId)}`
      );
      await inFlight;
      if (generation !== this.initialLoadGeneration) {
        RemoteLogger.info(
          `message load superseded before request session=${RemoteChatCommandController.shortId(sessionId)}`
        );
        return;
      }
    }
    if (generation !== this.initialLoadGeneration) {
      RemoteLogger.info(
        `message load superseded before start session=${RemoteChatCommandController.shortId(sessionId)}`
      );
      return;
    }

    const request = this.reloadMessages(sessionId, canApply);
    this.initialLoadInFlight = request;
    this.initialLoadSessionId = sessionId;
    try {
      await request;
    } finally {
      if (this.initialLoadInFlight === request) {
        this.initialLoadInFlight = undefined;
        this.initialLoadSessionId = '';
      }
    }
  }

  /**
   * Fetches the whole transcript and makes it the cached one.
   *
   * Also the way out of a desktop-side rewrite: the tail protocol counts from
   * the start of the transcript, so once the desktop reports fewer messages
   * than the phone holds there is no offset left to resume from.
   */
  async reloadMessages(
    sessionId: string,
    canApply: (sessionId: string) => boolean
  ): Promise<void> {
    if (sessionId.length === 0) {
      return;
    }
    try {
      // Guarded like every other write below. A tap that lands while a disk read
      // is still settling gets here for a session that is already off screen,
      // and "正在加载消息" against someone else's transcript reads as a stall the
      // visible session is not having.
      if (canApply(sessionId)) {
        this.callbacks.onStatusText(RemoteI18n.t('status.loadMessages'));
      }
      const result = await this.client.getSessionMessages(sessionId);
      RemoteLogger.info(
        `get_session_messages mapped session=${RemoteChatCommandController.shortId(sessionId)} ` +
          `${TranscriptIntegrityPolicy.describeMessages(result.messages)}`
      );
      if (canApply(sessionId)) {
        this.applyMessages(result.messages);
        this.callbacks.onStatusText(RemoteI18n.t('status.messagesSynced'));
      }
      // Stored whether or not that session is still the one on screen: the
      // transcript was fetched by session id, so it is as valid for a session
      // the user just switched away from as for one they stayed on. Discarding
      // it would leave a session the user demonstrably opened uncached, and a
      // fetch slow enough to be switched away from is exactly the one worth not
      // paying for twice.
      //
      // Writing another session's transcript moves the cache's append cursor
      // off the visible one, which costs that session a single full rewrite on
      // its next sync before appends resume.
      await this.cache.replaceSnapshot(sessionId, result.messages);
    } catch (err) {
      // A failure belongs to the session it was fetched for. Unguarded, a slow
      // fetch the user switched away from would land its error text and its
      // empty timeline on whatever session they switched to — the failure of
      // the session they left, reported as the state of the one they are on.
      if (!canApply(sessionId)) {
        RemoteLogger.info(
          `stale message load dropped session=${RemoteChatCommandController.shortId(sessionId)}`);
        return;
      }
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
      this.callbacks.onMessageCountKnown(0, 0);
      this.callbacks.onTimelineReady();
    }
  }

  /**
   * A poll version of 0 is deliberate on both paths: the desktop resets its
   * tracker when it restarts, so a version carried over from a previous
   * connection would match by accident and answer "nothing changed".
   *
   * The count handed on is the message count, which for a restored transcript
   * can sit below what the desktop counts, because the timeline drops synthetic
   * system entries. Erring low is the safe direction: the next poll returns a
   * few messages already on screen, they merge by id, and the count corrects
   * itself from the desktop's own total.
   */
  private applyMessages(messages: ChatMessage[]): void {
    this.callbacks.onMessagesLoaded(messages, false);
    this.callbacks.onMessageCountKnown(0, messages.length);
    this.callbacks.onTimelineReady();
    // The last thing that happens before ArkUI owns the delay: everything after
    // this line is build, measure and paint.
    RemoteLogger.info(
      `messages applied to state ${TranscriptIntegrityPolicy.describeMessages(messages)}`
    );
  }

  /** Drops a session's cached transcript once it no longer exists upstream. */
  async forgetMessages(sessionId: string): Promise<void> {
    await this.cache.forget(sessionId);
  }

  /**
   * Reaches back for history the timeline is missing.
   *
   * No desktop reachable over the relay reports one today: `get_session_messages`
   * ignores its `limit`/`before_message_id` and answers with the whole
   * transcript and `has_more: false`, so `hasMoreMessages` never turns true and
   * this stays a fetch nothing currently asks for. It is kept whole, and kept
   * writing to the cache like every other full fetch, so that the day the
   * desktop does page its history this path is not the odd one out.
   */
  async loadOlderMessages(
    sessionId: string,
    currentPollVersion: number,
    hasMoreMessages: boolean,
    isBusy: boolean
  ): Promise<void> {
    if (sessionId.length === 0 || !hasMoreMessages || isBusy) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      this.callbacks.onStatusText(RemoteI18n.t('status.loadOlderMessages'));
      const result = await this.client.getSessionMessages(sessionId);
      this.callbacks.onMessagesLoaded(result.messages, false);
      this.callbacks.onMessageCountKnown(currentPollVersion, result.messages.length);
      this.callbacks.onStatusText(RemoteI18n.t('status.messagesSynced'));
      await this.cache.replaceSnapshot(sessionId, result.messages);
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async sendPreparedMessage(
    sessionId: string,
    text: string,
    agentType: string,
    rawText: string,
    images: SelectedImageAttachment[],
    imageContexts: RemoteImageContext[],
    localMessageId: string,
    pendingActiveId: string,
    isBusy: boolean,
    remoteAvailable: boolean,
    clientTurnId: string = ''
  ): Promise<void> {
    if ((text.length === 0 && images.length === 0) || sessionId.length === 0 || isBusy || !remoteAvailable) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      this.callbacks.onStatusText(images.length > 0 ? RemoteI18n.t('status.sendImage') : RemoteI18n.t('status.sendCommand'));
      const requestedTurnId = clientTurnId.trim().length > 0 ? clientTurnId : localMessageId;
      const turnId = images.length > 0 ?
        await this.client.sendMessageWithImages(sessionId, text, agentType, imageContexts, requestedTurnId) :
        await this.client.sendMessage(sessionId, text, agentType, requestedTurnId);
      this.callbacks.onSendSucceeded(turnId, pendingActiveId, localMessageId);
      this.callbacks.onStatusText(RemoteI18n.t('status.sentWaiting'));
    } catch (err) {
      this.callbacks.onSendFailed(rawText, images, localMessageId, pendingActiveId);
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async steerPreparedMessage(
    sessionId: string,
    turnId: string,
    text: string,
    displayText: string,
    images: SelectedImageAttachment[],
    imageContexts: RemoteImageContext[],
    isBusy: boolean,
    remoteAvailable: boolean
  ): Promise<void> {
    if ((text.length === 0 && images.length === 0) || sessionId.length === 0 || turnId.length === 0 ||
      isBusy || !remoteAvailable) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      const result = await this.client.steerTurn(
        sessionId,
        turnId,
        text,
        displayText,
        imageContexts
      );
      if (this.callbacks.onSteerSucceeded) {
        this.callbacks.onSteerSucceeded(result.steeringId, result.turnId, displayText, images);
      }
      this.callbacks.onStatusText(RemoteI18n.t('status.sentWaiting'));
      this.callbacks.onPollRequested();
    } catch (err) {
      this.callbacks.onSendFailed(displayText, images, '', '');
      const errorText = ConnectionErrorPolicy.errorText(err);
      this.callbacks.onStatusText(errorText);
      this.callbacks.onToast(errorText);
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async buildPlan(
    sessionId: string,
    planFilePath: string,
    planName: string,
    agentType: string,
    localMessageId: string,
    pendingActiveId: string,
    isBusy: boolean,
    remoteAvailable: boolean
  ): Promise<void> {
    if (sessionId.length === 0 || planFilePath.length === 0 || isBusy || !remoteAvailable) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      const turnId = await this.client.buildPlan(sessionId, planFilePath, planName, agentType);
      this.callbacks.onSendSucceeded(turnId, pendingActiveId, localMessageId);
      this.callbacks.onStatusText(RemoteI18n.t('status.sentWaiting'));
      this.callbacks.onPollRequested();
    } catch (err) {
      this.callbacks.onSendFailed('', [], localMessageId, pendingActiveId);
      const errorText = ConnectionErrorPolicy.errorText(err);
      this.callbacks.onStatusText(errorText);
      this.callbacks.onToast(errorText);
    } finally {
      this.callbacks.onBusy(false);
    }
  }

  async stopTask(
    sessionId: string,
    activeTurnMessageId: string,
    activeTurnId: string,
    remoteAvailable: boolean
  ): Promise<void> {
    RemoteLogger.info(`stop task requested session=${RemoteChatCommandController.shortId(sessionId)} turn=${RemoteChatCommandController.shortId(activeTurnId)} active=${activeTurnMessageId.length > 0 ? '1' : '0'}`);
    if (sessionId.length === 0 || !remoteAvailable) {
      return;
    }
    if (activeTurnMessageId.length === 0) {
      const nothingRunning = RemoteI18n.t('status.noRunningTask');
      this.callbacks.onStatusText(nothingRunning);
      this.callbacks.onToast(nothingRunning);
      return;
    }
    try {
      this.callbacks.onStatusText(RemoteI18n.t('status.stoppingTask'));
      await this.client.cancelTask(sessionId, activeTurnId);
      RemoteLogger.info(`stop task accepted session=${RemoteChatCommandController.shortId(sessionId)} turn=${RemoteChatCommandController.shortId(activeTurnId)}`);
      this.callbacks.onStatusText(RemoteI18n.t('status.stopRequested'));
      this.callbacks.onPollRequested();
    } catch (err) {
      // A rejected cancel leaves the task running, so failing quietly here is
      // indistinguishable from the stop having worked.
      const errorText = ConnectionErrorPolicy.errorText(err);
      RemoteLogger.info(`stop task failed session=${RemoteChatCommandController.shortId(sessionId)} turn=${RemoteChatCommandController.shortId(activeTurnId)}`);
      this.callbacks.onStatusText(errorText);
      this.callbacks.onToast(errorText);
    }
  }

  async renameActiveSession(
    session: SessionSummary,
    title: string,
    isBusy: boolean,
    remoteAvailable: boolean
  ): Promise<void> {
    const sessionId = session.sessionId || '';
    const nextTitle = title.trim();
    if (sessionId.length === 0 || nextTitle.length === 0 || nextTitle === session.title || isBusy || !remoteAvailable) {
      return;
    }
    try {
      this.callbacks.onBusy(true);
      this.callbacks.onStatusText(RemoteI18n.t('status.updatingTitle'));
      await this.client.renameSession(sessionId, nextTitle);
      const updatedSession: SessionSummary = {
        sessionId,
        title: nextTitle,
        workspacePath: session.workspacePath,
        agentType: session.agentType,
        initialTurnId: session.initialTurnId
      };
      this.callbacks.onActiveSession(updatedSession);
      this.callbacks.onSessionTitleChanged(sessionId, nextTitle);
      this.callbacks.onStatusText(RemoteI18n.t('status.titleUpdated'));
    } catch (err) {
      this.callbacks.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.callbacks.onBusy(false);
    }
  }
}
