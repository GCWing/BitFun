import { FilePreviewTarget } from '../../model/FilePreviewTarget';

export enum FilePreviewPhase {
  Idle = 'idle',
  Loading = 'loading',
  Ready = 'ready',
  Unsupported = 'unsupported',
  Error = 'error'
}

export enum FilePreviewRendererKind {
  Text = 'text',
  Markdown = 'markdown',
  Image = 'image',
  Unsupported = 'unsupported'
}

@ObservedV2
export class FilePreviewState {
  @Trace visible: boolean = false;
  @Trace phase: FilePreviewPhase = FilePreviewPhase.Idle;
  @Trace target: FilePreviewTarget = FilePreviewTarget.empty();
  @Trace fileName: string = '';
  @Trace mimeType: string = '';
  @Trace fileSize: number = 0;
  @Trace rendererKind: FilePreviewRendererKind = FilePreviewRendererKind.Unsupported;
  @Trace textContent: string = '';
  @Trace contentBase64: string = '';
  @Trace truncated: boolean = false;
  @Trace loadedBytes: number = 0;
  @Trace errorText: string = '';
  @Trace errorRetryable: boolean = true;
  @Trace requestVersion: number = 0;
  scrollOffsetX: number = 0;
  scrollOffsetY: number = 0;
  hasRecordedScroll: boolean = false;

  begin(target: FilePreviewTarget): number {
    this.requestVersion += 1;
    this.visible = true;
    this.phase = FilePreviewPhase.Loading;
    this.target = target;
    this.fileName = target.displayName;
    this.mimeType = '';
    this.fileSize = 0;
    this.rendererKind = FilePreviewRendererKind.Unsupported;
    this.textContent = '';
    this.contentBase64 = '';
    this.truncated = false;
    this.loadedBytes = 0;
    this.errorText = '';
    this.errorRetryable = true;
    this.resetScroll();
    return this.requestVersion;
  }

  close(): void {
    this.requestVersion += 1;
    this.visible = false;
    this.phase = FilePreviewPhase.Idle;
    this.target = FilePreviewTarget.empty();
    this.fileName = '';
    this.mimeType = '';
    this.fileSize = 0;
    this.rendererKind = FilePreviewRendererKind.Unsupported;
    this.textContent = '';
    this.contentBase64 = '';
    this.truncated = false;
    this.loadedBytes = 0;
    this.errorText = '';
    this.errorRetryable = true;
    this.resetScroll();
  }

  isCurrent(version: number, target: FilePreviewTarget): boolean {
    return this.visible && this.requestVersion === version &&
      this.target.remotePath === target.remotePath &&
      this.target.sessionId === target.sessionId &&
      this.target.controlTargetEpoch === target.controlTargetEpoch;
  }

  recordScroll(xOffset: number, yOffset: number): void {
    this.scrollOffsetX = Math.max(0, xOffset);
    this.scrollOffsetY = Math.max(0, yOffset);
    this.hasRecordedScroll = true;
  }

  initialScrollX(): number {
    return this.hasRecordedScroll ? this.scrollOffsetX : 0;
  }

  initialScrollY(lineHeight: number = 19, contextLines: number = 2): number {
    if (this.hasRecordedScroll) {
      return this.scrollOffsetY;
    }
    if (this.target.lineStart <= 1) {
      return 0;
    }
    return Math.max(0, this.target.lineStart - 1 - contextLines) * lineHeight;
  }

  private resetScroll(): void {
    this.scrollOffsetX = 0;
    this.scrollOffsetY = 0;
    this.hasRecordedScroll = false;
  }
}
