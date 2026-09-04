import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import ts from 'typescript';
import { describe, expect, it } from 'vitest';

const sourceRoot = path.resolve(__dirname, '../..');
const markerOwners = new Set([
  'infrastructure/config/components/common/ConfigPageLayout.tsx',
  'infrastructure/config/components/form-controls/ConfigInput.tsx',
  'infrastructure/config/components/form-controls/ConfigTextarea.tsx',
]);

function filesIn(directory: string, pattern: RegExp): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    if (entry.name === 'generated') return [];
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) return filesIn(file, pattern);
    return pattern.test(file) && !/\.(test|spec|appearance)\./.test(file) ? [file] : [];
  });
}

function relativePath(file: string): string {
  return path.relative(sourceRoot, file).replaceAll('\\', '/');
}

function staticFragments(expression: ts.Expression): string[] {
  if (ts.isStringLiteralLike(expression)) return [expression.text];
  if (ts.isTemplateExpression(expression)) {
    return [expression.head.text, ...expression.templateSpans.map(span => span.literal.text)];
  }
  if (ts.isParenthesizedExpression(expression)) return staticFragments(expression.expression);
  if (ts.isConditionalExpression(expression)) {
    return [
      ...staticFragments(expression.whenTrue),
      ...staticFragments(expression.whenFalse),
    ];
  }
  if (ts.isBinaryExpression(expression) && expression.operatorToken.kind === ts.SyntaxKind.PlusToken) {
    return [...staticFragments(expression.left), ...staticFragments(expression.right)];
  }
  return [];
}

function labelFragments(attribute: ts.JsxAttribute): string[] {
  const initializer = attribute.initializer;
  if (!initializer) return [];
  if (ts.isStringLiteral(initializer)) return [initializer.text];
  if (ts.isJsxExpression(initializer) && initializer.expression) {
    return staticFragments(initializer.expression);
  }
  return [];
}

function locationOf(file: string, sourceFile: ts.SourceFile, node: ts.Node): string {
  const { line } = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
  return `${relativePath(file)}:${line + 1}`;
}

describe('required field design-system integration', () => {
  it('keeps required markers out of label copy and inside approved field owners', () => {
    const embeddedLabelMarkers: string[] = [];
    const unownedMarkerAnatomy: string[] = [];

    for (const file of filesIn(sourceRoot, /\.tsx$/)) {
      const sourceFile = ts.createSourceFile(
        file,
        readFileSync(file, 'utf8'),
        ts.ScriptTarget.Latest,
        true,
        ts.ScriptKind.TSX,
      );
      const owner = relativePath(file);

      const visit = (node: ts.Node): void => {
        if (
          ts.isJsxAttribute(node)
          && ts.isIdentifier(node.name)
          && node.name.text === 'label'
          && labelFragments(node).some(fragment => /(?:^|\s)\*(?:\s|$)/.test(fragment))
        ) {
          embeddedLabelMarkers.push(locationOf(file, sourceFile, node));
        }

        const isMarkerText = ts.isJsxText(node) && node.getText(sourceFile).trim() === '*';
        const isMarkerExpression = ts.isJsxExpression(node)
          && node.expression !== undefined
          && ts.isStringLiteralLike(node.expression)
          && node.expression.text.trim() === '*';
        if ((isMarkerText || isMarkerExpression) && !markerOwners.has(owner)) {
          unownedMarkerAnatomy.push(locationOf(file, sourceFile, node));
        }

        ts.forEachChild(node, visit);
      };
      visit(sourceFile);
    }

    expect(embeddedLabelMarkers).toEqual([]);
    expect(unownedMarkerAnatomy).toEqual([]);
  });

  it('forbids product styles from synthesizing required markers with pseudo-elements', () => {
    const pseudoMarkers = filesIn(sourceRoot, /\.(?:css|scss)$/).flatMap(file => {
      const source = readFileSync(file, 'utf8');
      return /content\s*:\s*["']\s*\*\s*["']/.test(source) ? [relativePath(file)] : [];
    });

    expect(pseudoMarkers).toEqual([]);
  });

  it('keeps every product-owned required marker on the shared semantic color', () => {
    for (const file of [
      'infrastructure/config/components/common/ConfigPageLayout.scss',
      'infrastructure/config/components/ConfigForm.scss',
    ]) {
      expect(readFileSync(path.join(sourceRoot, file), 'utf8')).toContain(
        'var(--openbitfun-color-content-required-indicator)',
      );
    }
  });

  it('registers the shared config required anatomy with the Appearance contract', () => {
    expect(
      readFileSync(path.join(sourceRoot, 'infrastructure/config/appearance.ts'), 'utf8'),
    ).toMatch(/\{ id: ['"]required['"] \}/);
  });
});
