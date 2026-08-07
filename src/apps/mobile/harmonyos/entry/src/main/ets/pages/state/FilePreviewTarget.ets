export class FilePreviewTarget {
  readonly kind: string;
  readonly rawReference: string;
  readonly remotePath: string;
  readonly displayName: string;
  readonly sessionId: string;
  readonly workspacePath: string;
  readonly controlTargetEpoch: number;
  readonly lineStart: number;
  readonly lineEnd: number;

  constructor(
    rawReference: string,
    remotePath: string,
    displayName: string,
    sessionId: string,
    workspacePath: string,
    controlTargetEpoch: number,
    lineStart: number = 0,
    lineEnd: number = 0
  ) {
    this.kind = 'remote_workspace_file';
    this.rawReference = rawReference;
    this.remotePath = remotePath;
    this.displayName = displayName;
    this.sessionId = sessionId;
    this.workspacePath = workspacePath;
    this.controlTargetEpoch = controlTargetEpoch;
    this.lineStart = lineStart;
    this.lineEnd = lineEnd;
  }

  static empty(): FilePreviewTarget {
    return new FilePreviewTarget('', '', '', '', '', 0);
  }

  isValid(): boolean {
    return this.remotePath.length > 0 && this.sessionId.length > 0;
  }
}

export class FilePreviewTargetContext {
  readonly sessionId: string;
  readonly workspacePath: string;
  readonly controlTargetEpoch: number;

  constructor(sessionId: string, workspacePath: string, controlTargetEpoch: number) {
    this.sessionId = sessionId;
    this.workspacePath = workspacePath;
    this.controlTargetEpoch = controlTargetEpoch;
  }
}

export class FilePreviewRequest {
  readonly reference: string;
  readonly label: string;

  constructor(reference: string, label: string) {
    this.reference = reference;
    this.label = label;
  }
}
