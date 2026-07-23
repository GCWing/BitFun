export function sanitizeSlideDocumentRoot(doc = document, aggressive = false) {
  const document = doc;
  const view = document.defaultView || window;
  const diagnostics = [];
  const seenDiagnostics = new Set();

    const skipTags = new Set(['SCRIPT', 'STYLE', 'PRE', 'CODE', 'SVG', 'TEXTAREA']);
    const inlineSelector = 'strong,b,em,i,u,span,a,small,mark,sub,sup,code';
    const textSelector = 'p,h1,h2,h3,h4,h5,h6,li';
    const textContainerSelector = 'p,h1,h2,h3,h4,h5,h6,li';
    const manualBulletPattern = /^(\s*)([•●○▪‣·▸◆◇■□–—*-])\s+/u;
    const ambiguousBulletSymbols = new Set(['-', '*', '–', '—']);

    function sourceIdOf(element) {
      return element?.dataset?.pptxSourceId || element?.id || null;
    }

    function assignSourceIds() {
      const body = document.body;
      if (!body) return;
      const elements = [body, ...body.querySelectorAll('*')];
      const reserved = new Set(
        elements.map((element) => element.dataset.pptxSourceId?.trim()).filter(Boolean),
      );
      const used = new Set();
      elements.forEach((element) => {
        const sourceId = element.dataset.pptxSourceId?.trim();
        if (!sourceId) return;
        if (!used.has(sourceId)) {
          element.dataset.pptxSourceId = sourceId;
          used.add(sourceId);
          return;
        }
        let suffix = 2;
        let candidate = `${sourceId}-${suffix}`;
        while (used.has(candidate) || reserved.has(candidate)) {
          suffix += 1;
          candidate = `${sourceId}-${suffix}`;
        }
        element.dataset.pptxSourceId = candidate;
        used.add(candidate);
        reserved.add(candidate);
        addDiagnostic(
          'repaired',
          'duplicate_source_id_repaired',
          `Duplicate source element id "${sourceId}" was reassigned to "${candidate}".`,
          element,
        );
      });
      let sequence = 1;
      elements.forEach((element) => {
        if (element.dataset?.pptxSourceId) return;
        while (used.has(`pptx-source-${sequence}`) || reserved.has(`pptx-source-${sequence}`)) {
          sequence += 1;
        }
        const sourceId = `pptx-source-${sequence}`;
        sequence += 1;
        element.dataset.pptxSourceId = sourceId;
        used.add(sourceId);
      });
    }

    function derivedSourceId(element, suffix) {
      const base = sourceIdOf(element) || 'pptx-source';
      let candidate = `${base}-${suffix}`;
      let sequence = 2;
      while (document.querySelector(`[data-pptx-source-id="${candidate}"]`)) {
        candidate = `${base}-${suffix}-${sequence}`;
        sequence += 1;
      }
      return candidate;
    }

    function addDiagnostic(severity, code, message, element = null) {
      const sourceId = sourceIdOf(element);
      const key = `${severity}:${code}:${sourceId || ''}`;
      if (seenDiagnostics.has(key)) return;
      seenDiagnostics.add(key);
      const diagnostic = {
        severity,
        kind: severity === 'blocking' ? 'blocking' : undefined,
        code,
        message,
        sourceId,
        tag: element?.tagName?.toLowerCase?.() || null,
      };
      try {
        const rect = element?.getBoundingClientRect?.();
        if (rect && rect.width > 0 && rect.height > 0) {
          diagnostic.bbox = {
            x: rect.left,
            y: rect.top,
            width: rect.width,
            height: rect.height,
          };
        }
      } catch {
        // Diagnostics remain useful without optional geometry.
      }
      diagnostics.push(diagnostic);
    }

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

    function repairNestedParagraphs(root) {
      root.querySelectorAll(textContainerSelector).forEach((outer) => {
        const nested = [...outer.children].filter((child) => child.matches(textContainerSelector));
        if (!nested.length || !outer.parentNode) return;
        const fragments = [];
        let current = document.createElement(outer.tagName.toLowerCase());
        const copyOuterAttributes = (target) => {
          [...outer.attributes].forEach((attribute) => {
            if (attribute.name !== 'data-pptx-source-id') target.setAttribute(attribute.name, attribute.value);
          });
        };
        copyOuterAttributes(current);
        current.dataset.pptxSourceId = sourceIdOf(outer);
        [...outer.childNodes].forEach((node) => {
          if (node.nodeType === view.Node.ELEMENT_NODE && node.matches(textContainerSelector)) {
            if (current.textContent.trim() || current.children.length) fragments.push(current);
            fragments.push(node);
            current = document.createElement(outer.tagName.toLowerCase());
            copyOuterAttributes(current);
            current.dataset.pptxSourceId = derivedSourceId(outer, `split-${fragments.length + 1}`);
          } else {
            current.appendChild(node);
          }
        });
        if (current.textContent.trim() || current.children.length) fragments.push(current);
        outer.replaceWith(...fragments);
        addDiagnostic(
          'repaired',
          'nested_paragraph_repaired',
          'Nested paragraph structure was split into ordered sibling paragraphs.',
          fragments[0] || nested[0],
        );
      });
    }

    function wrapDirectTextNodes(root) {
      root.querySelectorAll('div').forEach((div) => {
        if (skipTags.has(div.tagName)) return;
        let sequence = 1;
        let nodes = [...div.childNodes];
        while (nodes.length) {
          const firstDirectText = nodes.findIndex(
            (node) => node.nodeType === view.Node.TEXT_NODE && node.textContent.trim(),
          );
          if (firstDirectText < 0) break;
          let start = firstDirectText;
          while (start > 0) {
            const previous = nodes[start - 1];
            if (previous.nodeType === view.Node.ELEMENT_NODE
              && !previous.matches(inlineSelector)
              && previous.tagName !== 'BR') break;
            start -= 1;
          }
          let end = firstDirectText;
          while (end + 1 < nodes.length) {
            const next = nodes[end + 1];
            if (next.nodeType === view.Node.ELEMENT_NODE
              && !next.matches(inlineSelector)
              && next.tagName !== 'BR') break;
            end += 1;
          }
          const group = nodes.slice(start, end + 1);
          const block = document.createElement(inferBlockTag(div));
          block.dataset.pptxSourceId = derivedSourceId(div, `text-${sequence}`);
          sequence += 1;
          group[0].before(block);
          group.forEach((node) => {
            if (node.nodeType === view.Node.TEXT_NODE) {
              node.textContent = node.textContent.replace(/\s+/g, ' ');
            }
            block.appendChild(node);
          });
          block.normalize();
          addDiagnostic(
            'repaired',
            'direct_text_wrapped',
            'Direct container text was wrapped in a semantic text block.',
            block,
          );
          nodes = [...div.childNodes];
        }
        [...div.childNodes].forEach((node) => {
          if (node.nodeType === view.Node.TEXT_NODE && !node.textContent.trim()) node.remove();
        });
      });
    }

    function promoteDecoratedSpans(root) {
      root.querySelectorAll('span').forEach((span) => {
        const computed = view.getComputedStyle(span);
        const hasBg = computed.backgroundColor && computed.backgroundColor !== 'rgba(0, 0, 0, 0)';
        const hasBorder = hasVisibleBorder(computed);
        if (!hasBg && !hasBorder) return;
        if (span.closest(textContainerSelector)) return;
        const block = document.createElement('p');
        if (span.className) block.className = span.className;
        if (span.getAttribute('style')) block.setAttribute('style', span.getAttribute('style'));
        block.dataset.pptxSourceId = sourceIdOf(span);
        while (span.firstChild) block.appendChild(span.firstChild);
        span.replaceWith(block);
        addDiagnostic(
          'repaired',
          'decorated_inline_promoted',
          'Decorated inline text was promoted to a block that can be exported as shape plus text.',
          block,
        );
      });
    }

    function removeManualBullet(element) {
      const symbol = element.textContent.match(manualBulletPattern)?.[2];
      if (!symbol) return;
      const walker = document.createTreeWalker(element, view.NodeFilter.SHOW_TEXT);
      let textNode = walker.nextNode();
      let removed = false;
      while (textNode) {
        if (!removed) {
          const symbolIndex = textNode.textContent.search(/\S/u);
          if (symbolIndex >= 0) {
            if (textNode.textContent.slice(symbolIndex).startsWith(symbol)) {
              textNode.textContent = textNode.textContent.slice(symbolIndex + symbol.length).replace(/^\s+/u, '');
              removed = true;
              if (textNode.textContent) return;
            } else {
              return;
            }
          }
        } else if (textNode.textContent) {
          textNode.textContent = textNode.textContent.replace(/^\s+/u, '');
          return;
        }
        textNode = walker.nextNode();
      }
    }

    function normalizeManualBulletBlocks(root) {
      root.querySelectorAll('body, div, section, article, aside, main, td, th').forEach((parent) => {
        let group = [];
        const flush = () => {
          if (!group.length) return;
          const first = group[0];
          const firstSymbol = first.textContent.match(manualBulletPattern)?.[2];
          if (group.length === 1 && ambiguousBulletSymbols.has(firstSymbol)) {
            group = [];
            return;
          }
          const list = document.createElement('ul');
          list.dataset.pptxSourceId = derivedSourceId(first, 'list');
          const firstComputed = view.getComputedStyle(first);
          const authoredIndent = parseFloat(firstComputed.marginLeft || first.style.marginLeft || '0') || 0;
          list.style.margin = '0';
          list.style.paddingLeft = `${Math.max(24, authoredIndent + 24)}px`;
          first.parentNode.insertBefore(list, first);
          group.forEach((block) => {
            const item = document.createElement('li');
            [...block.attributes].forEach((attribute) => {
              if (!['id', 'data-pptx-source-id'].includes(attribute.name)) {
                item.setAttribute(attribute.name, attribute.value);
              }
            });
            item.dataset.pptxSourceId = sourceIdOf(block);
            while (block.firstChild) item.appendChild(block.firstChild);
            removeManualBullet(item);
            list.appendChild(item);
            block.remove();
          });
          addDiagnostic(
            'repaired',
            'manual_bullet_list',
            `${group.length} consecutive manual bullet paragraph(s) were converted to a semantic list.`,
            list,
          );
          group = [];
        };
        [...parent.children].forEach((child) => {
          const isTextBlock = /^(P|H[1-6])$/.test(child.tagName);
          if (isTextBlock && manualBulletPattern.test(child.textContent || '')) {
            group.push(child);
          } else {
            flush();
          }
        });
        flush();
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

    function collectEditableExportDiagnostics(root) {
      root.querySelectorAll('*').forEach((element) => {
        const computed = view.getComputedStyle(element);
        const backgroundImage = String(computed.backgroundImage || element.style?.backgroundImage || '');
        if (backgroundImage.includes('gradient')) {
          addDiagnostic(
            'rewrite',
            'css_gradient',
            'CSS gradient will be rewritten as editable solid strips.',
            element,
          );
        }
        const filter = String(computed.filter || element.style?.filter || '');
        if (filter && filter !== 'none') {
          addDiagnostic(
            'blocking',
            'css_filter',
            'CSS filter cannot be represented as editable objects.',
            element,
          );
        }
      });
      root.querySelectorAll('svg').forEach((svg) => {
        if (svg.querySelector('filter,mask,foreignObject,use,pattern,textPath,clipPath,image')) {
          addDiagnostic(
            'blocking',
            'complex_svg_unsupported',
            'SVG filter, mask, or foreignObject cannot be represented as editable objects.',
            svg,
          );
        } else if (svg.querySelector('path')) {
          addDiagnostic(
            'rewrite',
            'svg_path_rewrite',
            'SVG path geometry will be rewritten as editable line segments.',
            svg,
          );
        }
      });
      root.querySelectorAll('style').forEach((style) => {
        let rules = [];
        try {
          rules = [...(style.sheet?.cssRules || [])];
        } catch {
          return;
        }
        rules.forEach((rule) => {
          const selector = rule.selectorText || '';
          const content = rule.style?.content;
          if (!/::(before|after)/.test(selector)
            || !content
            || ['none', 'normal', '""', "''"].includes(content)) return;
          const baseSelector = selector.replace(/::(before|after)/g, '').trim();
          let matches = [];
          try {
            matches = [...root.querySelectorAll(baseSelector)];
          } catch {
            return;
          }
          matches.forEach((element) => addDiagnostic(
            'blocking',
            'generated_content',
            'Pseudo-element generated content cannot be represented as editable objects.',
            element,
          ));
        });
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

    document.querySelectorAll('[style]').forEach((element) => {
      element.dataset.pptxAuthoredStyle = element.getAttribute('style');
    });
    assignSourceIds();
    collectEditableExportDiagnostics(document);
    ensureExportCanvas();
    repairNestedParagraphs(document);
    promoteDecoratedSpans(document);
    wrapDirectTextNodes(document);
    normalizeManualBulletBlocks(document);
    normalizeInlineLists(document);

    if (aggressive) {
      flattenGradients(document);
      stripUnsupportedDivBackgrounds(document);
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
    return { diagnostics };
}
