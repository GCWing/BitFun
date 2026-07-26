import { ChatMessage, ImageAttachment, RemoteSession } from '../../model/RemoteModels';
import { GeneralChatCommandClient } from './GeneralChatCommandController';
import { GeneralChatLocalRdbStore } from './GeneralChatLocalRdbStore';
import { GeneralChatConfigStore } from './GeneralChatConfigStore';
import { GeneralChatDraftStore, GeneralChatSendResult, GeneralChatStreamObserver } from './GeneralChatPort';
import { GeneralChatRepository } from './GeneralChatRepository';
import { ModelProviderGeneralChatAdapter } from './ModelProviderGeneralChatAdapter';

export class GeneralChatController implements GeneralChatCommandClient, GeneralChatDraftStore {
  private readonly repository: GeneralChatRepository;

  constructor(repository: GeneralChatRepository) {
    this.repository = repository;
  }

  static createDefault(configStore: GeneralChatConfigStore): GeneralChatController {
    return new GeneralChatController(new GeneralChatRepository(
      new ModelProviderGeneralChatAdapter(configStore),
      new GeneralChatLocalRdbStore()
    ));
  }

  async init(context: Context): Promise<void> {
    await this.repository.init(context);
  }

  sessions(): RemoteSession[] {
    return this.repository.listSessions();
  }

  async createSession(firstMessage: string): Promise<RemoteSession> {
    return this.repository.createSession(firstMessage);
  }

  async messages(sessionId: string): Promise<ChatMessage[]> {
    return this.repository.messages(sessionId);
  }

  async sendMessage(
    sessionId: string,
    text: string,
    images: ImageAttachment[] = [],
    observer?: GeneralChatStreamObserver
  ): Promise<GeneralChatSendResult> {
    return this.repository.sendMessage(sessionId, text, images, observer);
  }

  cancelCurrentRequest(): void {
    this.repository.cancelCurrentRequest();
  }

  async recordAssistantMessage(sessionId: string, message: ChatMessage): Promise<void> {
    await this.repository.recordAssistantMessage(sessionId, message);
  }

  async renameSession(sessionId: string, title: string): Promise<void> {
    await this.repository.renameSession(sessionId, title);
  }

  async archiveSession(sessionId: string, archived: boolean): Promise<void> {
    await this.repository.archiveSession(sessionId, archived);
  }

  async pinSession(sessionId: string, pinned: boolean): Promise<void> {
    await this.repository.pinSession(sessionId, pinned);
  }

  async deleteSession(sessionId: string): Promise<void> {
    await this.repository.deleteSession(sessionId);
  }

  async draft(draftId: string): Promise<string> {
    return this.repository.draft(draftId);
  }

  async saveDraft(draftId: string, text: string): Promise<void> {
    await this.repository.saveDraft(draftId, text);
  }

  async clearDraft(draftId: string): Promise<void> {
    await this.repository.clearDraft(draftId);
  }

  async removeFailedUserMessage(sessionId: string, text: string): Promise<void> {
    await this.repository.removeFailedUserMessage(sessionId, text);
  }
}
