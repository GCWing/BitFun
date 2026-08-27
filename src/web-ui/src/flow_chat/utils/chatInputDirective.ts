import type { ChatInputDirectiveId } from './chatInputMode';

export interface ChatInputTurnDirective {
  id: ChatInputDirectiveId;
  instruction: string;
}

const DIRECTIVES: Record<ChatInputDirectiveId, ChatInputTurnDirective> = {
  Plan: {
    id: 'Plan',
    instruction:
      'For this task, stay in planning mode: clarify material uncertainties, inspect the relevant context, and produce an actionable plan. Do not implement changes unless the user explicitly asks you to proceed.',
  },
  Multitask: {
    id: 'Multitask',
    instruction:
      'For this task, identify independent workstreams and run them in parallel when that materially helps. Integrate and verify the results before responding.',
  },
};

export function chatInputTurnDirective(
  id: ChatInputDirectiveId,
): ChatInputTurnDirective {
  return DIRECTIVES[id];
}

export function applyChatInputTurnDirective(
  message: string,
  directive: ChatInputTurnDirective | null | undefined,
): string {
  if (!directive) return message;

  return [
    `<task-directive name="${directive.id}">`,
    directive.instruction,
    '</task-directive>',
    '',
    message,
  ].join('\n');
}
