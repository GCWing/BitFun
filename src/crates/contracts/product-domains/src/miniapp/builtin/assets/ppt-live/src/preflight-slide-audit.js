import { EditableExportError } from './editable-slide-scene.js';

const VISIBLE_UNSUPPORTED_SELECTOR = [
  'canvas', 'video', 'audio', 'iframe', 'object', 'embed',
  'frame', 'frameset', 'portal', 'applet',
  'input', 'button', 'select', 'textarea', 'progress', 'meter', 'dialog',
  'link[rel~="stylesheet" i]',
].join(',');

function sourceIdOf(element, slideNumber) {
  return element?.getAttribute?.('data-pptx-source-id')
    || element?.id
    || `slide-${slideNumber}`;
}

function fail(slideNumber, element, code, message) {
  throw new EditableExportError({
    slideNumber,
    sourceId: sourceIdOf(element, slideNumber),
    code,
    message,
  });
}

function canonicalizeCssSyntax(value) {
  const input = String(value || '');
  let output = '';
  let quote = null;
  for (let index = 0; index < input.length;) {
    const character = input[index];
    if (quote) {
      output += character;
      index += 1;
      if (character === '\\' && index < input.length) {
        output += input[index];
        index += 1;
      } else if (character === quote) {
        quote = null;
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      output += character;
      index += 1;
      continue;
    }
    if (character === '/' && input[index + 1] === '*') {
      const end = input.indexOf('*/', index + 2);
      index = end < 0 ? input.length : end + 2;
      continue;
    }
    if (character !== '\\') {
      output += character;
      index += 1;
      continue;
    }
    index += 1;
    if (index >= input.length) break;
    if (input[index] === '\n' || input[index] === '\r' || input[index] === '\f') {
      if (input[index] === '\r' && input[index + 1] === '\n') index += 1;
      index += 1;
      continue;
    }
    const hex = input.slice(index).match(/^[0-9a-f]{1,6}/i)?.[0];
    if (hex) {
      const codePoint = Number.parseInt(hex, 16);
      output += String.fromCodePoint(
        codePoint === 0 || codePoint > 0x10FFFF ? 0xFFFD : codePoint,
      );
      index += hex.length;
      if (/[\t\n\f\r ]/.test(input[index] || '')) index += 1;
      continue;
    }
    output += input[index];
    index += 1;
  }
  return output;
}

function cssValueHasUrlToken(value) {
  const css = canonicalizeCssSyntax(value);
  let quote = null;
  for (let index = 0; index < css.length;) {
    const character = css[index];
    if (quote) {
      if (character === '\\') index += 2;
      else {
        if (character === quote) quote = null;
        index += 1;
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      index += 1;
      continue;
    }
    if (!/[a-z_-]/i.test(character)) {
      index += 1;
      continue;
    }
    const start = index;
    while (index < css.length && /[a-z0-9_-]/i.test(css[index])) index += 1;
    const identifier = css.slice(start, index).toLowerCase();
    while (index < css.length && /\s/.test(css[index])) index += 1;
    if (identifier === 'url' && css[index] === '(') return true;
  }
  return false;
}

function referencedCustomProperties(value) {
  const css = canonicalizeCssSyntax(value);
  const references = [];
  let quote = null;
  for (let index = 0; index < css.length;) {
    const character = css[index];
    if (quote) {
      if (character === '\\') index += 2;
      else {
        if (character === quote) quote = null;
        index += 1;
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      index += 1;
      continue;
    }
    const match = css.slice(index).match(/^var\s*\(\s*(--[a-z0-9_-]+)/i);
    if (match) {
      references.push(match[1]);
      index += match[0].length;
      continue;
    }
    index += 1;
  }
  return references;
}

function valueOrVariableHasUrl(value, declarations, seen = new Set()) {
  if (cssValueHasUrlToken(value)) return true;
  return referencedCustomProperties(value).some((property) => {
    if (seen.has(property) || !declarations.has(property)) return false;
    const nextSeen = new Set(seen);
    nextSeen.add(property);
    return valueOrVariableHasUrl(declarations.get(property), declarations, nextSeen);
  });
}

function textShadowIsVisible(value, declarations, seen = new Set()) {
  const normalized = canonicalizeCssSyntax(value).trim().toLowerCase();
  if (!normalized || normalized === 'none') return false;
  const references = referencedCustomProperties(value);
  if (!references.length) return true;
  return references.some((property) => {
    if (seen.has(property) || !declarations.has(property)) return false;
    const nextSeen = new Set(seen);
    nextSeen.add(property);
    return textShadowIsVisible(declarations.get(property), declarations, nextSeen);
  });
}

function styleDeclarations(style) {
  const declarations = new Map();
  for (let index = 0; index < style.length; index += 1) {
    const property = style[index].toLowerCase();
    declarations.set(property, {
      value: style.getPropertyValue(property),
      important: style.getPropertyPriority(property) === 'important',
    });
  }
  return declarations;
}

function parsedDeclarations(doc, cssText) {
  const probe = doc.createElement('div');
  probe.style.cssText = canonicalizeCssSyntax(cssText);
  return styleDeclarations(probe.style);
}

function compareSpecificity(left, right) {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return 0;
}

function applyCascadeDeclaration(target, property, declaration) {
  const current = target.get(property);
  const shouldReplace = !current
    || Number(declaration.important) > Number(current.important)
    || (declaration.important === current.important
      && (compareSpecificity(declaration.specificity, current.specificity) > 0
        || (compareSpecificity(declaration.specificity, current.specificity) === 0
          && declaration.sourceOrder >= current.sourceOrder)));
  if (shouldReplace) target.set(property, declaration);
}

function splitSelectorList(selectorText) {
  const selectors = [];
  let start = 0;
  let quote = null;
  let squareDepth = 0;
  let roundDepth = 0;
  for (let index = 0; index < selectorText.length; index += 1) {
    const character = selectorText[index];
    if (quote) {
      if (character === '\\') index += 1;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'") quote = character;
    else if (character === '[') squareDepth += 1;
    else if (character === ']') squareDepth = Math.max(0, squareDepth - 1);
    else if (character === '(') roundDepth += 1;
    else if (character === ')') roundDepth = Math.max(0, roundDepth - 1);
    else if (character === ',' && squareDepth === 0 && roundDepth === 0) {
      selectors.push(selectorText.slice(start, index).trim());
      start = index + 1;
    }
  }
  selectors.push(selectorText.slice(start).trim());
  return selectors.filter(Boolean);
}

function matchingParenthesis(value, openingIndex) {
  let depth = 1;
  let quote = null;
  for (let index = openingIndex + 1; index < value.length; index += 1) {
    const character = value[index];
    if (quote) {
      if (character === '\\') index += 1;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'") quote = character;
    else if (character === '(') depth += 1;
    else if (character === ')') {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  return value.length - 1;
}

function replaceCssUrlsForProjection(value) {
  const css = canonicalizeCssSyntax(value);
  let output = '';
  let quote = null;
  for (let index = 0; index < css.length;) {
    const character = css[index];
    if (quote) {
      output += character;
      index += 1;
      if (character === '\\' && index < css.length) {
        output += css[index];
        index += 1;
      } else if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      output += character;
      index += 1;
      continue;
    }
    if (!/[a-z_-]/i.test(character)) {
      output += character;
      index += 1;
      continue;
    }
    const start = index;
    while (index < css.length && /[a-z0-9_-]/i.test(css[index])) index += 1;
    const identifier = css.slice(start, index);
    let opening = index;
    while (opening < css.length && /\s/.test(css[opening])) opening += 1;
    if (identifier.toLowerCase() !== 'url' || css[opening] !== '(') {
      output += css.slice(start, index);
      continue;
    }
    const closing = matchingParenthesis(css, opening);
    output += 'url("#__pptx_paint_server_probe__")';
    index = closing + 1;
  }
  return output;
}

function stripCssImports(value) {
  const css = String(value || '');
  let output = '';
  let quote = null;
  for (let index = 0; index < css.length;) {
    const character = css[index];
    if (quote) {
      output += character;
      index += 1;
      if (character === '\\' && index < css.length) {
        output += css[index];
        index += 1;
      } else if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      output += character;
      index += 1;
      continue;
    }
    const importMatch = css.slice(index).match(/^@import\b/i);
    if (!importMatch) {
      output += character;
      index += 1;
      continue;
    }
    index += importMatch[0].length;
    let parentheses = 0;
    let importQuote = null;
    while (index < css.length) {
      const importCharacter = css[index];
      if (importQuote) {
        if (importCharacter === '\\') index += 2;
        else {
          if (importCharacter === importQuote) importQuote = null;
          index += 1;
        }
      } else if (importCharacter === '"' || importCharacter === "'") {
        importQuote = importCharacter;
        index += 1;
      } else if (importCharacter === '(') {
        parentheses += 1;
        index += 1;
      } else if (importCharacter === ')') {
        parentheses = Math.max(0, parentheses - 1);
        index += 1;
      } else if (importCharacter === ';' && parentheses === 0) {
        index += 1;
        break;
      } else index += 1;
    }
  }
  return output;
}

function maxSpecificity(selectors) {
  return selectors.reduce((maximum, selector) => {
    const specificity = selectorSpecificity(selector);
    return compareSpecificity(specificity, maximum) > 0 ? specificity : maximum;
  }, [0, 0, 0, 0]);
}

function selectorSpecificity(selector) {
  const css = canonicalizeCssSyntax(selector);
  const specificity = [0, 0, 0, 0];
  let expectsType = true;
  for (let index = 0; index < css.length;) {
    const character = css[index];
    if (/\s/.test(character) || ['>', '+', '~', ','].includes(character)) {
      expectsType = true;
      index += 1;
      continue;
    }
    if (character === '#') {
      specificity[1] += 1;
      index += 1;
      while (index < css.length && /[\w-]/.test(css[index])) index += 1;
      expectsType = false;
      continue;
    }
    if (character === '.') {
      specificity[2] += 1;
      index += 1;
      while (index < css.length && /[\w-]/.test(css[index])) index += 1;
      expectsType = false;
      continue;
    }
    if (character === '[') {
      specificity[2] += 1;
      let quote = null;
      index += 1;
      while (index < css.length) {
        if (quote) {
          if (css[index] === '\\') index += 2;
          else {
            if (css[index] === quote) quote = null;
            index += 1;
          }
        } else if (css[index] === '"' || css[index] === "'") {
          quote = css[index];
          index += 1;
        } else if (css[index] === ']') {
          index += 1;
          break;
        } else index += 1;
      }
      expectsType = false;
      continue;
    }
    if (character === ':') {
      const pseudoElement = css[index + 1] === ':';
      index += pseudoElement ? 2 : 1;
      const nameStart = index;
      while (index < css.length && /[\w-]/.test(css[index])) index += 1;
      const name = css.slice(nameStart, index).toLowerCase();
      const legacyPseudoElement = ['before', 'after', 'first-line', 'first-letter'].includes(name);
      if (pseudoElement || legacyPseudoElement) specificity[3] += 1;
      else if (!['where', 'is', 'not', 'has'].includes(name)) specificity[2] += 1;
      if (css[index] === '(') {
        const closing = matchingParenthesis(css, index);
        const argument = css.slice(index + 1, closing);
        if (['is', 'not', 'has'].includes(name)) {
          const nested = maxSpecificity(splitSelectorList(argument));
          specificity[1] += nested[1];
          specificity[2] += nested[2];
          specificity[3] += nested[3];
        } else if (['nth-child', 'nth-last-child'].includes(name)) {
          const ofMatch = argument.match(/\bof\b([\s\S]*)$/i);
          if (ofMatch) {
            const nested = maxSpecificity(splitSelectorList(ofMatch[1]));
            specificity[1] += nested[1];
            specificity[2] += nested[2];
            specificity[3] += nested[3];
          }
        }
        index = closing + 1;
      }
      expectsType = false;
      continue;
    }
    if (character === '*') {
      expectsType = false;
      index += 1;
      continue;
    }
    if (expectsType && /[a-z_-]/i.test(character)) {
      specificity[3] += 1;
      while (index < css.length && /[\w-]/.test(css[index])) index += 1;
      expectsType = false;
      continue;
    }
    index += 1;
  }
  return specificity;
}

function matchingSelectorSpecificity(element, selectorText) {
  const matchingSelectors = splitSelectorList(selectorText).filter((selector) => {
    try {
      return element.matches(selector);
    } catch {
      return false;
    }
  });
  return matchingSelectors.length ? maxSpecificity(matchingSelectors) : null;
}

function walkCssRules(rules, visit) {
  for (const rule of rules || []) {
    if (rule.selectorText && rule.style) visit(rule);
    if (rule.cssRules) walkCssRules(rule.cssRules, visit);
  }
}

function findCssOpeningBrace(css, start) {
  let quote = null;
  let parentheses = 0;
  for (let index = start; index < css.length; index += 1) {
    const character = css[index];
    if (quote) {
      if (character === '\\') index += 1;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'") quote = character;
    else if (character === '(') parentheses += 1;
    else if (character === ')') parentheses = Math.max(0, parentheses - 1);
    else if (character === '{' && parentheses === 0) return index;
    else if (character === ';' && parentheses === 0) return -index - 2;
  }
  return -1;
}

function findCssClosingBrace(css, openingBrace) {
  let quote = null;
  let depth = 1;
  for (let index = openingBrace + 1; index < css.length; index += 1) {
    const character = css[index];
    if (quote) {
      if (character === '\\') index += 1;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'") quote = character;
    else if (character === '{') depth += 1;
    else if (character === '}') {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  return -1;
}

function walkCssTextRules(cssText, visit) {
  const css = canonicalizeCssSyntax(cssText);
  let cursor = 0;
  while (cursor < css.length) {
    while (cursor < css.length && /[\s;]/.test(css[cursor])) cursor += 1;
    if (cursor >= css.length) break;
    const openingResult = findCssOpeningBrace(css, cursor);
    if (openingResult < 0) {
      if (openingResult < -1) {
        cursor = -openingResult - 1;
        continue;
      }
      break;
    }
    const closingBrace = findCssClosingBrace(css, openingResult);
    if (closingBrace < 0) break;
    const prelude = css.slice(cursor, openingResult).trim();
    const block = css.slice(openingResult + 1, closingBrace);
    if (/^@(media|supports|layer|container|document)\b/i.test(prelude)) {
      walkCssTextRules(block, visit);
    } else if (!prelude.startsWith('@')) {
      visit(prelude, block);
    }
    cursor = closingBrace + 1;
  }
}

function collectStylesheetDeclarations(parsed) {
  const byElement = new Map();
  let sourceOrder = 0;
  for (const authoredStyle of parsed.querySelectorAll('style')) {
    const cssDocument = parsed.implementation.createHTMLDocument('');
    const style = cssDocument.createElement('style');
    style.textContent = canonicalizeCssSyntax(authoredStyle.textContent || '');
    cssDocument.head.appendChild(style);
    const mergeRule = (selectorText, declarationsToMerge) => {
      let targets;
      try {
        targets = parsed.querySelectorAll(selectorText);
      } catch {
        return;
      }
      for (const target of targets) {
        const specificity = matchingSelectorSpecificity(target, selectorText);
        if (!specificity) continue;
        const declarations = byElement.get(target) || new Map();
        const entries = declarationsToMerge instanceof Map
          ? declarationsToMerge
          : styleDeclarations(declarationsToMerge);
        for (const [property, declaration] of entries) {
          applyCascadeDeclaration(declarations, property, {
            ...declaration,
            specificity,
            sourceOrder,
          });
        }
        byElement.set(target, declarations);
      }
      sourceOrder += 1;
    };
    if (style.sheet?.cssRules) {
      walkCssRules(style.sheet.cssRules, (rule) => mergeRule(rule.selectorText, rule.style));
    } else {
      walkCssTextRules(authoredStyle.textContent || '', (selectorText, declarationText) => {
        mergeRule(selectorText, parsedDeclarations(parsed, declarationText));
      });
    }
  }
  for (const element of parsed.querySelectorAll('svg [fill], svg [stroke], svg[fill], svg[stroke]')) {
    const declarations = byElement.get(element) || new Map();
    for (const property of ['fill', 'stroke']) {
      if (!element.hasAttribute(property)) continue;
      applyCascadeDeclaration(declarations, property, {
        value: canonicalizeCssSyntax(element.getAttribute(property)),
        important: false,
        specificity: [0, 0, 0, 0],
        sourceOrder: -1,
      });
    }
    byElement.set(element, declarations);
  }
  for (const element of parsed.querySelectorAll('[style]')) {
    const declarations = byElement.get(element) || new Map();
    for (const [property, declaration] of parsedDeclarations(
      parsed,
      element.getAttribute('style') || '',
    )) {
      applyCascadeDeclaration(declarations, property, {
        ...declaration,
        specificity: [1, 0, 0, 0],
        sourceOrder,
      });
    }
    byElement.set(element, declarations);
    sourceOrder += 1;
  }
  return byElement;
}

function declarationsWithInheritedCustomProperties(element, declarationsByElement) {
  const declarations = new Map();
  const ancestors = [];
  let current = element.parentElement;
  while (current) {
    ancestors.unshift(current);
    current = current.parentElement;
  }
  for (const ancestor of ancestors) {
    for (const [property, declaration] of declarationsByElement.get(ancestor) || []) {
      if (property.startsWith('--') || ['fill', 'stroke', 'text-shadow'].includes(property)) {
        declarations.set(property, declaration.value);
      }
    }
  }
  for (const [property, declaration] of declarationsByElement.get(element) || []) {
    declarations.set(property, declaration.value);
  }
  return declarations;
}

function auditAuthoredStyles(parsed, slideNumber) {
  const declarationsByElement = collectStylesheetDeclarations(parsed);

  for (const [element] of declarationsByElement) {
    const declarations = declarationsWithInheritedCustomProperties(
      element,
      declarationsByElement,
    );
    if (element.closest?.('svg')) {
      for (const property of ['fill', 'stroke']) {
        const value = declarations.get(property);
        if (value && valueOrVariableHasUrl(value, declarations)) {
          fail(
            slideNumber,
            element,
            'svg_paint_server_unsupported',
            'SVG paint-server fills and strokes cannot be represented as editable PowerPoint paint.',
          );
        }
      }
    }
    const textShadow = declarations.get('text-shadow');
    if (textShadow && textShadowIsVisible(textShadow, declarations)) {
      fail(
        slideNumber,
        element,
        'text_shadow_unsupported',
        'CSS text-shadow cannot be represented as editable PowerPoint text.',
      );
    }
  }
}

function makeMountedStyleProjection(parsed) {
  const ownerDocument = globalThis.document;
  if (!ownerDocument?.body || typeof globalThis.getComputedStyle !== 'function') return null;

  const host = ownerDocument.createElement('div');
  host.setAttribute('aria-hidden', 'true');
  host.style.cssText = [
    'position:fixed',
    'left:-30000px',
    'top:0',
    'width:1280px',
    'height:720px',
    'overflow:hidden',
    'opacity:0',
    'pointer-events:none',
  ].join(';');
  const projectionRoot = host.attachShadow?.({ mode: 'open' }) || host;
  const indexedElements = [...parsed.querySelectorAll('*')];
  indexedElements.forEach((element, index) => {
    element.setAttribute('data-pptx-preflight-index', String(index));
  });

  for (const authoredStyle of parsed.querySelectorAll('style')) {
    const style = ownerDocument.createElement('style');
    style.textContent = stripCssImports(
      replaceCssUrlsForProjection(authoredStyle.textContent || ''),
    );
    projectionRoot.appendChild(style);
  }
  const controlStyle = ownerDocument.createElement('style');
  controlStyle.textContent = [
    '#pptx-cascade-control { fill:url("#__pptx_specificity_control__"); }',
    '.pptx-cascade-control { fill:#fff;stroke:url("#__pptx_important_control__") !important; }',
    '#pptx-cascade-control { stroke:#fff; }',
  ].join('');
  projectionRoot.appendChild(controlStyle);

  const body = ownerDocument.importNode(parsed.body, true);
  body.querySelectorAll('style,script').forEach((element) => element.remove());
  for (const element of body.querySelectorAll('*')) {
    if (element.hasAttribute('style')) {
      element.setAttribute(
        'style',
        replaceCssUrlsForProjection(element.getAttribute('style') || ''),
      );
    }
    for (const property of ['fill', 'stroke']) {
      if (element.hasAttribute(property)) {
        element.setAttribute(
          property,
          replaceCssUrlsForProjection(element.getAttribute(property) || ''),
        );
      }
    }
    for (const attribute of ['src', 'srcset', 'poster', 'data']) {
      if (element.hasAttribute(attribute)) element.setAttribute(attribute, 'data:,');
    }
    for (const attribute of ['href', 'xlink:href']) {
      if (element.hasAttribute(attribute)) {
        element.setAttribute(attribute, '#__pptx_preflight_resource__');
      }
    }
    for (const attribute of [...element.attributes]) {
      if (attribute.name.toLowerCase().startsWith('on')) element.removeAttribute(attribute.name);
    }
  }
  projectionRoot.appendChild(body);

  const controlSvg = ownerDocument.createElementNS('http://www.w3.org/2000/svg', 'svg');
  const control = ownerDocument.createElementNS('http://www.w3.org/2000/svg', 'rect');
  control.id = 'pptx-cascade-control';
  control.setAttribute('class', 'pptx-cascade-control');
  controlSvg.appendChild(control);
  projectionRoot.appendChild(controlSvg);
  ownerDocument.body.appendChild(host);

  return {
    host,
    indexedElements,
    projectedElement(element) {
      const index = element.getAttribute('data-pptx-preflight-index');
      return body.querySelector(`[data-pptx-preflight-index="${index}"]`);
    },
    supportsCascade() {
      const computed = globalThis.getComputedStyle(control);
      return cssValueHasUrlToken(computed.fill) && cssValueHasUrlToken(computed.stroke);
    },
    cleanup() {
      host.remove();
      indexedElements.forEach((element) => {
        element.removeAttribute('data-pptx-preflight-index');
      });
    },
  };
}

function auditMountedComputedStyles(parsed, slideNumber) {
  let projection;
  try {
    projection = makeMountedStyleProjection(parsed);
    if (!projection || !projection.supportsCascade()) return false;
    for (const element of projection.indexedElements) {
      const projected = projection.projectedElement(element);
      if (!projected) continue;
      const computed = globalThis.getComputedStyle(projected);
      if (element.closest?.('svg')
        && (cssValueHasUrlToken(computed.fill) || cssValueHasUrlToken(computed.stroke))) {
        fail(
          slideNumber,
          element,
          'svg_paint_server_unsupported',
          'SVG paint-server fills and strokes cannot be represented as editable PowerPoint paint.',
        );
      }
      if (textShadowIsVisible(computed.textShadow, new Map())) {
        fail(
          slideNumber,
          element,
          'text_shadow_unsupported',
          'CSS text-shadow cannot be represented as editable PowerPoint text.',
        );
      }
    }
    return true;
  } catch (error) {
    if (error instanceof EditableExportError) throw error;
    return false;
  } finally {
    projection?.cleanup();
  }
}

function isActiveScript(script) {
  const type = String(script.getAttribute('type') || '').trim().toLowerCase();
  return !type
    || type === 'module'
    || type.includes('javascript')
    || type.includes('ecmascript');
}

export function auditRawSlideForEditableExport(html, slideNumber) {
  const parsed = new DOMParser().parseFromString(String(html || ''), 'text/html');
  const unsupported = parsed.querySelector(VISIBLE_UNSUPPORTED_SELECTOR);
  if (unsupported) {
    fail(
      slideNumber,
      unsupported,
      'unsupported_visible_html',
      `Visible <${unsupported.localName}> content has no editable PowerPoint rewrite.`,
    );
  }
  const activeScript = [...parsed.querySelectorAll('script')].find(isActiveScript);
  if (activeScript) {
    fail(
      slideNumber,
      activeScript,
      'active_content_unsupported',
      'Active script content is not allowed in editable PowerPoint export.',
    );
  }
  if (!auditMountedComputedStyles(parsed, slideNumber)) {
    auditAuthoredStyles(parsed, slideNumber);
  }
}
