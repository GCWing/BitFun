import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

import * as adapters from './index';

describe('OpenBitFun Canvas SDK adapters', () => {
  it('exports the initial Canvas runtime adapter set', () => {
    expect(Object.keys(adapters).sort()).toEqual([
      'Alert',
      'BarChart',
      'Box',
      'Button',
      'Callout',
      'Card',
      'CardBody',
      'CardHeader',
      'Checkbox',
      'Code',
      'CollapsibleSection',
      'DependencyGraph',
      'DiffStats',
      'DiffView',
      'Divider',
      'Empty',
      'FileTree',
      'FlowDiagram',
      'Grid',
      'H1',
      'H2',
      'H3',
      'IconButton',
      'Input',
      'KeyValueList',
      'LineChart',
      'Link',
      'PieChart',
      'Pill',
      'ProgressBar',
      'Row',
      'Select',
      'Spacer',
      'Stack',
      'Stat',
      'Swatch',
      'Table',
      'Tabs',
      'Text',
      'TextArea',
      'TextInput',
      'Timeline',
      'TodoList',
      'TodoListCard',
      'Toggle',
      'UsageBar',
      'canvasPaletteDark',
      'canvasPaletteLight',
      'canvasTokens',
      'canvasTokensLight',
      'categoryPaletteDark',
      'categoryPaletteLight',
      'computeDAGLayout',
      'mergeStyle',
      'normalizeDiffLines',
      'usageColorSequence',
      'useCallback',
      'useCanvasAction',
      'useCanvasState',
      'useEffect',
      'useHostAppearance',
      'useMemo',
      'useRef',
      'useState',
    ]);
  });

  it('keeps sandbox runtime adapters free of host capability imports', () => {
    const source = readFileSync(
      fileURLToPath(new URL('./adapters.tsx', import.meta.url)),
      'utf8',
    );

    expect(source).not.toMatch(/@\/infrastructure\/api|@\/flow_chat|@tauri-apps|useI18n|zustand/);
  });
});
