import { FilePreviewTarget, FilePreviewTargetContext } from '../../model/FilePreviewTarget';
import { RemoteUiState } from '../../services/RemoteUiState';

export enum FileReferenceKind {
  RemoteWorkspaceFile = 'remote_workspace_file',
  HttpUrl = 'http_url',
  Anchor = 'anchor',
  UnsupportedScheme = 'unsupported_scheme',
  Invalid = 'invalid'
}

export class FileReferenceResolution {
  readonly kind: FileReferenceKind;
  readonly target?: FilePreviewTarget;

  constructor(kind: FileReferenceKind, target?: FilePreviewTarget) {
    this.kind = kind;
    this.target = target;
  }
}

export class FileTargetResolver {
  static matchesRemotePath(reference: string, remotePath: string): boolean {
    const raw = FileTargetResolver.cleanReference(reference);
    if (raw.length === 0 || remotePath.length === 0 || raw.indexOf('#') === 0) {
      return false;
    }
    const range = FileTargetResolver.extractLineRange(raw);
    if (FileTargetResolver.hasUnsupportedScheme(range.path)) {
      return false;
    }
    return RemoteUiState.normalizeRemoteFilePath(range.path) === remotePath;
  }

  static resolve(reference: string, label: string, context: FilePreviewTargetContext): FileReferenceResolution {
    const raw = FileTargetResolver.cleanReference(reference);
    if (raw.length === 0) {
      return new FileReferenceResolution(FileReferenceKind.Invalid);
    }
    const lower = raw.toLowerCase();
    if (lower.indexOf('http://') === 0 || lower.indexOf('https://') === 0) {
      return new FileReferenceResolution(FileReferenceKind.HttpUrl);
    }
    if (raw.indexOf('#') === 0) {
      return new FileReferenceResolution(FileReferenceKind.Anchor);
    }
    const range = FileTargetResolver.extractLineRange(raw);
    if (FileTargetResolver.hasUnsupportedScheme(range.path)) {
      return new FileReferenceResolution(FileReferenceKind.UnsupportedScheme);
    }

    const remotePath: string = RemoteUiState.normalizeRemoteFilePath(range.path);
    if (remotePath.length === 0 || remotePath === '/') {
      return new FileReferenceResolution(FileReferenceKind.Invalid);
    }
    const displayName = label.trim().length > 0 ? label.trim() : FileTargetResolver.basename(remotePath);
    return new FileReferenceResolution(
      FileReferenceKind.RemoteWorkspaceFile,
      new FilePreviewTarget(
        raw,
        remotePath,
        displayName,
        context.sessionId,
        context.workspacePath,
        context.controlTargetEpoch,
        range.start,
        range.end
      )
    );
  }

  private static extractLineRange(reference: string): FileTargetLineRange {
    const hashIndex = reference.lastIndexOf('#');
    if (hashIndex > 0) {
      const parsed = FileTargetResolver.parseLineMarker(reference.slice(hashIndex + 1));
      if (parsed.start > 0) {
        return new FileTargetLineRange(reference.slice(0, hashIndex), parsed.start, parsed.end);
      }
    }
    const colon = reference.match(/^(.+):(\d+)(?:-(\d+))?$/);
    if (colon && !FileTargetResolver.isWindowsDrivePrefix(colon[1])) {
      return new FileTargetLineRange(
        colon[1],
        Number.parseInt(colon[2]),
        colon[3] ? Number.parseInt(colon[3]) : 0
      );
    }
    return new FileTargetLineRange(reference, 0, 0);
  }

  private static parseLineMarker(marker: string): FileTargetLineRange {
    const match = marker.match(/^L?(\d+)(?:-L?(\d+))?$/i);
    if (!match) {
      return new FileTargetLineRange('', 0, 0);
    }
    return new FileTargetLineRange(
      '',
      Number.parseInt(match[1]),
      match[2] ? Number.parseInt(match[2]) : 0
    );
  }

  private static hasUnsupportedScheme(reference: string): boolean {
    const scheme = reference.match(/^([A-Za-z][A-Za-z0-9+.-]*):/);
    if (!scheme) {
      return false;
    }
    if (scheme[1].length === 1 && reference.length >= 3 &&
      (reference.charAt(2) === '/' || reference.charAt(2) === '\\')) {
      return false;
    }
    const lower = scheme[1].toLowerCase();
    return lower !== 'computer' && lower !== 'file';
  }

  private static isWindowsDrivePrefix(value: string): boolean {
    return value.length === 1 && /[A-Za-z]/.test(value);
  }

  private static cleanReference(reference: string): string {
    let clean = reference.trim();
    while (clean.length > 0) {
      const last = clean.charAt(clean.length - 1);
      if (last === ',' || last === '.' || last === ';' || last === ':' || last === ')' ||
        last === ']' || last === '}' || last === '>' || last === '，' || last === '。' ||
        last === '；' || last === '：') {
        clean = clean.slice(0, clean.length - 1);
      } else {
        break;
      }
    }
    try {
      return decodeURIComponent(clean);
    } catch (_err) {
      return clean;
    }
  }

  private static basename(path: string): string {
    const parts = path.replace(/\\/g, '/').split('/');
    return parts[parts.length - 1] || path || 'file';
  }
}

class FileTargetLineRange {
  readonly path: string;
  readonly start: number;
  readonly end: number;

  constructor(path: string, start: number, end: number) {
    this.path = path;
    this.start = start;
    this.end = end;
  }
}
