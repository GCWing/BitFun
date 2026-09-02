export type ChatInputTarget = 'main' | 'btw';

export function resolveChatInputTargetSessionId(params: {
  currentSessionId: string | null;
  inputTarget: ChatInputTarget;
  activeBtwSessionId?: string;
}): string | null {
  if (params.inputTarget === 'btw' && params.activeBtwSessionId) {
    return params.activeBtwSessionId;
  }
  return params.currentSessionId;
}
