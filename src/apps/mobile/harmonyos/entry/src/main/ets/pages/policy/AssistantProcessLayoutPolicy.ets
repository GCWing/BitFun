export class AssistantProcessLayoutPolicy {
  /**
   * Streaming used to pin the current thinking block after tools so the
   * viewport could chase it. That inverts chronology: a Task card appears
   * above the thought that produced it. Keep thinking in document order.
   */
  static pinsThinkingAfterTools(): boolean {
    return false;
  }

  static omitsInFlowThinkingWhileStreaming(): boolean {
    return false;
  }

  static shouldRenderActivityThinking(hasContent: boolean): boolean {
    return hasContent;
  }

  static processBlockOrder(
    thinkingText: string,
    hasStructuredThinking: boolean,
    visibleText: string,
    showTyping: boolean,
    toolCount: number
  ): string[] {
    const order: string[] = [];
    const hasLeadingThinking = thinkingText.trim().length > 0 && !hasStructuredThinking;
    if (hasLeadingThinking || hasStructuredThinking) {
      order.push('thinking');
    }
    if (toolCount > 0) {
      order.push('tools');
    }
    if (showTyping) {
      order.push('typing');
    }
    if (visibleText.trim().length > 0 && visibleText.trim() !== thinkingText.trim()) {
      order.push('text');
    }
    return order;
  }

  static activityGroupBlockOrder(hasThinking: boolean, toolCount: number): string[] {
    const order: string[] = [];
    if (hasThinking) {
      order.push('thinking');
    }
    if (toolCount > 0) {
      order.push('tools');
    }
    return order;
  }
}
