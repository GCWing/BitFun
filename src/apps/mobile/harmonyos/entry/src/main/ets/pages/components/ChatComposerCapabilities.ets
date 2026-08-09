import { ChatSurface } from './ChatSurface';

export class ChatComposerCapabilities {
  readonly surface: ChatSurface;
  readonly supportsAttachments: boolean;
  readonly requiresRemoteConnection: boolean;
  readonly showAddButton: boolean;
  readonly showVoiceInput: boolean;

  constructor(
    surface: ChatSurface,
    supportsAttachments: boolean,
    requiresRemoteConnection: boolean,
    showAddButton: boolean = true,
    showVoiceInput: boolean = true
  ) {
    this.surface = surface;
    this.supportsAttachments = supportsAttachments;
    this.requiresRemoteConnection = requiresRemoteConnection;
    this.showAddButton = showAddButton;
    this.showVoiceInput = showVoiceInput;
  }
}

export const GENERAL_CHAT_COMPOSER_CAPABILITIES: ChatComposerCapabilities =
  new ChatComposerCapabilities(ChatSurface.General, false, false);

export const REMOTE_CHAT_COMPOSER_CAPABILITIES: ChatComposerCapabilities =
  new ChatComposerCapabilities(ChatSurface.Remote, true, true);

export const REMOTE_CREATE_COMPOSER_CAPABILITIES: ChatComposerCapabilities =
  new ChatComposerCapabilities(ChatSurface.Remote, false, true, false, true);
