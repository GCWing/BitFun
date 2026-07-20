import { MarkdownContent } from './MarkdownContent';

const STREAMING_MARKDOWN_CACHE: Map<string, string> = new Map<string, string>();

@Component
export struct StreamingMarkdownContent {
  @Prop @Watch('handleTextChanged') text: string = '';
  @Prop @Watch('handleTextChanged') active: boolean = false;
  @Prop @Watch('handleTextChanged') streamKey: string = '';
  onCopyText: (text: string) => void = (_text: string) => {};
  @State renderedText: string = '';
  private targetText: string = '';
  private timerId: number = 0;
  private frameIntervalMs: number = 40;
  private revealDurationMs: number = 1400;
  private cacheWriteInterval: number = 4;
  private frameCount: number = 0;

  aboutToAppear(): void {
    this.targetText = this.text;
    this.renderedText = this.initialRenderedText();
    if (this.active && this.text.length > 0) {
      this.startTimer();
    }
  }

  aboutToDisappear(): void {
    this.saveRenderedText();
    this.clearTimer();
  }

  build() {
    MarkdownContent({
      text: this.renderedText,
      onCopyText: (_body: string) => {
        this.onCopyText(this.text);
      }
    })
  }

  private handleTextChanged(): void {
    if (!this.active) {
      this.clearTimer();
      this.targetText = this.text;
      this.renderedText = this.text;
      this.saveRenderedText();
      return;
    }
    const cached = this.cachedRenderedText();
    if (cached.length > this.renderedText.length && this.text.startsWith(cached)) {
      this.renderedText = cached;
    }
    if (this.text === this.renderedText) {
      this.clearTimer();
      this.targetText = this.text;
      this.saveRenderedText();
      return;
    }
    if (this.text.length < this.renderedText.length || this.text.length < this.targetText.length) {
      this.clearTimer();
      this.saveRenderedText();
      if (this.renderedText.length < this.targetText.length) {
        this.startTimer();
      }
      return;
    }
    if (!this.text.startsWith(this.renderedText) && this.renderedText.length > 0) {
      this.clearTimer();
      this.saveRenderedText();
      return;
    }
    this.targetText = this.text;
    this.startTimer();
  }

  private startTimer(): void {
    if (this.timerId !== 0) {
      return;
    }
    this.frameCount = 0;
    this.timerId = setInterval(() => {
      if (this.renderedText.length >= this.targetText.length) {
        this.clearTimer();
        return;
      }
      const remaining = this.targetText.length - this.renderedText.length;
      const totalFrames = Math.max(1, Math.floor(this.revealDurationMs / this.frameIntervalMs));
      const step = Math.max(1, Math.ceil(remaining / totalFrames));
      this.renderedText = this.targetText.slice(0, Math.min(this.targetText.length, this.renderedText.length + step));
      this.frameCount += 1;
      if (this.frameCount % this.cacheWriteInterval === 0 || this.renderedText.length >= this.targetText.length) {
        this.saveRenderedText();
      }
    }, this.frameIntervalMs);
  }

  private clearTimer(): void {
    if (this.timerId !== 0) {
      clearInterval(this.timerId);
      this.timerId = 0;
    }
  }

  private initialRenderedText(): string {
    if (!this.active) {
      return this.text;
    }
    const cached = this.cachedRenderedText();
    if (cached.length > 0 && this.text.startsWith(cached)) {
      return cached;
    }
    if (cached.length > 0) {
      return cached;
    }
    return '';
  }

  private cachedRenderedText(): string {
    const key = this.normalizedStreamKey();
    if (key.length === 0) {
      return '';
    }
    return STREAMING_MARKDOWN_CACHE.get(key) || '';
  }

  private saveRenderedText(): void {
    const key = this.normalizedStreamKey();
    if (key.length === 0) {
      return;
    }
    STREAMING_MARKDOWN_CACHE.set(key, this.renderedText);
  }

  private normalizedStreamKey(): string {
    return this.streamKey.trim();
  }

}
