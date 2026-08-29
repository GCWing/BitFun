export class ThinkingPresentationPolicy {
  static readonly RUNNING_DOT_COUNT: number = 3;
  static readonly RUNNING_DOT_WIDTH: number = 6;
  static readonly RUNNING_DOT_GAP: number = 6;

  static runningDotsWidth(): number {
    return ThinkingPresentationPolicy.RUNNING_DOT_WIDTH * ThinkingPresentationPolicy.RUNNING_DOT_COUNT +
      ThinkingPresentationPolicy.RUNNING_DOT_GAP * (ThinkingPresentationPolicy.RUNNING_DOT_COUNT - 1);
  }

  static isRunning(status: string): boolean {
    const normalized = (status || '').toLowerCase();
    return normalized === 'active' || normalized === 'running';
  }

  static shouldRender(status: string, text: string): boolean {
    if (ThinkingPresentationPolicy.isRunning(status)) {
      return true;
    }
    return text.trim().length > 0;
  }

  static defaultExpanded(isRunning: boolean, keepExpandedWhenDone: boolean): boolean {
    return isRunning || keepExpandedWhenDone;
  }

  static allowsToggle(hasBody: boolean): boolean {
    return hasBody;
  }

  /**
   * Option 2: the live thinking block starts expanded. The user may collapse it
   * while it streams. Completing always folds it back unless keepExpandedWhenDone.
   */
  static expandedForStatus(
    isRunning: boolean,
    keepExpandedWhenDone: boolean,
    userToggled: boolean,
    currentlyExpanded: boolean
  ): boolean {
    if (!isRunning) {
      return keepExpandedWhenDone;
    }
    if (userToggled) {
      return currentlyExpanded;
    }
    return true;
  }

  static liveThinkingStatus(messageStatus: string, hasLaterOutput: boolean): string {
    if (hasLaterOutput) {
      return 'done';
    }
    return messageStatus;
  }

  static mergedText(parts: string[]): string {
    const kept: string[] = [];
    parts.forEach((part: string) => {
      const text = (part || '').trim();
      if (text.length > 0) {
        kept.push(text);
      }
    });
    return kept.join('\n\n');
  }

  static mergedStatusValue(statuses: string[], streamings: boolean[]): string {
    const count = statuses.length > streamings.length ? statuses.length : streamings.length;
    for (let index = count - 1; index >= 0; index--) {
      const streaming = index < streamings.length ? streamings[index] : false;
      const status = index < statuses.length ? statuses[index] : '';
      if (streaming || ThinkingPresentationPolicy.isRunning(status)) {
        return status.length > 0 ? status : 'active';
      }
    }
    if (statuses.length > 0 && statuses[statuses.length - 1].length > 0) {
      return statuses[statuses.length - 1];
    }
    return 'done';
  }

  static mergedStreaming(streamings: boolean[]): boolean {
    return streamings.some((streaming: boolean) => streaming);
  }
}
