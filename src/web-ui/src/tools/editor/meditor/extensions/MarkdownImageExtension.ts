import { mergeAttributes, Node } from '@tiptap/core';
import { closeHistory } from '@tiptap/pm/history';
import { getCachedLocalImageDataUrl } from '../utils/loadLocalImages';
import { isLocalPath, resolveImagePath } from '../utils/rehype-local-images';

type MarkdownImageOptions = {
  basePath?: string;
  editLabel?: string;
  doneLabel?: string;
  srcLabel?: string;
  altLabel?: string;
  titleLabel?: string;
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
    return ({ editor, node, getPos }) => {
      let currentNode = node;
      let editing = false;
      let lastSource: string | undefined;
      const dom = document.createElement('span');
      dom.className = 'm-editor-image';
      dom.dataset.testid = 'md-image';
      dom.contentEditable = 'false';
      const image = document.createElement('img');
      image.tabIndex = 0;
      const action = document.createElement('button');
      action.type = 'button';
      action.className = 'm-editor-source-block-action';
      action.dataset.testid = 'md-image-edit';
      const fields = document.createElement('span');
      fields.className = 'm-editor-image-fields';
      const inputs = new Map<string, HTMLInputElement>();
      for (const [attr, text] of [
        ['src', this.options.srcLabel],
        ['alt', this.options.altLabel],
        ['title', this.options.titleLabel],
      ]) {
        const label = document.createElement('label');
        label.textContent = text ?? attr ?? '';
        const input = document.createElement('input');
        input.type = 'text';
        input.dataset.testid = `md-image-${attr}`;
        input.addEventListener('input', () => {
          const pos = getPos();
          if (!editor.isEditable || typeof pos !== 'number' || !attr) return;
          editor.view.dispatch(editor.state.tr.setNodeMarkup(pos, undefined, {
            ...currentNode.attrs, [attr]: input.value,
          }));
        });
        if (attr) inputs.set(attr, input);
        label.append(input);
        fields.append(label);
      }
      dom.append(image, action, fields);
      const sync = () => {
        const src = String(currentNode.attrs.src ?? '');
        if (src !== lastSource) {
          lastSource = src;
          delete image.dataset.localResolved;
          image.removeAttribute('data-local-image');
          image.removeAttribute('data-local-path');
          image.removeAttribute('data-original-src');
          image.classList.remove('local-image-loading', 'local-image-loaded', 'local-image-error');
          const resolvedSrc = src && isLocalPath(src)
            ? getCachedLocalImageDataUrl(resolveImagePath(src, this.options.basePath)) ?? src
            : src;
          image.setAttribute('src', resolvedSrc);
        }
        image.alt = String(currentNode.attrs.alt ?? '');
        image.title = String(currentNode.attrs.title ?? '');
        if (!editor.isEditable) editing = false;
        action.hidden = !editor.isEditable || !editing;
        action.textContent = editing ? this.options.doneLabel ?? '' : this.options.editLabel ?? '';
        action.setAttribute('aria-expanded', String(editing));
        fields.hidden = !editing;
        inputs.forEach((input, attr) => {
          const value = String(currentNode.attrs[attr] ?? '');
          if (input.value !== value) input.value = value;
          input.readOnly = !editor.isEditable;
        });
      };
      const toggle = (event: Event) => {
        if (!editor.isEditable) return;
        event.preventDefault();
        event.stopPropagation();
        editor.view.dispatch(closeHistory(editor.state.tr));
        editing = !editing;
        sync();
        if (editing) inputs.get('src')?.focus();
        else image.focus();
      };
      const finish = (focus = false) => {
        editing = false;
        sync();
        if (focus) image.focus();
      };
      const finishOutside = (event: Event) => {
        if (editing && event.target instanceof globalThis.Node && !dom.contains(event.target)) finish();
      };
      document.addEventListener('click', finishOutside, true);
      dom.addEventListener('keydown', event => {
        if (event.key === 'Tab') window.setTimeout(() => {
          if (dom.isConnected && !dom.contains(document.activeElement)) finish();
        }, 0);
      }, true);
      image.addEventListener('keydown', event => {
        if (event.key === 'Enter' || event.key === ' ') toggle(event);
      });
      action.addEventListener('click', toggle);
      image.addEventListener('click', toggle);
      fields.addEventListener('keydown', event => {
        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') return;
        if ((event.key === 'Escape' || event.key === 'Enter') && !event.isComposing) {
          event.preventDefault();
          finish(true);
        } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') {
          event.preventDefault();
          if (event.shiftKey) editor.commands.redo();
          else editor.commands.undo();
        }
        event.stopPropagation();
      });
      sync();
      editor.on('update', sync);
      return {
        dom,
        update: updatedNode => {
          if (updatedNode.type !== currentNode.type) return false;
          currentNode = updatedNode;
          sync();
          return true;
        },
        stopEvent: event => event.target instanceof HTMLElement && !!event.target.closest('input, button'),
        ignoreMutation: () => true,
        destroy: () => {
          editor.off('update', sync);
          document.removeEventListener('click', finishOutside, true);
        },
      };
    };
  },
});
