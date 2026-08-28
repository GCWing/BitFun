import { FilePreviewRequest, FilePreviewTargetContext } from '../../model/FilePreviewTarget';
import { SessionSummary } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { FileReferenceKind, FileTargetResolver } from '../policy/FileTargetResolver';
import { RemoteWorkspaceFileClient } from '../../services/RemoteWorkspaceFileClient';
import { AppRoute, AppRouteContract } from '../navigation/AppRouteContract';
import { FilePreviewState } from '../state/FilePreviewState';
import { RemoteFilePreviewController } from './RemoteFilePreviewController';

export interface FilePreviewControllerHooks {
  readonly remoteAvailable: () => boolean;
  readonly activeSession: () => SessionSummary;
  readonly workspacePath: () => string;
  readonly openExternalLink: (reference: string) => Promise<boolean>;
  readonly onGeneralStatus: (statusText: string) => void;
  readonly onRemoteStatus: (statusText: string) => void;
}

/** Owns file-preview routing, target validity and the underlying remote file load. */
export class FilePreviewController {
  private readonly state: FilePreviewState;
  private readonly hooks: FilePreviewControllerHooks;
  private readonly loader: RemoteFilePreviewController;
  private controlTargetEpoch: number = 1;

  constructor(
    client: RemoteWorkspaceFileClient,
    state: FilePreviewState,
    hooks: FilePreviewControllerHooks
  ) {
    this.state = state;
    this.hooks = hooks;
    this.loader = new RemoteFilePreviewController(
      client,
      state,
      hooks.remoteAvailable,
      (): number => this.controlTargetEpoch
    );
  }

  open(route: AppRoute, request: FilePreviewRequest): void {
    const activeSession = this.hooks.activeSession();
    const context = new FilePreviewTargetContext(
      activeSession.sessionId,
      activeSession.workspacePath || this.hooks.workspacePath(),
      this.controlTargetEpoch
    );
    const resolution = FileTargetResolver.resolve(request.reference, request.label, context);
    if (resolution.kind === FileReferenceKind.HttpUrl) {
      void this.openExternalLink(route, request.reference);
      return;
    }
    if (route !== AppRoute.RemoteChat) {
      this.hooks.onGeneralStatus(RemoteI18n.t('generalChat.filePreviewUnavailable'));
      return;
    }
    if (resolution.kind !== FileReferenceKind.RemoteWorkspaceFile || !resolution.target) {
      return;
    }
    void this.loader.open(resolution.target);
  }

  close(): void {
    this.loader.close();
  }

  refresh(): void {
    void this.loader.refresh();
  }

  openLink(reference: string, label: string): void {
    this.open(AppRoute.RemoteChat, new FilePreviewRequest(reference, label));
  }

  invalidate(): void {
    this.controlTargetEpoch += 1;
    this.loader.close();
  }

  private async openExternalLink(route: AppRoute, reference: string): Promise<void> {
    const opened = await this.hooks.openExternalLink(reference);
    if (opened) {
      return;
    }
    const statusText = RemoteI18n.t('errors.operationFailed');
    if (AppRouteContract.isGeneralComposerRoute(route)) {
      this.hooks.onGeneralStatus(statusText);
      return;
    }
    this.hooks.onRemoteStatus(statusText);
  }
}
