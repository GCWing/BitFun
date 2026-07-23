import { mergeAttributes, Node } from '@tiptap/core';
import { getCachedLocalImageDataUrl } from '../utils/loadLocalImages';
import { isLocalPath, resolveImagePath } from '../utils/rehype-local-images';

type MarkdownImageOptions = {
  basePath?: string;
};

export const MarkdownImage = Node.create<MarkdownImageOptions>({
  name: 'markdownImage',
  inline: true,
  group: 'inline',
  atom: true,
  draggable: true,

  addOptions() {
    return {
      basePath: undefined,
    };
  },

  addAttributes() {
    return {
      src: {
        default: '',
      },
      alt: {
        default: '',
      },
      title: {
        default: null,
      },
    };
  },

  parseHTML() {
    return [{ tag: 'img[src]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ['img', mergeAttributes(HTMLAttributes)] as const;
  },

  addNodeView() {
    return ({ node }) => {
      const dom = document.createElement('img');
      const src = typeof node.attrs.src === 'string' ? node.attrs.src : '';
      const alt = typeof node.attrs.alt === 'string' ? node.attrs.alt : '';
      const title = typeof node.attrs.title === 'string' ? node.attrs.title : '';

      if (alt) {
        dom.setAttribute('alt', alt);
      }
      if (title) {
        dom.setAttribute('title', title);
      }

      if (src && isLocalPath(src)) {
        const absolutePath = resolveImagePath(src, this.options.basePath);
        const cachedDataUrl = getCachedLocalImageDataUrl(absolutePath);
        dom.setAttribute('src', cachedDataUrl ?? src);
      } else {
        dom.setAttribute('src', src);
      }

      return { dom };
    };
  },
});
