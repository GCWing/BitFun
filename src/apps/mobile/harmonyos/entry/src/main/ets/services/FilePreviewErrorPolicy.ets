import { RemoteI18n } from '../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from './ConnectionErrorPolicy';

export class FilePreviewErrorResult {
  readonly text: string;
  readonly retryable: boolean;

  constructor(text: string, retryable: boolean) {
    this.text = text;
    this.retryable = retryable;
  }
}

export class FilePreviewErrorPolicy {
  static resolve(err: Object): FilePreviewErrorResult {
    const raw = err instanceof Error ? err.message : JSON.stringify(err);
    const text = raw.toLowerCase();
    if (text.indexOf('file not found') >= 0 || text.indexOf('file does not exist') >= 0 ||
      text.indexOf('not a regular file') >= 0 || text.indexOf('no such file') >= 0) {
      return new FilePreviewErrorResult(RemoteI18n.t('filePreview.notFound'), true);
    }
    if (text.indexOf('could not be resolved') >= 0) {
      return new FilePreviewErrorResult(RemoteI18n.t('filePreview.unavailable'), true);
    }
    if (text.indexOf('access denied') >= 0 || text.indexOf('restricted') >= 0 ||
      text.indexOf('outside') >= 0) {
      return new FilePreviewErrorResult(RemoteI18n.t('filePreview.accessDenied'), false);
    }
    if (text.indexOf('file too large') >= 0 || text.indexOf('too large') >= 0) {
      return new FilePreviewErrorResult(RemoteI18n.t('filePreview.tooLarge'), false);
    }
    const connectionText = ConnectionErrorPolicy.errorText(err);
    if (connectionText !== raw || /[\u3400-\u9fff]/.test(raw)) {
      return new FilePreviewErrorResult(connectionText, true);
    }
    return new FilePreviewErrorResult(RemoteI18n.t('filePreview.loadFailed'), true);
  }

  static errorText(err: Object): string {
    return FilePreviewErrorPolicy.resolve(err).text;
  }
}
