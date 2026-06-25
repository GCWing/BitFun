export const PT_PER_PX = 0.75;
export const PX_PER_IN = 96;

export function measureBodyDimensions(doc = document) {
  const view = doc.defaultView || window;
  const body = doc.body;
  const style = view.getComputedStyle(body);
  const bodyDimensions = {
    width: parseFloat(style.width),
    height: parseFloat(style.height),
    scrollWidth: body.scrollWidth,
    scrollHeight: body.scrollHeight,
  };
  const errors = [];
  const widthOverflowPx = Math.max(0, bodyDimensions.scrollWidth - bodyDimensions.width - 1);
  const heightOverflowPx = Math.max(0, bodyDimensions.scrollHeight - bodyDimensions.height - 1);
  const widthOverflowPt = widthOverflowPx * PT_PER_PX;
  const heightOverflowPt = heightOverflowPx * PT_PER_PX;
  if (widthOverflowPt > 0 || heightOverflowPt > 0) {
    const directions = [];
    if (widthOverflowPt > 0) directions.push(`${widthOverflowPt.toFixed(1)}pt horizontally`);
    if (heightOverflowPt > 0) directions.push(`${heightOverflowPt.toFixed(1)}pt vertically`);
    const reminder = heightOverflowPt > 0 ? ' (Remember: leave 0.5" margin at bottom of slide)' : '';
    errors.push(`HTML content overflows body by ${directions.join(' and ')}${reminder}`);
  }
  return { ...bodyDimensions, errors };
}

