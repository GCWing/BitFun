import { FileInfo, ReadFileChunkResult, ReadFileResult } from '../../model/RemoteModels';
import {
  FilePreviewPhase,
  FilePreviewRendererKind,
  FilePreviewState
} from '../state/FilePreviewState';
import { FilePreviewTarget } from '../../model/FilePreviewTarget';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { Encoding } from '../../services/Encoding';
import { FilePreviewErrorPolicy } from '../../services/FilePreviewErrorPolicy';
import { FilePreviewPolicy } from '../../services/FilePreviewPolicy';
import { RemoteWorkspaceFileClient } from '../../services/RemoteWorkspaceFileClient';

export class RemoteFilePreviewController {
  private readonly client: RemoteWorkspaceFileClient;
  private readonly state: FilePreviewState;
  private readonly remoteAvailable: () => boolean;
  private readonly currentControlTargetEpoch: () => number;

  constructor(
    client: RemoteWorkspaceFileClient,
    state: FilePreviewState,
    remoteAvailable: () => boolean,
    currentControlTargetEpoch: () => number
  ) {
    this.client = client;
    this.state = state;
    this.remoteAvailable = remoteAvailable;
    this.currentControlTargetEpoch = currentControlTargetEpoch;
  }

  async open(target: FilePreviewTarget): Promise<void> {
    if (!target.isValid() || !this.remoteAvailable() ||
      target.controlTargetEpoch !== this.currentControlTargetEpoch()) {
      return;
    }
    const version = this.state.begin(target);
    try {
      const info = await this.client.getFileInfo(target.remotePath, target.sessionId);
      if (!this.isCurrentTarget(version, target) || this.failIfUnavailable()) {
        return;
      }
      this.applyMetadata(info);
      const renderer = RemoteFilePreviewController.rendererFor(info.name, info.mimeType);
      this.state.rendererKind = renderer;
      if (renderer === FilePreviewRendererKind.Unsupported ||
        (renderer === FilePreviewRendererKind.Image && !FilePreviewPolicy.canPreviewImage(info.size))) {
        this.state.phase = FilePreviewPhase.Unsupported;
        return;
      }
      if (renderer === FilePreviewRendererKind.Image) {
        const image = await this.client.readFile(target.remotePath, target.sessionId);
        if (!this.isCurrentTarget(version, target) || this.failIfUnavailable()) {
          return;
        }
        if (!FilePreviewPolicy.canPreviewImage(image.size)) {
          this.state.fileName = image.name;
          this.state.fileSize = image.size;
          this.state.mimeType = image.mimeType;
          this.state.phase = FilePreviewPhase.Unsupported;
          return;
        }
        this.applyImage(image);
        return;
      }
      const limit = FilePreviewPolicy.textReadLimit(info.size);
      const chunk = await this.client.readFileChunk(target.remotePath, 0, limit, target.sessionId);
      if (!this.isCurrentTarget(version, target) || this.failIfUnavailable()) {
        return;
      }
      this.applyText(chunk, info.size > limit);
    } catch (err) {
      if (!this.isCurrentTarget(version, target)) {
        return;
      }
      this.state.phase = FilePreviewPhase.Error;
      if (this.remoteAvailable()) {
        const failure = FilePreviewErrorPolicy.resolve(err);
        this.state.errorText = failure.text;
        this.state.errorRetryable = failure.retryable;
      } else {
        this.state.errorText = RemoteI18n.t('filePreview.offline');
        this.state.errorRetryable = true;
      }
    }
  }

  async refresh(): Promise<void> {
    if (!this.state.visible || !this.state.target.isValid()) {
      return;
    }
    await this.open(this.state.target);
  }

  close(): void {
    this.state.close();
  }

  private isCurrentTarget(version: number, target: FilePreviewTarget): boolean {
    return this.state.isCurrent(version, target) &&
      target.controlTargetEpoch === this.currentControlTargetEpoch();
  }

  private failIfUnavailable(): boolean {
    if (this.remoteAvailable()) {
      return false;
    }
    this.state.phase = FilePreviewPhase.Error;
    this.state.errorText = RemoteI18n.t('filePreview.offline');
    this.state.errorRetryable = true;
    return true;
  }

  private applyMetadata(info: FileInfo): void {
    this.state.fileName = info.name;
    this.state.fileSize = info.size;
    this.state.mimeType = info.mimeType;
  }

  private applyText(chunk: ReadFileChunkResult, truncated: boolean): void {
    const bytes = Encoding.base64ToBytes(chunk.contentBase64);
    if (RemoteFilePreviewController.isBinary(bytes)) {
      this.state.phase = FilePreviewPhase.Unsupported;
      this.state.rendererKind = FilePreviewRendererKind.Unsupported;
      this.state.loadedBytes = bytes.length;
      return;
    }
    const text = Encoding.bytesToUtf8(bytes);
    if (RemoteFilePreviewController.isSuspiciousText(bytes, text)) {
      this.state.phase = FilePreviewPhase.Unsupported;
      this.state.rendererKind = FilePreviewRendererKind.Unsupported;
      this.state.loadedBytes = bytes.length;
      return;
    }
    this.state.textContent = text;
    this.state.contentBase64 = '';
    this.state.loadedBytes = bytes.length;
    this.state.truncated = truncated || chunk.totalSize > bytes.length;
    this.state.phase = FilePreviewPhase.Ready;
  }

