import { ChatSurface } from './ChatSurface';

export class ChatComposerCapabilities {
  readonly surface: ChatSurface;
  readonly supportsAttachments: boolean;
  readonly requiresRemoteConnection: boolean;

  constructor(surface: ChatSurface, supportsAttachments: boolean, requiresRemoteConnection: boolean) {
    this.surface = surface;
    this.supportsAttachments = supportsAttachments;
    this.requiresRemoteConnection = requiresRemoteConnection;
  }
}

export const GENERAL_CHAT_COMPOSER_CAPABILITIES: ChatComposerCapabilities =
  new ChatComposerCapabilities(ChatSurface.General, false, false);

export const REMOTE_CHAT_COMPOSER_CAPABILITIES: ChatComposerCapabilities =
  new ChatComposerCapabilities(ChatSurface.Remote, true, true);
