import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import type { MarkdownSourceRange } from '@/infrastructure/markdown/rehypeSourceRange';

type SourceRegion = { start: number; end: number };
type ReferenceNode = {
  type: string;
  position?: { start: { offset?: number }; end: { offset?: number } };
  children?: ReferenceNode[];
};
const referenceTypes = new Set([
  'definition', 'linkReference', 'imageReference', 'footnoteDefinition', 'footnoteReference',
]);
const contexts = new WeakMap<ProseMirrorNode, { content: string; regions: Map<number, SourceRegion> }>();

/** Source-backed nodes share reference facts, while native editor blocks stay native. */
export function sourceBlockPreview(doc: ProseMirrorNode, pos: number, idPrefix: string): {
  content: string;
  sourceRange: MarkdownSourceRange;
} | undefined {
  let context = contexts.get(doc);
  if (!context) {
    let content = '';
    const candidates = new Map<number, SourceRegion>();
    doc.descendants((node, offset) => {
      if (node.type.name !== 'renderOnlyBlock' && node.type.name !== 'rawHtmlBlock') return;
      const source = String(node.attrs.markdown ?? node.attrs.html ?? '');
      if (content) content += '\n\n';
      const start = content.length;
      content += source;
      candidates.set(offset, { start, end: content.length });
    });
    const references: SourceRegion[] = [];
    const visit = (node: ReferenceNode) => {
      const start = node.position?.start.offset;
      const end = node.position?.end.offset;
      if (referenceTypes.has(node.type) && start !== undefined && end !== undefined) {
        references.push({ start, end });
      }
      node.children?.forEach(visit);
    };
    if (content) visit(unified().use(remarkParse).use(remarkGfm).parse(content));
    const regions = new Map([...candidates].filter(([, region]) =>
      references.some(ref => ref.start >= region.start && ref.end <= region.end)));
    context = { content, regions };
    contexts.set(doc, context);
  }
  const region = context.regions.get(pos);
  return region ? { content: context.content, sourceRange: { ...region, idPrefix } } : undefined;
}
