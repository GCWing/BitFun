import type { ContextItem } from '@/shared/types/context';
import type { ComposerPresentation } from '../utils/composerPresentation';
import type { GroupChatActor, GroupChatMember } from '../types/flow-chat';

/**
 * One submission produced by the shared ChatInput surface.
 *
 * Alternate hosts (for example an Agentic MiniApp) may register a transport
 * for the standard composer, but they receive the same normalized submission
 * shape instead of replacing the composer UI.
 */
export interface ChatInputSubmission {
  /** Model-facing text after rich-composer tokens have been expanded. */
  text: string;
  /** User-facing text preserved for conversation rendering. */
  displayText: string;
  contexts: ContextItem[];
  composerPresentation: ComposerPresentation | null;
  sessionId?: string;
  workspacePath?: string;
}

export interface ChatInputDraftRegistration {
  /** Distinguishes repeated requests that happen to carry the same text. */
  id: number;
  text: string;
}

/**
 * Declarative extension points for the one shared ChatInput implementation.
 * This contract intentionally exposes content and routing only: layout,
 * controls, state transitions, attachments, voice, model/permission controls,
 * and stop behavior remain owned by ChatInput.
 */
export interface ChatInputRegistration {
  /** Stable owner identity used to isolate draft requests across registrations. */
  registrationId?: string;
  placeholder?: string;
  /**
   * Workspace used by mentions and the workspace strip. An empty value is an
   * explicit isolated workspace and must not fall back to the active project.
   */
  workspacePath?: string;
  draft?: ChatInputDraftRegistration;
  onDraftConsumed?: (id: number) => void;
  onSubmit?: (submission: ChatInputSubmission) => void | Promise<void>;
  /**
   * Group chat member mention (`@@`, R-GC-15/16) host. When present, a `@@`
   * mention opens the group member picker instead of the file/session picker.
   * `members` feed the picker; `onMentionSelect` records the chosen member
   * (the composer insertion of the `@name` token is owned by ChatInput).
   */
  groupChatMention?: {
    members: GroupChatMember[];
    onMentionSelect: (target: GroupChatActor) => void;
  };
}

export async function submitThroughChatInputRegistration(
  registration: ChatInputRegistration | undefined,
  submission: ChatInputSubmission,
  submitToSession: () => Promise<void>,
): Promise<'registered' | 'session'> {
  if (registration?.onSubmit) {
    await registration.onSubmit(submission);
    return 'registered';
  }

  await submitToSession();
  return 'session';
}

/**
 * Group chat registration (R-GC-18, contract §2.4, P1-3 file ownership).
 *
 * The group chat panel reuses the standard ChatInput and routes submissions
 * through registration.onSubmit to groupChatStore.sendMessage.
 */
export interface GroupChatRegistration {
  roomId: string;                    // passed by the group chat panel
  onSubmit: (text: string, author: GroupChatActor, mentionTargets: GroupChatActor[], urgent?: boolean) => void;  // P2-4 adds urgent
}