  private applyImage(result: ReadFileResult): void {
    this.state.fileName = result.name;
    this.state.fileSize = result.size;
    this.state.mimeType = result.mimeType;
    this.state.contentBase64 = result.contentBase64;
    this.state.textContent = '';
    this.state.loadedBytes = result.size;
    this.state.truncated = false;
    this.state.phase = FilePreviewPhase.Ready;
  }

  static rendererFor(name: string, mimeType: string): FilePreviewRendererKind {
    const mime = mimeType.toLowerCase();
    const extension = RemoteFilePreviewController.extension(name);
    if (mime.indexOf('image/') === 0 && extension !== 'svg') {
      return FilePreviewRendererKind.Image;
    }
    if (mime === 'text/markdown' || extension === 'md' || extension === 'mdx') {
      return FilePreviewRendererKind.Markdown;
    }
    if (mime.indexOf('text/') === 0 || mime === 'application/json' || mime === 'application/xml' ||
      RemoteFilePreviewController.isTextExtension(extension) ||
      RemoteFilePreviewController.isTextFileName(name)) {
      return FilePreviewRendererKind.Text;
    }
    return FilePreviewRendererKind.Unsupported;
  }

  private static extension(name: string): string {
    const fileName = name.replace(/\\/g, '/').split('/').pop() || '';
    const dot = fileName.lastIndexOf('.');
    return dot >= 0 ? fileName.slice(dot + 1).toLowerCase() : '';
  }

  private static isTextExtension(extension: string): boolean {
    return '|js|jsx|ts|tsx|mjs|cjs|py|rs|go|java|kt|c|cpp|h|hpp|cs|rb|php|swift|vue|svelte|css|scss|less|json|jsonc|yaml|yml|toml|xml|csv|tsv|txt|log|sh|bash|zsh|fish|ps1|bat|cmd|sql|graphql|gql|proto|lock|env|ini|cfg|conf|ets|gitignore|editorconfig|'.indexOf(`|${extension}|`) >= 0;
  }

  private static isTextFileName(name: string): boolean {
    const fileName = name.replace(/\\/g, '/').split('/').pop()?.toLowerCase() || '';
    return fileName === 'dockerfile' || fileName === 'makefile' || fileName === 'justfile' ||
      fileName === 'gemfile' || fileName === 'rakefile' || fileName === 'procfile' ||
      fileName === 'license' || fileName === 'readme' || fileName === 'changelog';
  }

  private static isBinary(bytes: Uint8Array): boolean {
    const limit = Math.min(bytes.length, 4096);
    for (let index = 0; index < limit; index++) {
      if (bytes[index] === 0) {
        return true;
      }
    }
    return false;
  }

  private static isSuspiciousText(bytes: Uint8Array, text: string): boolean {
    if (!RemoteFilePreviewController.isValidUtf8(bytes)) {
      return true;
    }
    if (text.indexOf('\uFFFD') >= 0) {
      return true;
    }
    const limit = Math.min(bytes.length, 4096);
    let controls = 0;
    for (let index = 0; index < limit; index++) {
      const value = bytes[index];
      if (value < 32 && value !== 9 && value !== 10 && value !== 13) {
        controls += 1;
      }
    }
    return limit > 0 && controls / limit > 0.02;
  }

  private static isValidUtf8(bytes: Uint8Array): boolean {
    let index = 0;
    while (index < bytes.length) {
      const first = bytes[index];
      if (first <= 0x7F) {
        index += 1;
        continue;
      }
      let continuationCount = 0;
      let minimumFirst = 0x80;
      let maximumFirst = 0xBF;
      if (first >= 0xC2 && first <= 0xDF) {
        continuationCount = 1;
      } else if (first === 0xE0) {
        continuationCount = 2;
        minimumFirst = 0xA0;
      } else if (first >= 0xE1 && first <= 0xEC) {
        continuationCount = 2;
      } else if (first === 0xED) {
        continuationCount = 2;
        maximumFirst = 0x9F;
      } else if (first >= 0xEE && first <= 0xEF) {
        continuationCount = 2;
      } else if (first === 0xF0) {
        continuationCount = 3;
        minimumFirst = 0x90;
      } else if (first >= 0xF1 && first <= 0xF3) {
        continuationCount = 3;
      } else if (first === 0xF4) {
        continuationCount = 3;
        maximumFirst = 0x8F;
      } else {
        return false;
      }
      if (index + continuationCount >= bytes.length) {
        return false;
      }
      const second = bytes[index + 1];
      if (second < minimumFirst || second > maximumFirst) {
        return false;
      }
      for (let offset = 2; offset <= continuationCount; offset++) {
        const continuation = bytes[index + offset];
        if (continuation < 0x80 || continuation > 0xBF) {
          return false;
        }
      }
      index += continuationCount + 1;
    }
    return true;
  }
}
