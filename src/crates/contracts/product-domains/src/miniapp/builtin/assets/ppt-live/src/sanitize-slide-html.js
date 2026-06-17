export function sanitizeSlideDocumentRoot(doc = document, aggressive = false) {
  const document = doc;
  const view = document.defaultView || window;

    const skipTags = new Set(['SCRIPT', 'STYLE', 'PRE', 'CODE', 'SVG', 'TEXTAREA']);
    const inlineSelector = 'strong,b,em,i,u,span,a,small,mark,sub,sup,code';
    const textSelector = 'p,h1,h2,h3,h4,h5,h6,li';

    function inferBlockTag(node) {
      const cls = String(node.className || '').toLowerCase();
      const role = String(node.getAttribute?.('role') || '').toLowerCase();
      if (/h1|title|headline|hero/.test(cls) || role === 'heading') return 'h1';
      if (/h2|subtitle|subhead|section-title/.test(cls)) return 'h2';
      if (/h3|kicker|eyebrow|label|caption/.test(cls)) return 'h3';
      return 'p';
    }

    function isTransparentColor(color) {
      return !color || color === 'transparent' || color === 'rgba(0, 0, 0, 0)';
    }

    function ensureExportCanvas() {
      const body = document.body;
      if (!body) return;
      const computed = view.getComputedStyle(body);
      const width = parseFloat(computed.width);
      const height = parseFloat(computed.height);
      body.style.width = width > 0 ? `${width}px` : '1280px';
      body.style.height = height > 0 ? `${height}px` : '720px';
      body.style.margin = '0';
      body.style.padding = computed.padding || '0';
      body.style.overflow = 'hidden';
      body.style.position = computed.position === 'static' ? 'relative' : computed.position;
      if (!isTransparentColor(computed.backgroundColor)) {
        body.style.backgroundColor = computed.backgroundColor;
      }
      if (computed.color) body.style.color = computed.color;
      document.documentElement.style.margin = '0';
      document.documentElement.style.padding = '0';
      const rootBg = view.getComputedStyle(document.documentElement).backgroundColor;
      if (!isTransparentColor(rootBg) && isTransparentColor(computed.backgroundColor)) {
        body.style.backgroundColor = rootBg;
      }
    }

    function wrapDirectTextNodes(root) {
      root.querySelectorAll('div').forEach((div) => {
        if (skipTags.has(div.tagName)) return;
        [...div.childNodes].forEach((node) => {
          if (node.nodeType !== Node.TEXT_NODE) return;
          const text = node.textContent.replace(/\s+/g, ' ').trim();
          if (!text) {
            node.remove();
            return;
          }
          const block = document.createElement(inferBlockTag(div));
          block.textContent = text;
          div.replaceChild(block, node);
        });
      });
    }

    function promoteDecoratedSpans(root) {
      root.querySelectorAll('span').forEach((span) => {
        const computed = view.getComputedStyle(span);
        const hasBg = computed.backgroundColor && computed.backgroundColor !== 'rgba(0, 0, 0, 0)';
        const hasBorder = hasVisibleBorder(computed);
        if (!hasBg && !hasBorder) return;
        const block = document.createElement('p');
        if (span.className) block.className = span.className;
        if (span.getAttribute('style')) block.setAttribute('style', span.getAttribute('style'));
        block.textContent = span.textContent;
        span.replaceWith(block);
      });
    }

    function normalizeInlineLists(root) {
      root.querySelectorAll('div').forEach((div) => {
        const onlySpans = [...div.children].length > 0
          && [...div.children].every((child) => child.tagName === 'SPAN' || child.tagName === 'BR');
        const text = div.textContent.replace(/\s+/g, ' ').trim();
        if (!onlySpans || !text || div.querySelector('ul,ol,p,h1,h2,h3,h4,h5,h6')) return;
        const items = text.split(/\s*[•·▪-]\s+/).map((item) => item.trim()).filter(Boolean);
        if (items.length >= 2) {
          const ul = document.createElement('ul');
          items.forEach((item) => {
            const li = document.createElement('li');
            li.textContent = item;
            ul.appendChild(li);
          });
          div.replaceChildren(ul);
        }
      });
    }

    function hasVisibleBorder(computed) {
      return ['Top', 'Right', 'Bottom', 'Left'].some((side) => parseFloat(computed[`border${side}Width`] || 0) > 0);
    }

    function hoistTextDecorations(root) {
      root.querySelectorAll(textSelector).forEach((el) => {
        const computed = view.getComputedStyle(el);
        const hasBg = computed.backgroundColor && computed.backgroundColor !== 'rgba(0, 0, 0, 0)';
        const hasBgImage = computed.backgroundImage && computed.backgroundImage !== 'none';
        const hasBorder = hasVisibleBorder(computed);
        const hasShadow = computed.boxShadow && computed.boxShadow !== 'none';
        if (!hasBg && !hasBgImage && !hasBorder && !hasShadow) return;
        const wrapper = document.createElement('div');
        if (hasBg || hasBgImage) {
          wrapper.style.background = computed.background;
          wrapper.style.backgroundColor = computed.backgroundColor;
        }
        if (hasBgImage && !String(computed.backgroundImage || '').includes('gradient')) {
          wrapper.style.backgroundImage = 'none';
        }
        if (hasBorder) wrapper.style.border = computed.border;
        if (computed.borderRadius) wrapper.style.borderRadius = computed.borderRadius;
        if (hasShadow) wrapper.style.boxShadow = computed.boxShadow;
        if (computed.padding) wrapper.style.padding = computed.padding;
        el.style.background = 'transparent';
        el.style.backgroundColor = 'transparent';
        el.style.backgroundImage = 'none';
        el.style.border = 'none';
        el.style.boxShadow = 'none';
        el.style.padding = '0';
        el.parentNode.insertBefore(wrapper, el);
        wrapper.appendChild(el);
      });
    }

    function flattenGradients(root) {
      root.querySelectorAll('*').forEach((el) => {
        const computed = view.getComputedStyle(el);
        const bgImage = computed.backgroundImage || '';
        if (!bgImage.includes('gradient')) return;
        const colorMatch = bgImage.match(/#[0-9a-f]{3,8}|rgba?\([^)]+\)/i);
        el.style.backgroundImage = 'none';
        if (colorMatch) {
          el.style.backgroundColor = colorMatch[0];
        } else if (computed.backgroundColor && computed.backgroundColor !== 'rgba(0, 0, 0, 0)') {
          el.style.backgroundColor = computed.backgroundColor;
        }
      });
    }

    function stripUnsupportedDivBackgrounds(root) {
      root.querySelectorAll('div').forEach((el) => {
        const computed = view.getComputedStyle(el);
        const bgImage = computed.backgroundImage;
        if (!bgImage || bgImage === 'none') return;
        el.style.backgroundImage = 'none';
        if (computed.backgroundColor && computed.backgroundColor !== 'rgba(0, 0, 0, 0)') {
          el.style.backgroundColor = computed.backgroundColor;
        }
      });
    }

    function resetInlineBoxModel(root) {
      root.querySelectorAll(inlineSelector).forEach((el) => {
        el.style.setProperty('margin', '0', 'important');
        el.style.setProperty('padding', '0', 'important');
        el.style.setProperty('border', 'none', 'important');
        el.style.setProperty('box-shadow', 'none', 'important');
        el.style.setProperty('background', 'transparent', 'important');
        el.style.setProperty('background-color', 'transparent', 'important');
        el.style.setProperty('background-image', 'none', 'important');
        if (view.getComputedStyle(el).display === 'block') {
          el.style.setProperty('display', 'inline', 'important');
        }
      });
    }

    function stripInlineClasses(root) {
      root.querySelectorAll(inlineSelector).forEach((el) => {
        el.removeAttribute('class');
        el.removeAttribute('style');
      });
    }

    function stripAuthorStylesheets(root) {
      root.querySelectorAll('link[rel="stylesheet"], style').forEach((node) => {
        if (node.id === 'ppt-live-export-safe-styles') return;
        node.remove();
      });
    }

    function enforceInlineElementsSafe(root) {
      root.querySelectorAll(inlineSelector).forEach((el) => {
        const computed = view.getComputedStyle(el);
        const hasBadMargin = ['marginTop', 'marginRight', 'marginBottom', 'marginLeft'].some(
          (prop) => parseFloat(computed[prop]) > 0,
        );
        const hasBadPadding = ['paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft'].some(
          (prop) => parseFloat(computed[prop]) > 0,
        );
        const hasBorder = hasVisibleBorder(computed);
        const hasBg = computed.backgroundColor && computed.backgroundColor !== 'rgba(0, 0, 0, 0)';
        const hasBgImage = computed.backgroundImage && computed.backgroundImage !== 'none';
        if (!hasBadMargin && !hasBadPadding && !hasBorder && !hasBg && !hasBgImage) return;

        const tag = el.tagName.toLowerCase();
        const clean = document.createElement(tag);
        clean.textContent = el.textContent;
        el.replaceWith(clean);
      });
    }

    function inlineSnapshotLayoutStyles(root) {
      const slideBody = root.body || root.querySelector('.ppt-export-body');
      const nodes = slideBody
        ? [slideBody, ...slideBody.querySelectorAll('*')]
        : [...root.querySelectorAll('body, body *')];
      nodes.forEach((el) => {
        if (skipTags.has(el.tagName)) return;
        const computed = view.getComputedStyle(el);
        const style = el.style;
        if (computed.position && computed.position !== 'static') style.position = computed.position;
        if (computed.display && computed.display !== 'inline') style.display = computed.display;
        ['left', 'top', 'right', 'bottom', 'width', 'height', 'maxWidth', 'maxHeight'].forEach((prop) => {
          const value = computed[prop];
          if (value && value !== 'auto' && value !== 'none' && value !== '0px') {
            style[prop] = value;
          }
        });
        if (computed.zIndex && computed.zIndex !== 'auto') style.zIndex = computed.zIndex;
        if (computed.color) style.color = computed.color;
        if (computed.fontSize) style.fontSize = computed.fontSize;
        if (computed.fontWeight) style.fontWeight = computed.fontWeight;
        if (computed.fontFamily) style.fontFamily = computed.fontFamily;
        if (computed.lineHeight && computed.lineHeight !== 'normal') style.lineHeight = computed.lineHeight;
        if (computed.textAlign) style.textAlign = computed.textAlign;
        const bg = computed.backgroundColor;
        if (bg && bg !== 'rgba(0, 0, 0, 0)') style.backgroundColor = bg;
        if (computed.border && computed.border !== 'none' && hasVisibleBorder(computed)) {
          style.border = computed.border;
        }
        if (computed.borderRadius && computed.borderRadius !== '0px') {
          style.borderRadius = computed.borderRadius;
        }
        if (computed.padding && computed.padding !== '0px') style.padding = computed.padding;
        if (computed.gap && computed.gap !== 'normal') style.gap = computed.gap;
        if (computed.flexDirection && computed.flexDirection !== 'row') {
          style.flexDirection = computed.flexDirection;
        }
        if (computed.alignItems && computed.alignItems !== 'normal') {
          style.alignItems = computed.alignItems;
        }
        if (computed.justifyContent && computed.justifyContent !== 'normal') {
          style.justifyContent = computed.justifyContent;
        }
      });
    }

    function injectExportSafeStyles(root) {
      const styleId = 'ppt-live-export-safe-styles';
      root.getElementById(styleId)?.remove();
      const style = document.createElement('style');
      style.id = styleId;
      style.textContent = `
        ${inlineSelector}, [class] ${inlineSelector.split(',').join(', [class] ')} {
          margin: 0 !important;
          padding: 0 !important;
          border: none !important;
          box-shadow: none !important;
          background: transparent !important;
          background-color: transparent !important;
          background-image: none !important;
        }
        p, h1, h2, h3, h4, h5, h6, li {
          box-shadow: none !important;
        }
      `;
      (root.head || root.documentElement).appendChild(style);
    }

    ensureExportCanvas();
    wrapDirectTextNodes(document);
    promoteDecoratedSpans(document);
    normalizeInlineLists(document);
    flattenGradients(document);
    stripUnsupportedDivBackgrounds(document);

    if (aggressive) {
      hoistTextDecorations(document);
      resetInlineBoxModel(document);
      enforceInlineElementsSafe(document);
      injectExportSafeStyles(document);
      enforceInlineElementsSafe(document);
      inlineSnapshotLayoutStyles(document);
      document.querySelectorAll('[class]').forEach((el) => el.removeAttribute('class'));
      stripAuthorStylesheets(document);
      stripInlineClasses(document);
      resetInlineBoxModel(document);
      enforceInlineElementsSafe(document);
      injectExportSafeStyles(document);
    } else {
      // Preserve author layout/CSS; snapshot computed styles for a stable second paint.
      inlineSnapshotLayoutStyles(document);
    }
}
