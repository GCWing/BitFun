import { FilePreviewTargetContext } from '../../model/FilePreviewTarget';
import { FileReferenceKind, FileTargetResolver } from './FileTargetResolver';
import {
  MarkdownParser,
  ParsedMarkdownBlock,
  ParsedMarkdownInline,
  ParsedMarkdownListItem,
  ParsedMarkdownTableCell,
  ParsedMarkdownTableRow
} from '../../services/MarkdownParser';

export class MessageFileReference {
  readonly id: string;
  readonly path: string;
  readonly remotePath: string;
  readonly label: string;

  constructor(id: string, path: string, remotePath: string, label: string) {
    this.id = id;
    this.path = path;
    this.remotePath = remotePath;
    this.label = label;
  }
}

export class MessageFileReferenceProjectionCache {
  private source: string = '';
  private references: MessageFileReference[] = [];
  private initialized: boolean = false;

  referencesFor(source: string): MessageFileReference[] {
    if (this.initialized && this.source === source) {
      return this.references;
    }
    this.source = source;
    this.references = MessageFileReferenceProjector.project(source);
    this.initialized = true;
    return this.references;
  }
}

export class MessageFileReferenceProjector {
  private static readonly CONTEXT: FilePreviewTargetContext =
    new FilePreviewTargetContext('_message_projection_', '', 0);

  static project(source: string, limit: number = 4): MessageFileReference[] {
    const references: MessageFileReference[] = [];
    const seenRemotePaths = new Set<string>();
    MarkdownParser.parse(source).forEach((block: ParsedMarkdownBlock) => {
      MessageFileReferenceProjector.collectInlines(block.inlines, references, seenRemotePaths, limit);
      block.items.forEach((item: ParsedMarkdownListItem) => {
        MessageFileReferenceProjector.collectInlines(item.inlines, references, seenRemotePaths, limit);
      });
      block.tableRows.forEach((row: ParsedMarkdownTableRow) => {
        row.cells.forEach((cell: ParsedMarkdownTableCell) => {
          MessageFileReferenceProjector.collectInlines(cell.inlines, references, seenRemotePaths, limit);
        });
      });
    });
    return references;
  }

  private static collectInlines(
    inlines: ParsedMarkdownInline[],
    references: MessageFileReference[],
    seenRemotePaths: Set<string>,
    limit: number
  ): void {
    inlines.forEach((inline: ParsedMarkdownInline) => {
      if (references.length >= limit) {
        return;
      }
      if (inline.type === 'link') {
        MessageFileReferenceProjector.add(inline.url, references, seenRemotePaths);
        return;
      }
      if (inline.type === 'code') {
        return;
      }
      const found: string[] = inline.text.match(/computer:\/\/[^\s)\]}>"']+/g) || [];
      found.forEach((reference: string) => {
        if (references.length < limit) {
          MessageFileReferenceProjector.add(reference, references, seenRemotePaths);
        }
      });
    });
  }

  private static add(
    reference: string,
    references: MessageFileReference[],
    seenRemotePaths: Set<string>
  ): void {
    if (reference.trim().toLowerCase().indexOf('computer://') !== 0) {
      return;
    }
    const resolution = FileTargetResolver.resolve(reference, '', MessageFileReferenceProjector.CONTEXT);
    if (resolution.kind !== FileReferenceKind.RemoteWorkspaceFile || !resolution.target ||
      seenRemotePaths.has(resolution.target.remotePath)) {
      return;
    }
    seenRemotePaths.add(resolution.target.remotePath);
    references.push(new MessageFileReference(
      `file-${references.length}-${resolution.target.remotePath}`,
      resolution.target.rawReference,
      resolution.target.remotePath,
      resolution.target.displayName
    ));
  }
}
