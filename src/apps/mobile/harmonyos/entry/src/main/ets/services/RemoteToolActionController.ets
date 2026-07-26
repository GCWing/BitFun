import { RemoteQuestionAnswerPayload } from '../model/RemoteModels';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from './ConnectionErrorPolicy';

export interface RemoteToolActionClient {
  confirmTool(toolId: string, updatedInput?: Object): Promise<void>;
  rejectTool(toolId: string, reason: string): Promise<void>;
  cancelTool(toolId: string, reason: string): Promise<void>;
  answerQuestion(toolId: string, answers: RemoteQuestionAnswerPayload): Promise<void>;
}

export class RemoteToolActionController {
  private readonly client: RemoteToolActionClient;
  private readonly onStatusText: (statusText: string) => void;
  private readonly onBusy: (isBusy: boolean) => void;
  private readonly onPoll: () => void;

  constructor(
    client: RemoteToolActionClient,
    onStatusText: (statusText: string) => void,
    onBusy: (isBusy: boolean) => void,
    onPoll: () => void
  ) {
    this.client = client;
    this.onStatusText = onStatusText;
    this.onBusy = onBusy;
    this.onPoll = onPoll;
  }

  async approve(
    toolId: string,
    sessionId: string,
    isBusy: boolean,
    remoteAvailable: boolean,
    updatedInput?: Object
  ): Promise<void> {
    await this.runToolAction(
      toolId,
      sessionId,
      isBusy,
      remoteAvailable,
      RemoteI18n.t('status.approvingTool'),
      RemoteI18n.t('status.toolApproved'),
      () => {
        return this.client.confirmTool(toolId, updatedInput);
      }
    );
  }

  async reject(toolId: string, sessionId: string, isBusy: boolean, remoteAvailable: boolean): Promise<void> {
    await this.runToolAction(
      toolId,
      sessionId,
      isBusy,
      remoteAvailable,
      RemoteI18n.t('status.rejectingTool'),
      RemoteI18n.t('status.toolRejected'),
      () => {
        return this.client.rejectTool(toolId, 'Rejected from HarmonyOS remote client');
      }
    );
  }

  async cancel(toolId: string, sessionId: string, isBusy: boolean, remoteAvailable: boolean): Promise<void> {
    await this.runToolAction(
      toolId,
      sessionId,
      isBusy,
      remoteAvailable,
      RemoteI18n.t('status.cancellingTool'),
      RemoteI18n.t('status.toolCancelRequested'),
      () => {
        return this.client.cancelTool(toolId, 'Cancelled from HarmonyOS remote client');
      }
    );
  }

  async answer(
    toolId: string,
    sessionId: string,
    isBusy: boolean,
    remoteAvailable: boolean,
    answers: RemoteQuestionAnswerPayload
  ): Promise<void> {
    await this.runToolAction(
      toolId,
      sessionId,
      isBusy,
      remoteAvailable,
      RemoteI18n.t('status.submittingAnswer'),
      RemoteI18n.t('status.answerSubmitted'),
      () => {
        return this.client.answerQuestion(toolId, answers);
      }
    );
  }

  private async runToolAction(
    toolId: string,
    sessionId: string,
    isBusy: boolean,
    remoteAvailable: boolean,
    pendingText: string,
    successText: string,
    action: () => Promise<void>
  ): Promise<void> {
    if (sessionId.length === 0 || toolId.length === 0 || isBusy || !remoteAvailable) {
      return;
    }
    try {
      this.onBusy(true);
      this.onStatusText(pendingText);
      await action();
      this.onStatusText(successText);
      this.onPoll();
    } catch (err) {
      this.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.onBusy(false);
    }
  }
}
