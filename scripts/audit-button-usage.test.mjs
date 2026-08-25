import assert from 'node:assert/strict';
import test from 'node:test';

import { analyzeButtonSource } from './audit-button-usage.mjs';

test('classifies independent, icon, and compound native buttons', () => {
  const inventory = analyzeButtonSource('Example.tsx', `
    export function Example() {
      return <>
        <button onClick={() => {}}>Save</button>
        <button aria-label="Close"><svg /></button>
        <button role="menuitem">Open</button>
        <button aria-haspopup="menu">More</button>
        <button className="feature__menu-item">Rename</button>
        <button data-bf-part="marketCard">Theme</button>
        <button aria-current="page">Browse</button>
      </>;
    }
  `);

  assert.deepEqual(
    inventory.buttons.map(button => button.kind),
    ['action', 'icon', 'compound', 'compound', 'compound', 'compound', 'compound'],
  );
});

test('finds local Button wrappers independently of imports', () => {
  const inventory = analyzeButtonSource('Example.tsx', `
    export const SaveButton = () => <button>Save</button>;
    function InlineAction() { return <button>Open</button>; }
  `);

  assert.deepEqual(inventory.localButtonDefinitions, [
    { file: 'Example.tsx', line: 2, name: 'SaveButton' },
  ]);
});