export function extractSlideDataFromDocument(doc = document) {
  const document = doc;
  const view = document.defaultView || window;

    const PT_PER_PX = 0.75;
    const PX_PER_IN = 96;

    // Fonts that are single-weight and should not have bold applied
    // (applying bold causes PowerPoint to use faux bold which makes text wider)
    const SINGLE_WEIGHT_FONTS = ['impact'];

    // Helper: Check if a font should skip bold formatting
    const shouldSkipBold = (fontFamily) => {
      if (!fontFamily) return false;
      const normalizedFont = fontFamily.toLowerCase().replace(/['"]/g, '').split(',')[0].trim();
      return SINGLE_WEIGHT_FONTS.includes(normalizedFont);
    };

    // Unit conversion helpers
    const pxToInch = (px) => px / PX_PER_IN;
    const pxToPoints = (pxStr) => parseFloat(pxStr) * PT_PER_PX;
    const rgbToHex = (rgbStr) => {
      // Handle transparent backgrounds by defaulting to white
      if (rgbStr === 'rgba(0, 0, 0, 0)' || rgbStr === 'transparent') return 'FFFFFF';

      const match = rgbStr.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
      if (!match) return 'FFFFFF';
      return match.slice(1).map(n => parseInt(n).toString(16).padStart(2, '0')).join('');
    };

    const extractAlpha = (rgbStr) => {
      const match = rgbStr.match(/rgba\((\d+),\s*(\d+),\s*(\d+),\s*([\d.]+)\)/);
      if (!match || !match[4]) return null;
      const alpha = parseFloat(match[4]);
      return Math.round((1 - alpha) * 100);
    };

    const parseRgbChannels = (rgbStr) => {
      const match = String(rgbStr || '').match(/rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\)/);
      if (!match) return null;
      return {
        r: parseInt(match[1], 10),
        g: parseInt(match[2], 10),
        b: parseInt(match[3], 10),
        a: match[4] != null ? parseFloat(match[4]) : 1,
      };
    };

    const hexToRgb = (hex) => {
      const clean = String(hex || '0E0E12').replace('#', '');
      return {
        r: parseInt(clean.slice(0, 2), 16),
        g: parseInt(clean.slice(2, 4), 16),
        b: parseInt(clean.slice(4, 6), 16),
      };
    };

    const resolveSolidFill = (rgbStr, backdropHex = '0E0E12') => {
      const channels = parseRgbChannels(rgbStr);
      if (!channels) {
        return { fill: rgbToHex(rgbStr), transparency: extractAlpha(rgbStr) };
      }
      if (channels.a >= 0.98) {
        return { fill: rgbToHex(rgbStr), transparency: null };
      }
      const bg = hexToRgb(backdropHex);
      const r = Math.round(bg.r * (1 - channels.a) + channels.r * channels.a);
      const g = Math.round(bg.g * (1 - channels.a) + channels.g * channels.a);
      const b = Math.round(bg.b * (1 - channels.a) + channels.b * channels.a);
      return {
        fill: [r, g, b].map((n) => n.toString(16).padStart(2, '0')).join('').toUpperCase(),
        transparency: null,
      };
    };

    const applyTextTransform = (text, textTransform) => {
      if (textTransform === 'uppercase') return text.toUpperCase();
      if (textTransform === 'lowercase') return text.toLowerCase();
      if (textTransform === 'capitalize') {
        return text.replace(/\b\w/g, c => c.toUpperCase());
      }
      return text;
    };

    const getTextDirection = (writingMode) => {
      if (writingMode === 'vertical-rl' || writingMode === 'vertical-lr') {
        return 'eaVert';
      }
      return null;
    };

    // Extract rotation angle from CSS transform only. CSS writing-mode is exported
    // as native PowerPoint vertical text so its box geometry stays anchored.
    const getRotation = (transform) => {
      let angle = 0;

      if (transform && transform !== 'none') {
        // Try to match rotate() function
        const rotateMatch = transform.match(/rotate\((-?\d+(?:\.\d+)?)deg\)/);
        if (rotateMatch) {
          angle += parseFloat(rotateMatch[1]);
        } else {
          // Browser may compute as matrix - extract rotation from matrix
          const matrixMatch = transform.match(/matrix\(([^)]+)\)/);
          if (matrixMatch) {
            const values = matrixMatch[1].split(',').map(parseFloat);
            // matrix(a, b, c, d, e, f) where rotation = atan2(b, a)
            const matrixAngle = Math.atan2(values[1], values[0]) * (180 / Math.PI);
            angle += Math.round(matrixAngle);
          }
        }
      }

      // Normalize to 0-359 range
      angle = angle % 360;
      if (angle < 0) angle += 360;

      return angle === 0 ? null : angle;
    };

    // Get position/dimensions accounting for rotation
    const getPositionAndSize = (el, rect, rotation) => {
      if (rotation === null) {
        return { x: rect.left, y: rect.top, w: rect.width, h: rect.height };
      }

      const isVertical = rotation === 90 || rotation === 270;

      if (isVertical) {
        const centerX = rect.left + rect.width / 2;
        const centerY = rect.top + rect.height / 2;

        return {
          x: centerX - rect.height / 2,
          y: centerY - rect.width / 2,
          w: rect.height,
          h: rect.width,
        };
      }

      const centerX = rect.left + rect.width / 2;
      const centerY = rect.top + rect.height / 2;
      return {
        x: centerX - el.offsetWidth / 2,
        y: centerY - el.offsetHeight / 2,
        w: el.offsetWidth,
        h: el.offsetHeight,
      };
    };

    // Parse CSS box-shadow into PptxGenJS shadow properties
    const parseBoxShadow = (boxShadow) => {
      if (!boxShadow || boxShadow === 'none') return null;

      // Browser computed style format: "rgba(0, 0, 0, 0.3) 2px 2px 8px 0px [inset]"
      // CSS format: "[inset] 2px 2px 8px 0px rgba(0, 0, 0, 0.3)"

      const insetMatch = boxShadow.match(/inset/);

      // IMPORTANT: PptxGenJS/PowerPoint doesn't properly support inset shadows
      // Only process outer shadows to avoid file corruption
      if (insetMatch) return null;

      // Extract color first (rgba or rgb at start)
      const colorMatch = boxShadow.match(/rgba?\([^)]+\)/);

      // Extract numeric values (handles both px and pt units)
      const parts = boxShadow.match(/([-\d.]+)(px|pt)/g);

      if (!parts || parts.length < 2) return null;

      const offsetX = parseFloat(parts[0]);
      const offsetY = parseFloat(parts[1]);
      const blur = parts.length > 2 ? parseFloat(parts[2]) : 0;

      // Calculate angle from offsets (in degrees, 0 = right, 90 = down)
      let angle = 0;
      if (offsetX !== 0 || offsetY !== 0) {
        angle = Math.atan2(offsetY, offsetX) * (180 / Math.PI);
        if (angle < 0) angle += 360;
      }

      // Calculate offset distance (hypotenuse)
      const offset = Math.sqrt(offsetX * offsetX + offsetY * offsetY) * PT_PER_PX;

      // Extract opacity from rgba
      let opacity = 0.5;
      if (colorMatch) {
        const opacityMatch = colorMatch[0].match(/[\d.]+\)$/);
        if (opacityMatch) {
          opacity = parseFloat(opacityMatch[0].replace(')', ''));
        }
      }

      return {
        type: 'outer',
        angle: Math.round(angle),
        blur: blur * 0.75, // Convert to points
        color: colorMatch ? rgbToHex(colorMatch[0]) : '000000',
        offset: offset,
        opacity
      };
    };

    // Parse inline formatting tags (<b>, <i>, <u>, <strong>, <em>, <span>) into text runs
    const parseInlineFormatting = (element, baseOptions = {}, runs = [], baseTextTransform = (x) => x) => {
      let prevNodeIsText = false;

      element.childNodes.forEach((node) => {
        let textTransform = baseTextTransform;

        const isText = node.nodeType === Node.TEXT_NODE || node.tagName === 'BR';
        if (isText) {
          const text = node.tagName === 'BR' ? '\n' : textTransform(node.textContent.replace(/\s+/g, ' '));
          const prevRun = runs[runs.length - 1];
          if (prevNodeIsText && prevRun) {
            prevRun.text += text;
          } else {
            runs.push({ text, options: { ...baseOptions } });
          }

        } else if (node.nodeType === Node.ELEMENT_NODE && node.textContent.trim()) {
          const options = { ...baseOptions };
          const computed = view.getComputedStyle(node);

          // Handle inline elements with computed styles
          if (['SPAN', 'B', 'STRONG', 'I', 'EM', 'U', 'SMALL', 'LABEL', 'A', 'CODE', 'MARK', 'SUB', 'SUP'].includes(node.tagName)) {
            const isBold = computed.fontWeight === 'bold' || parseInt(computed.fontWeight) >= 600;
            if (isBold && !shouldSkipBold(computed.fontFamily)) options.bold = true;
            if (computed.fontStyle === 'italic') options.italic = true;
            if (computed.textDecoration && computed.textDecoration.includes('underline')) options.underline = true;
            if (computed.color && computed.color !== 'rgb(0, 0, 0)') {
              options.color = rgbToHex(computed.color);
              const transparency = extractAlpha(computed.color);
              if (transparency !== null) options.transparency = transparency;
            }
            if (computed.fontSize) options.fontSize = pxToPoints(computed.fontSize);

            // Apply text-transform on the span element itself
            if (computed.textTransform && computed.textTransform !== 'none') {
              const transformStr = computed.textTransform;
              textTransform = (text) => applyTextTransform(text, transformStr);
            }

            // Validate: Check for margins on inline elements
            if (computed.marginLeft && parseFloat(computed.marginLeft) > 0) {
              errors.push(`Inline element <${node.tagName.toLowerCase()}> has margin-left which is not supported in PowerPoint. Remove margin from inline elements.`);
            }
            if (computed.marginRight && parseFloat(computed.marginRight) > 0) {
              errors.push(`Inline element <${node.tagName.toLowerCase()}> has margin-right which is not supported in PowerPoint. Remove margin from inline elements.`);
            }
            if (computed.marginTop && parseFloat(computed.marginTop) > 0) {
              errors.push(`Inline element <${node.tagName.toLowerCase()}> has margin-top which is not supported in PowerPoint. Remove margin from inline elements.`);
            }
            if (computed.marginBottom && parseFloat(computed.marginBottom) > 0) {
              errors.push(`Inline element <${node.tagName.toLowerCase()}> has margin-bottom which is not supported in PowerPoint. Remove margin from inline elements.`);
            }

            // Recursively process the child node. This will flatten nested spans into multiple runs.
            parseInlineFormatting(node, options, runs, textTransform);
          }
        }

        prevNodeIsText = isText;
      });

      // Trim leading space from first run and trailing space from last run
      if (runs.length > 0) {
        runs[0].text = runs[0].text.replace(/^\s+/, '');
        runs[runs.length - 1].text = runs[runs.length - 1].text.replace(/\s+$/, '');
      }

      return runs.filter(r => r.text.length > 0);
    };

    const isTransparentBg = (color) => !color || color === 'transparent' || color === 'rgba(0, 0, 0, 0)';

    const resolveSlideBackground = (rootBody) => {
      const candidates = [
        rootBody,
        document.documentElement,
        rootBody?.querySelector?.(':scope > section, :scope > div, :scope > main'),
      ].filter(Boolean);
      for (const el of candidates) {
        const style = view.getComputedStyle(el);
        const bgImage = style.backgroundImage || '';
        if (bgImage.includes('linear-gradient') || bgImage.includes('radial-gradient')) {
          return { gradient: true };
        }
        if (bgImage && bgImage !== 'none') {
          const urlMatch = bgImage.match(/url\(["']?([^"')]+)["']?\)/);
          if (urlMatch) return { type: 'image', path: urlMatch[1] };
        }
        const bgColor = style.backgroundColor;
        if (!isTransparentBg(bgColor)) {
          return { type: 'color', value: rgbToHex(bgColor) };
        }
      }
      return { type: 'color', value: 'FFFFFF' };
    };

    // Extract background from body / slide root wrapper
    const body = document.body;
    const bodyRect = body.getBoundingClientRect();
    const boxFor = (rect) => ({
      left: rect.left - bodyRect.left,
      top: rect.top - bodyRect.top,
      width: rect.width,
      height: rect.height,
    });
    const rectFor = (el) => boxFor(el.getBoundingClientRect());
    const textRectFor = (el) => {
      try {
        const range = document.createRange();
        range.selectNodeContents(el);
        const rect = boxFor(range.getBoundingClientRect());
        range.detach?.();
        if (rect.width > 0 && rect.height > 0) return rect;
      } catch {
        // Fall back to the element box when Range geometry is unavailable.
      }
      return rectFor(el);
    };

    const resolveTextColor = (computed, el) => {
      const channels = parseRgbChannels(computed.color);
      if (!channels || channels.a >= 0.2) return rgbToHex(computed.color);
      const plain = el.textContent.trim();
      if (!plain) return rgbToHex(computed.color);
      return 'E8E8E8';
    };

    const expandTextFrame = (el, rect, rotation) => {
      let { x, y, w, h } = getPositionAndSize(el, rect, rotation);
      const slideWidthPx = bodyRect.width;
      const slideHeightPx = bodyRect.height;
      const maxWPx = Math.max(8, slideWidthPx - x - 4);
      const scrollH = el.scrollHeight || 0;
      const isHeading = /^H[1-6]$/.test(el.tagName);
      const widthPad = isHeading ? Math.min(Math.max(8, w * 0.05), 32) : Math.min(Math.max(2, w * 0.02), 12);
      if (isHeading) {
        w = Math.max(w + widthPad, slideWidthPx * 0.92 - x);
      } else {
        w = Math.min(w + widthPad, maxWPx);
      }
      w = Math.min(w, maxWPx);
      const heightPad = Math.max(6, h * (isHeading ? 0.18 : 0.12));
      const maxHPx = Math.max(8, slideHeightPx - y - 4);
      if (scrollH > h + 2) h = Math.min(scrollH + heightPad, maxHPx);
      else h = Math.min(h + heightPad, maxHPx);
      return { x, y, w, h };
    };

    const readZIndex = (el) => {
      const raw = view.getComputedStyle(el).zIndex;
      if (!raw || raw === 'auto') return 0;
      const parsed = parseInt(raw, 10);
      return Number.isFinite(parsed) ? parsed : 0;
    };

    const pushElement = (entry, el) => {
      if (el) entry.zIndex = readZIndex(el);
      elements.push(entry);
    };

    const resolveListBulletColor = (ul, liElements, textHex) => {
      const ulComputed = view.getComputedStyle(ul);
      const listColor = ulComputed.listStyleColor;
      if (listColor && listColor !== 'rgba(0, 0, 0, 0)') {
        const hex = rgbToHex(listColor);
        if (hex && hex !== textHex) return hex;
      }
      for (const li of liElements) {
        try {
          const marker = view.getComputedStyle(li, '::marker');
          if (marker?.color) {
            const hex = rgbToHex(marker.color);
            if (hex && hex !== textHex) return hex;
          }
        } catch {
          // ::marker not supported in this WebView
        }
      }
      return null;
    };

    // Collect validation errors
    const errors = [];

    const bgResolved = resolveSlideBackground(body);
    if (bgResolved.gradient) {
      errors.push(
        'CSS gradients are not supported. Use Sharp to rasterize gradients as PNG images first, ' +
        'then reference with background-image: url(\'gradient.png\')',
      );
    }

    const background = bgResolved.gradient
      ? { type: 'color', value: 'FFFFFF' }
      : { type: bgResolved.type, ...(bgResolved.path ? { path: bgResolved.path } : { value: bgResolved.value }) };
    const slideBackdropHex = background.value || '0E0E12';

    // Process all elements
    const elements = [];
    const placeholders = [];
    const textTags = ['P', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'UL', 'OL', 'LI'];
    const genericInlineTextTags = new Set([
      'SPAN', 'SMALL', 'LABEL', 'A', 'CODE', 'BUTTON', 'B', 'STRONG', 'I', 'EM', 'U', 'MARK', 'SUB', 'SUP',
    ]);
    const tableCellTags = new Set(['TD', 'TH']);
    const containerTags = new Set(['DIV', 'SECTION', 'ARTICLE', 'ASIDE', ...tableCellTags]);
    const processed = new Set();

    const hasTextOwnerAncestor = (el) => {
      let parent = el.parentElement;
      while (parent && parent !== body) {
        if (textTags.includes(parent.tagName) || parent.dataset?.pptxMerge === 'true') return true;
        parent = parent.parentElement;
      }
      return false;
    };

    const hasDirectText = (el) => Array.from(el.childNodes).some(
      (node) => node.nodeType === Node.TEXT_NODE && node.textContent.replace(/\s+/g, ' ').trim(),
    );

    const isGenericTextElement = (el) => {
      if (!el.textContent.replace(/\s+/g, ' ').trim() || hasTextOwnerAncestor(el)) return false;
      if (genericInlineTextTags.has(el.tagName)) return true;
      if (!containerTags.has(el.tagName) || (!hasDirectText(el) && !tableCellTags.has(el.tagName))) return false;
      if (el.querySelector('p,h1,h2,h3,h4,h5,h6,ul,ol,li')) return false;
      return !Array.from(el.children).some((child) => containerTags.has(child.tagName));
    };

    const resolveTextAlign = (computed) => {
      if (computed.textAlign && !['start', 'auto'].includes(computed.textAlign)) {
        return computed.textAlign === 'end' ? 'right' : computed.textAlign;
      }
      if (computed.display.includes('flex')) {
        if (computed.justifyContent === 'center') return 'center';
        if (computed.justifyContent === 'flex-end' || computed.justifyContent === 'end') return 'right';
      }
      if (computed.display.includes('grid')) {
        if ((computed.justifyItems || computed.placeItems || '').includes('center')) return 'center';
        if ((computed.justifyItems || computed.placeItems || '').includes('end')) return 'right';
      }
      return 'left';
    };

    const resolveVerticalAlign = (computed, rect) => {
      if (computed.display === 'table-cell') {
        if (computed.verticalAlign === 'middle') return 'mid';
        if (computed.verticalAlign === 'bottom') return 'bottom';
      }
      if (!computed.display.includes('flex') && !computed.display.includes('grid')) {
        const lineHeight = parseFloat(computed.lineHeight || '0');
        return lineHeight >= rect.height * 0.75 ? 'mid' : 'top';
      }
      const alignment = computed.alignItems || computed.placeItems || '';
      if (alignment.includes('center')) return 'mid';
      if (alignment.includes('end')) return 'bottom';
      return 'top';
    };

    const emitTextElement = (el, type = el.tagName.toLowerCase(), exactFrame = false, rectOverride = null) => {
      const rect = rectOverride || rectFor(el);
      const text = el.textContent.replace(/\s+/g, ' ').trim();
      if (rect.width === 0 || rect.height === 0 || !text) return false;

      if (type !== 'text' && el.tagName !== 'LI' && /^[•\-\*▪▸○●◆◇■□]\s/.test(text.trimStart())) {
        errors.push(
          `Text element <${el.tagName.toLowerCase()}> starts with bullet symbol "${text.substring(0, 20)}...". ` +
          'Use <ul> or <ol> lists instead of manual bullet symbols.'
        );
        return false;
      }

      const computed = view.getComputedStyle(el);
      if (computed.display === 'none' || computed.visibility === 'hidden' || parseFloat(computed.opacity || '1') <= 0) {
        return false;
      }
      const rotation = getRotation(computed.transform);
      const textDirection = getTextDirection(computed.writingMode);
      const frame = exactFrame
        ? getPositionAndSize(el, rect, rotation)
        : expandTextFrame(el, rect, rotation);
      const isBold = computed.fontWeight === 'bold' || parseInt(computed.fontWeight, 10) >= 600;
      const lineSpacing = computed.lineHeight && computed.lineHeight !== 'normal'
        ? pxToPoints(computed.lineHeight)
        : null;

      const baseStyle = {
        fontSize: pxToPoints(computed.fontSize),
        fontFace: computed.fontFamily.split(',')[0].replace(/['"]/g, '').trim(),
        color: resolveTextColor(computed, el),
        align: resolveTextAlign(computed),
        valign: exactFrame ? resolveVerticalAlign(computed, rect) : 'top',
        lineSpacing,
        paraSpaceBefore: pxToPoints(computed.marginTop),
        paraSpaceAfter: pxToPoints(computed.marginBottom),
        margin: [
          pxToPoints(computed.paddingLeft),
          pxToPoints(computed.paddingRight),
          pxToPoints(computed.paddingBottom),
          pxToPoints(computed.paddingTop)
        ]
      };

      const transparency = extractAlpha(computed.color);
      if (transparency !== null) baseStyle.transparency = transparency;
      if (rotation !== null) baseStyle.rotate = rotation;
      if (textDirection !== null) baseStyle.vert = textDirection;

      const hasFormatting = el.querySelector('b, i, u, strong, em, span, small, label, a, code, mark, sub, sup, br');
      if (hasFormatting) {
        const transformStr = computed.textTransform;
        const runBase = {};
        if (isBold && !shouldSkipBold(computed.fontFamily)) runBase.bold = true;
        let runs = parseInlineFormatting(el, runBase, [], (str) => applyTextTransform(str, transformStr));
        const runText = runs.map((run) => run.text).join('').trim();
        if (!runText && text) {
          runs = [{ text: applyTextTransform(text, transformStr), options: { ...runBase } }];
        }

        const adjustedStyle = { ...baseStyle };
        if (adjustedStyle.lineSpacing) {
          const maxFontSize = Math.max(
            adjustedStyle.fontSize,
            ...runs.map((run) => run.options?.fontSize || 0),
          );
          if (maxFontSize > adjustedStyle.fontSize) {
            adjustedStyle.lineSpacing = maxFontSize * (adjustedStyle.lineSpacing / adjustedStyle.fontSize);
          }
        }

        pushElement({
          type,
          text: runs,
          position: {
            x: pxToInch(frame.x),
            y: pxToInch(frame.y),
            w: pxToInch(frame.w),
            h: pxToInch(frame.h),
          },
          style: adjustedStyle,
        }, el);
      } else {
        pushElement({
          type,
          text: applyTextTransform(text, computed.textTransform),
          position: {
            x: pxToInch(frame.x),
            y: pxToInch(frame.y),
            w: pxToInch(frame.w),
            h: pxToInch(frame.h),
          },
          style: {
            ...baseStyle,
            bold: isBold && !shouldSkipBold(computed.fontFamily),
            italic: computed.fontStyle === 'italic',
            underline: computed.textDecoration.includes('underline'),
          },
        }, el);
      }

      processed.add(el);
      if (type === 'text') {
        el.querySelectorAll('span,small,label,a,code,b,strong,i,em,u,mark,sub,sup').forEach((child) => processed.add(child));
      }
      return true;
    };

    document.querySelectorAll('*').forEach((el) => {
      if (processed.has(el)) return;

      // [data-pptx-merge="true"] — opt-in: merge all <p>/<h1>-<h6> descendants
      // into ONE PowerPoint text frame (single editable text box).
      // Each child paragraph becomes a run with breakLine:true at the end;
      // per-paragraph fontSize/color/bold/italic/underline are preserved as run options.
      // The container's bg/border (if any) still becomes its own shape, same as a normal div.
      if (el.tagName === 'DIV' && el.dataset && el.dataset.pptxMerge === 'true') {
        const containerRect = rectFor(el);
        if (containerRect.width === 0 || containerRect.height === 0) {
          processed.add(el);
          return;
        }

        // Reject nested merge containers — undefined behavior.
        if (el.querySelector('[data-pptx-merge="true"]')) {
          errors.push(
            `data-pptx-merge container cannot contain another data-pptx-merge container. ` +
            'Nested merge is not supported.'
          );
          processed.add(el);
          return;
        }

        const mergeComputed = view.getComputedStyle(el);

        // Container background image — same restriction as regular divs.
        if (mergeComputed.backgroundImage && mergeComputed.backgroundImage !== 'none') {
          errors.push(
            'Background images on data-pptx-merge container are not supported. ' +
            'Use solid colors or borders, or layer images via slide.addImage().'
          );
          return;
        }

        // Emit a shape for the container's bg/uniform-border (mirrors the regular div branch).
        const mHasBg = mergeComputed.backgroundColor && mergeComputed.backgroundColor !== 'rgba(0, 0, 0, 0)';
        const mBorders = [
          mergeComputed.borderTopWidth,
          mergeComputed.borderRightWidth,
          mergeComputed.borderBottomWidth,
          mergeComputed.borderLeftWidth
        ].map(b => parseFloat(b) || 0);
        const mHasBorder = mBorders.some(b => b > 0);
        const mHasUniformBorder = mHasBorder && mBorders.every(b => b === mBorders[0]);

        if (mHasBg || mHasUniformBorder) {
          elements.push({
            type: 'shape',
            text: '',
            position: {
              x: pxToInch(containerRect.left),
              y: pxToInch(containerRect.top),
              w: pxToInch(containerRect.width),
              h: pxToInch(containerRect.height)
            },
            shape: {
              fill: mHasBg ? rgbToHex(mergeComputed.backgroundColor) : null,
              transparency: mHasBg ? extractAlpha(mergeComputed.backgroundColor) : null,
              line: mHasUniformBorder ? {
                color: rgbToHex(mergeComputed.borderColor),
                width: pxToPoints(mergeComputed.borderWidth)
              } : null,
              rectRadius: (() => {
                const radius = mergeComputed.borderRadius;
                const radiusValue = parseFloat(radius);
                if (radiusValue === 0) return 0;
                if (radius.includes('%')) {
                  if (radiusValue >= 50) return 1;
                  const minDim = Math.min(containerRect.width, containerRect.height);
                  return (radiusValue / 100) * pxToInch(minDim);
                }
                if (radius.includes('pt')) return radiusValue / 72;
                return radiusValue / PX_PER_IN;
              })(),
              shadow: parseBoxShadow(mergeComputed.boxShadow)
            }
          });
        }

        // Collect <p>/<h*> descendants in document order.
        const textDescendants = Array.from(el.querySelectorAll('p, h1, h2, h3, h4, h5, h6'));
        if (textDescendants.length === 0) {
          errors.push(
            `data-pptx-merge container has no <p>/<h*> children to merge. ` +
            'Remove the data-pptx-merge attribute or add text elements.'
          );
          processed.add(el);
          return;
        }

        // Use the first text element's computed style as the textbox-level base
        // (align / lineSpacing / paraSpace are paragraph/textbox-level in pptxgenjs, not per-run).
        const firstComputed = view.getComputedStyle(textDescendants[0]);
        const baseStyle = {
          fontSize: pxToPoints(firstComputed.fontSize),
          fontFace: firstComputed.fontFamily.split(',')[0].replace(/['"]/g, '').trim(),
          color: rgbToHex(firstComputed.color),
          align: firstComputed.textAlign === 'start' ? 'left' : firstComputed.textAlign,
          lineSpacing: firstComputed.lineHeight && firstComputed.lineHeight !== 'normal'
            ? pxToPoints(firstComputed.lineHeight)
            : null,
          paraSpaceBefore: 0,
          paraSpaceAfter: 0,
          // Container padding becomes the textbox internal margin (PptxGenJS: [left, right, bottom, top]).
          margin: [
            pxToPoints(mergeComputed.paddingLeft),
            pxToPoints(mergeComputed.paddingRight),
            pxToPoints(mergeComputed.paddingBottom),
            pxToPoints(mergeComputed.paddingTop)
          ]
        };
        const baseTransparency = extractAlpha(firstComputed.color);
        if (baseTransparency !== null) baseStyle.transparency = baseTransparency;

        // Build the merged runs.
        const mergedRuns = [];
        textDescendants.forEach((textEl, idx) => {
          const isLast = idx === textDescendants.length - 1;
          const tComputed = view.getComputedStyle(textEl);
          const transformStr = tComputed.textTransform;

          // Per-paragraph style overrides — only include if they differ from base.
          const elemFontSize = pxToPoints(tComputed.fontSize);
          const elemFontFace = tComputed.fontFamily.split(',')[0].replace(/['"]/g, '').trim();
          const elemColor = rgbToHex(tComputed.color);
          const elemBold = tComputed.fontWeight === 'bold' || parseInt(tComputed.fontWeight) >= 600;
          const elemItalic = tComputed.fontStyle === 'italic';
          const elemUnderline = tComputed.textDecoration.includes('underline');

          const runBaseOptions = {};
          if (elemFontSize !== baseStyle.fontSize) runBaseOptions.fontSize = elemFontSize;
          if (elemFontFace !== baseStyle.fontFace) runBaseOptions.fontFace = elemFontFace;
          if (elemColor !== baseStyle.color) runBaseOptions.color = elemColor;
          if (elemBold && !shouldSkipBold(tComputed.fontFamily)) runBaseOptions.bold = true;
          if (elemItalic) runBaseOptions.italic = true;
          if (elemUnderline) runBaseOptions.underline = true;

          const hasInline = textEl.querySelector('b, i, u, strong, em, span, small, label, a, code, mark, sub, sup, br');
          let runs;
          if (hasInline) {
            runs = parseInlineFormatting(
              textEl,
              runBaseOptions,
              [],
              (str) => applyTextTransform(str, transformStr)
            );
          } else {
            const txt = applyTextTransform(textEl.textContent.trim(), transformStr);
            if (!txt) return;
            runs = [{ text: txt, options: { ...runBaseOptions } }];
          }

          if (runs.length > 0 && !isLast) {
            runs[runs.length - 1].options.breakLine = true;
          }
          mergedRuns.push(...runs);
          processed.add(textEl);
        });

        if (mergedRuns.length === 0) {
          processed.add(el);
          return;
        }

        elements.push({
          type: 'merged-text',
          items: mergedRuns,
          position: {
            x: pxToInch(containerRect.left),
            y: pxToInch(containerRect.top),
            w: pxToInch(containerRect.width),
            h: pxToInch(containerRect.height)
          },
          style: baseStyle
        });

        processed.add(el);
        return;
      }

      const genericTextExtracted = isGenericTextElement(el)
        ? emitTextElement(
            el,
            'text',
            true,
            genericInlineTextTags.has(el.tagName) ? textRectFor(el) : null,
          )
        : false;

      // Text tags with decorative boxes (pills, chips) become a shape + text.
      if (textTags.includes(el.tagName) || genericInlineTextTags.has(el.tagName)) {
        const computed = view.getComputedStyle(el);
        const hasBg = computed.backgroundColor && computed.backgroundColor !== 'rgba(0, 0, 0, 0)';
        const hasBorder = (computed.borderWidth && parseFloat(computed.borderWidth) > 0) ||
                          (computed.borderTopWidth && parseFloat(computed.borderTopWidth) > 0) ||
                          (computed.borderRightWidth && parseFloat(computed.borderRightWidth) > 0) ||
                          (computed.borderBottomWidth && parseFloat(computed.borderBottomWidth) > 0) ||
                          (computed.borderLeftWidth && parseFloat(computed.borderLeftWidth) > 0);
        const hasShadow = computed.boxShadow && computed.boxShadow !== 'none';

        if (hasBg || hasBorder || hasShadow) {
          const decoRect = rectFor(el);
          if (decoRect.width > 0 && decoRect.height > 0) {
            const borders = [computed.borderTopWidth, computed.borderRightWidth, computed.borderBottomWidth, computed.borderLeftWidth]
              .map((b) => parseFloat(b) || 0);
            const hasUniformBorder = borders.some((b) => b > 0) && borders.every((b) => b === borders[0]);
            const solid = hasBg ? resolveSolidFill(computed.backgroundColor, slideBackdropHex) : { fill: null, transparency: null };
            if (solid.fill || hasUniformBorder) {
              const radius = computed.borderRadius;
              const radiusValue = parseFloat(radius);
              pushElement({
                type: 'shape',
                text: '',
                position: {
                  x: pxToInch(decoRect.left),
                  y: pxToInch(decoRect.top),
                  w: pxToInch(decoRect.width),
                  h: pxToInch(decoRect.height),
                },
                shape: {
                  fill: solid.fill,
                  transparency: solid.transparency,
                  line: hasUniformBorder ? {
                    color: rgbToHex(computed.borderColor),
                    width: pxToPoints(computed.borderWidth),
                  } : null,
                  rectRadius: (() => {
                    if (!radiusValue) return 0;
                    if (radius.includes('%')) {
                      if (radiusValue >= 50) return 1;
                      const minDim = Math.min(decoRect.width, decoRect.height);
                      return (radiusValue / 100) * pxToInch(minDim);
                    }
                    if (radius.includes('pt')) return radiusValue / 72;
                    return radiusValue / PX_PER_IN;
                  })(),
                  shadow: parseBoxShadow(computed.boxShadow),
                },
              }, el);
            }
          }
        }
      }

      // Extract placeholder elements (for charts, etc.)
      if (el.className && el.className.includes('placeholder')) {
        const rect = rectFor(el);
        if (rect.width === 0 || rect.height === 0) {
          errors.push(
            `Placeholder "${el.id || 'unnamed'}" has ${rect.width === 0 ? 'width: 0' : 'height: 0'}. Check the layout CSS.`
          );
        } else {
          placeholders.push({
            id: el.id || `placeholder-${placeholders.length}`,
            x: pxToInch(rect.left),
            y: pxToInch(rect.top),
            w: pxToInch(rect.width),
            h: pxToInch(rect.height)
          });
        }
        processed.add(el);
        return;
      }

      // Extract images
      if (el.tagName === 'IMG') {
        const rect = rectFor(el);
        if (rect.width > 0 && rect.height > 0) {
          elements.push({
            type: 'image',
            src: el.src,
            position: {
              x: pxToInch(rect.left),
              y: pxToInch(rect.top),
              w: pxToInch(rect.width),
              h: pxToInch(rect.height)
            }
          });
          processed.add(el);
          return;
        }
      }

      // Extract container blocks with backgrounds/borders as shapes
      const isContainer = containerTags.has(el.tagName);
      if (isContainer) {
        const computed = view.getComputedStyle(el);
        const hasBg = computed.backgroundColor && computed.backgroundColor !== 'rgba(0, 0, 0, 0)';

        // Check for background images on shapes
        const bgImage = computed.backgroundImage;
        if (bgImage && bgImage !== 'none') {
          errors.push(
            'Background images on DIV elements are not supported. ' +
            'Use solid colors or borders for shapes, or use slide.addImage() in PptxGenJS to layer images.'
          );
          return;
        }

        // Check for borders - both uniform and partial
        const borderTop = computed.borderTopWidth;
        const borderRight = computed.borderRightWidth;
        const borderBottom = computed.borderBottomWidth;
        const borderLeft = computed.borderLeftWidth;
        const borders = [borderTop, borderRight, borderBottom, borderLeft].map(b => parseFloat(b) || 0);
        const hasBorder = borders.some(b => b > 0);
        const hasUniformBorder = hasBorder && borders.every(b => b === borders[0]);
        const borderLines = [];

        if (hasBorder && !hasUniformBorder) {
          const rect = rectFor(el);
          const x = pxToInch(rect.left);
          const y = pxToInch(rect.top);
          const w = pxToInch(rect.width);
          const h = pxToInch(rect.height);

          // Collect lines to add after shape (inset by half the line width to center on edge)
          if (parseFloat(borderTop) > 0) {
            const widthPt = pxToPoints(borderTop);
            const inset = (widthPt / 72) / 2; // Convert points to inches, then half
            borderLines.push({
              type: 'line',
              x1: x, y1: y + inset, x2: x + w, y2: y + inset,
              width: widthPt,
              color: rgbToHex(computed.borderTopColor)
            });
          }
          if (parseFloat(borderRight) > 0) {
            const widthPt = pxToPoints(borderRight);
            const inset = (widthPt / 72) / 2;
            borderLines.push({
              type: 'line',
              x1: x + w - inset, y1: y, x2: x + w - inset, y2: y + h,
              width: widthPt,
              color: rgbToHex(computed.borderRightColor)
            });
          }
          if (parseFloat(borderBottom) > 0) {
            const widthPt = pxToPoints(borderBottom);
            const inset = (widthPt / 72) / 2;
            borderLines.push({
              type: 'line',
              x1: x, y1: y + h - inset, x2: x + w, y2: y + h - inset,
              width: widthPt,
              color: rgbToHex(computed.borderBottomColor)
            });
          }
          if (parseFloat(borderLeft) > 0) {
            const widthPt = pxToPoints(borderLeft);
            const inset = (widthPt / 72) / 2;
            borderLines.push({
              type: 'line',
              x1: x + inset, y1: y, x2: x + inset, y2: y + h,
              width: widthPt,
              color: rgbToHex(computed.borderLeftColor)
            });
          }
        }

        if (hasBg || hasBorder) {
          const rect = rectFor(el);
          const coversSlide = rect.width >= bodyRect.width * 0.97
            && rect.height >= bodyRect.height * 0.97;
          if (coversSlide && hasBg) {
            processed.add(el);
            return;
          }
          if (rect.width > 0 && rect.height > 0) {
            const shadow = parseBoxShadow(computed.boxShadow);

            // Only add shape if there's background or uniform border
            if (hasBg || hasUniformBorder) {
              const solid = hasBg
                ? resolveSolidFill(computed.backgroundColor, slideBackdropHex)
                : { fill: null, transparency: null };
              let fillHex = solid.fill;
              let fillTransparency = solid.transparency;
              if (!fillHex && hasUniformBorder) {
                fillHex = rgbToHex(computed.borderColor) || '2A2A30';
                fillTransparency = fillTransparency ?? 88;
              }
              pushElement({
                type: 'shape',
                text: '',  // Shape only - child text elements render on top
                position: {
                  x: pxToInch(rect.left),
                  y: pxToInch(rect.top),
                  w: pxToInch(rect.width),
                  h: pxToInch(rect.height)
                },
                shape: {
                  fill: fillHex,
                  transparency: fillTransparency,
                  line: hasUniformBorder ? {
                    color: rgbToHex(computed.borderColor),
                    width: pxToPoints(computed.borderWidth)
                  } : null,
                  // Convert border-radius to rectRadius (in inches)
                  // % values: 50%+ = circle (1), <50% = percentage of min dimension
                  // pt values: divide by 72 (72pt = 1 inch)
                  // px values: divide by 96 (96px = 1 inch)
                  rectRadius: (() => {
                    const radius = computed.borderRadius;
                    const radiusValue = parseFloat(radius);
                    if (radiusValue === 0) return 0;

                    if (radius.includes('%')) {
                      if (radiusValue >= 50) return 1;
                      // Calculate percentage of smaller dimension
                      const minDim = Math.min(rect.width, rect.height);
                      return (radiusValue / 100) * pxToInch(minDim);
                    }

                    if (radius.includes('pt')) return radiusValue / 72;
                    return radiusValue / PX_PER_IN;
                  })(),
                  shadow: shadow
                }
              }, el);
            }

            // Add partial border lines
            borderLines.forEach((line) => pushElement(line, el));

            processed.add(el);
            return;
          }
        }
      }

      if (genericTextExtracted) return;

      // Extract bullet lists as single text block
      if (el.tagName === 'UL' || el.tagName === 'OL') {
        const rect = rectFor(el);
        if (rect.width === 0 || rect.height === 0) return;

        const liElements = Array.from(el.querySelectorAll('li'));
        const items = [];
        const ulComputed = view.getComputedStyle(el);
        const ulPaddingLeftPt = pxToPoints(ulComputed.paddingLeft);

        // Split: margin-left for bullet position, indent for text position
        // margin-left + indent = ul padding-left
        const marginLeft = ulPaddingLeftPt * 0.5;
        const textIndent = ulPaddingLeftPt * 0.5;

        const computed = view.getComputedStyle(liElements[0] || el);
        const textHex = rgbToHex(computed.color);
        const bulletColor = resolveListBulletColor(el, liElements, textHex);

        liElements.forEach((li, idx) => {
          const isLast = idx === liElements.length - 1;
          const runs = parseInlineFormatting(li, { breakLine: false });
          // Clean manual bullets from first run
          if (runs.length > 0) {
            runs[0].text = runs[0].text.replace(/^[•\-\*▪▸]\s*/, '');
            runs[0].options.bullet = { indent: textIndent };
          }
          if (runs.length > 0 && bulletColor && bulletColor !== textHex) {
            runs.unshift({
              text: '\u200B',
              options: {
                bullet: { indent: textIndent },
                color: bulletColor,
                fontSize: runs[0]?.options?.fontSize || pxToPoints(computed.fontSize),
                breakLine: false,
              },
            });
          }
          // Set breakLine on last run
          if (runs.length > 0 && !isLast) {
            runs[runs.length - 1].options.breakLine = true;
          }
          items.push(...runs);
        });

        const listFrame = expandTextFrame(el, rect, null);

        pushElement({
          type: 'list',
          items: items,
          position: {
            x: pxToInch(listFrame.x),
            y: pxToInch(listFrame.y),
            w: pxToInch(listFrame.w),
            h: pxToInch(listFrame.h)
          },
          style: {
            fontSize: pxToPoints(computed.fontSize),
            fontFace: computed.fontFamily.split(',')[0].replace(/['"]/g, '').trim(),
            color: textHex,
            bulletColor,
            transparency: extractAlpha(computed.color),
            align: computed.textAlign === 'start' ? 'left' : computed.textAlign,
            lineSpacing: computed.lineHeight && computed.lineHeight !== 'normal' ? pxToPoints(computed.lineHeight) : null,
            paraSpaceBefore: 0,
            paraSpaceAfter: pxToPoints(computed.marginBottom),
            // PptxGenJS margin array is [left, right, bottom, top]
            margin: [marginLeft, 0, 0, 0]
          }
        }, el);

        liElements.forEach(li => processed.add(li));
        processed.add(el);
        return;
      }

      // Extract text elements (P, H1, H2, etc.)
      if (!textTags.includes(el.tagName)) return;

      emitTextElement(el);
    });

    const paintRank = (type) => {
      if (type === 'shape') return 0;
      if (type === 'line') return 1;
      if (type === 'image') return 2;
      return 3;
    };
    elements.sort((a, b) => {
      const z = (a.zIndex ?? 0) - (b.zIndex ?? 0);
      if (z !== 0) return z;
      return paintRank(a.type) - paintRank(b.type);
    });

    return { background, elements, placeholders, errors };
  
}
