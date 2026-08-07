export interface ToolFileInputPayload {
  path?: string;
  file_path?: string;
  filePath?: string;
}

export class ToolFileReference {
  readonly path: string;
  readonly label: string;

  constructor(path: string, label: string) {
    this.path = path;
    this.label = label;
  }
}

export class ToolFileReferenceResolver {
  static resolve(
    toolName: string,
    toolInput?: Object,
    inputPreview: string = ''
  ): ToolFileReference | undefined {
    if (!ToolFileReferenceResolver.supportsTool(toolName)) {
      return undefined;
    }
    const payload = ToolFileReferenceResolver.payload(toolInput, inputPreview);
    const path = (payload.file_path || payload.filePath || payload.path || '').trim();
    if (path.length === 0 || path.endsWith('/') || path.endsWith('\\')) {
      return undefined;
    }
    return new ToolFileReference(path, ToolFileReferenceResolver.basename(path));
  }

  private static payload(toolInput?: Object, inputPreview: string = ''): ToolFileInputPayload {
    if (toolInput) {
      try {
        return JSON.parse(JSON.stringify(toolInput)) as ToolFileInputPayload;
      } catch (_err) {
      }
    }
    const preview = inputPreview.trim();
    if (preview.length > 0 && preview.indexOf('{') === 0) {
      try {
        return JSON.parse(preview) as ToolFileInputPayload;
      } catch (_err) {
      }
    }
    return {};
  }

  private static supportsTool(toolName: string): boolean {
    const normalized = toolName.replace(/[\s-]/g, '_').toLowerCase();
    return '|read|read_file|write|write_file|create|create_file|edit|edit_file|file_edit|strreplace|str_replace|str_replace_editor|replace|replace_file|update_file|'.indexOf(`|${normalized}|`) >= 0;
  }

  private static basename(path: string): string {
    let normalized = path.replace(/^computer:\/\//, '').replace(/^file:\/\//, '').replace(/\\/g, '/');
    const hash = normalized.lastIndexOf('#');
    if (hash > 0) {
      normalized = normalized.slice(0, hash);
    }
    normalized = normalized.replace(/:\d+(?:-\d+)?$/, '');
    const parts = normalized.split('/');
    return parts[parts.length - 1] || normalized || 'file';
  }
}
