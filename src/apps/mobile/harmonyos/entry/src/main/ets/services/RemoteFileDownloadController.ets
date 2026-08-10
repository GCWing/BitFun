import fs from '@ohos.file.fs';
import picker from '@ohos.file.picker';
import { FileInfo, ReadFileResult } from '../model/RemoteModels';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from './ConnectionErrorPolicy';
import { Encoding } from './Encoding';
import { RemoteWorkspaceFileClient } from './RemoteWorkspaceFileClient';
import { RemoteUiState } from './RemoteUiState';
import { RemoteLogger } from './RemoteLogger';

export interface RemoteFileDownloadClient extends RemoteWorkspaceFileClient {}

export interface RemoteFileDownloadScheduler {
  setTimeout(callback: () => void, delayMs: number): number;
  clearTimeout(timerId: number): void;
}

export interface RemoteFileSaver {
  save(result: ReadFileResult): Promise<string>;
}

class SystemRemoteFileDownloadScheduler implements RemoteFileDownloadScheduler {
  setTimeout(callback: () => void, delayMs: number): number {
    return setTimeout(callback, delayMs);
  }

  clearTimeout(timerId: number): void {
    clearTimeout(timerId);
  }
}

class SystemRemoteFileSaver implements RemoteFileSaver {
  async save(result: ReadFileResult): Promise<string> {
    const options = new picker.DocumentSaveOptions();
    options.newFileNames = [result.name];
    options.autoCreateEmptyFile = true;
    const uris = await new picker.DocumentViewPicker().save(options);
    const uri = uris.length > 0 ? uris[0] : '';
    if (uri.length === 0) {
      throw new Error('No destination file was selected.');
    }
    const file = fs.openSync(uri, fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
    try {
      fs.writeSync(file.fd, Encoding.base64ToBytes(result.contentBase64).buffer);
    } finally {
      fs.closeSync(file);
    }
    return uri;
  }
}

export class RemoteFileDownloadController {
  private readonly client: RemoteFileDownloadClient;
  private readonly onDownloadStatus: (downloadingFilePath: string, downloadedFilePath: string, fileDownloadStatus: string) => void;
  private readonly onClearDownloadingFilePath: () => void;
  private readonly onStatusText: (statusText: string) => void;
  private readonly onBusy: (isBusy: boolean) => void;
  private readonly clearDelayMs: number;
  private readonly scheduler: RemoteFileDownloadScheduler;
  private readonly fileSaver: RemoteFileSaver;
  private downloadingFilePath: string = '';
  private downloadedFilePath: string = '';
  private fileDownloadStatus: string = '';
  private clearTimerId: number = 0;

  constructor(
    client: RemoteFileDownloadClient,
    onDownloadStatus: (downloadingFilePath: string, downloadedFilePath: string, fileDownloadStatus: string) => void,
    onClearDownloadingFilePath: () => void,
    onStatusText: (statusText: string) => void,
    onBusy: (isBusy: boolean) => void,
    clearDelayMs: number = 2500,
    scheduler: RemoteFileDownloadScheduler = new SystemRemoteFileDownloadScheduler(),
    fileSaver: RemoteFileSaver = new SystemRemoteFileSaver()
  ) {
    this.client = client;
    this.onDownloadStatus = onDownloadStatus;
    this.onClearDownloadingFilePath = onClearDownloadingFilePath;
    this.onStatusText = onStatusText;
    this.onBusy = onBusy;
    this.clearDelayMs = clearDelayMs;
    this.scheduler = scheduler;
    this.fileSaver = fileSaver;
  }

  async download(path: string, sessionId: string, isBusy: boolean, remoteAvailable: boolean): Promise<void> {
    const remotePath = RemoteUiState.normalizeRemoteFilePath(path);
    if (sessionId.length === 0 || remotePath.length === 0 || isBusy || !remoteAvailable) {
      return;
    }
    this.cancelClearTimer();
    try {
      RemoteLogger.info(`file_download start session=${sessionId.length > 0 ? 'set' : 'none'}`);
      this.onBusy(true);
      this.setDownloadStatus(path, '', RemoteI18n.t('status.fileInfo'));
      const info = await this.client.getFileInfo(remotePath, sessionId);
      RemoteLogger.info(`file_download metadata name=${info.name} size=${info.size} mime=${info.mimeType}`);
      this.setDownloadStatus(path, '', RemoteI18n.f('status.prepareDownload', RemoteUiState.formatBytes(info.size)));
      const result: ReadFileResult = await this.client.readFile(remotePath, sessionId, (downloaded: number, total: number) => {
        this.setDownloadStatus(
          path,
          '',
          `${RemoteUiState.formatBytes(downloaded)} / ${RemoteUiState.formatBytes(total)}`
        );
      });
      RemoteLogger.info(`file_download data_ready name=${result.name} size=${result.size}`);
      await this.fileSaver.save(result);
      RemoteLogger.info(`file_download saved name=${result.name} size=${result.size}`);
      this.setDownloadStatus(
        path,
        path,
        RemoteI18n.f2('status.downloadDone', result.name, RemoteUiState.formatBytes(result.size))
      );
      this.onStatusText(this.fileDownloadStatus);
    } catch (err) {
      const errorText = ConnectionErrorPolicy.errorText(err);
      RemoteLogger.error(`file_download failed message=${errorText}`);
      this.setDownloadStatus(this.downloadingFilePath, this.downloadedFilePath, errorText);
      this.onStatusText(this.fileDownloadStatus);
    } finally {
      this.onBusy(false);
      this.scheduleClearDownloadingFilePath();
    }
  }

  clear(): void {
    this.cancelClearTimer();
    this.setDownloadStatus('', '', '');
  }

  cancel(): void {
    this.cancelClearTimer();
  }

  isClearPending(): boolean {
    return this.clearTimerId !== 0;
  }

  currentStatusText(): string {
    return this.fileDownloadStatus;
  }

  private setDownloadStatus(downloadingFilePath: string, downloadedFilePath: string, fileDownloadStatus: string): void {
    this.downloadingFilePath = downloadingFilePath;
    this.downloadedFilePath = downloadedFilePath;
    this.fileDownloadStatus = fileDownloadStatus;
    this.onDownloadStatus(downloadingFilePath, downloadedFilePath, fileDownloadStatus);
  }

  private scheduleClearDownloadingFilePath(): void {
    this.cancelClearTimer();
    this.clearTimerId = this.scheduler.setTimeout(() => {
      this.clearTimerId = 0;
      this.downloadingFilePath = '';
      this.onClearDownloadingFilePath();
    }, this.clearDelayMs);
  }

  private cancelClearTimer(): void {
    if (this.clearTimerId !== 0) {
      this.scheduler.clearTimeout(this.clearTimerId);
      this.clearTimerId = 0;
    }
  }
}
