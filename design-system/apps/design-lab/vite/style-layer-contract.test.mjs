import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const stylesSource = new URL("../src/styles.css", import.meta.url);

function findLayerRange(source, name) {
  const marker = `@layer ${name}`;
  const markerIndex = source.indexOf(marker);

  assert.notEqual(markerIndex, -1, `Missing ${marker}`);

  const openingBraceIndex = source.indexOf("{", markerIndex + marker.length);

  assert.notEqual(openingBraceIndex, -1, `Missing opening brace for ${marker}`);

  let depth = 0;

  for (let index = openingBraceIndex; index < source.length; index += 1) {
    if (source[index] === "{") {
      depth += 1;
    } else if (source[index] === "}") {
      depth -= 1;

      if (depth === 0) {
        return { start: markerIndex, end: index + 1 };
      }
    }
  }

  assert.fail(`Missing closing brace for ${marker}`);
}

test("Design Lab form resets stay below component styles in openbitfun.reset", async () => {
  const source = await readFile(stylesSource, "utf8");
  const resetLayer = findLayerRange(source, "openbitfun.reset");
  const resetContracts = [
    /button,\s*input,\s*select\s*\{\s*font:\s*inherit;\s*\}/g,
    /button,\s*a,\s*select\s*\{\s*-webkit-tap-highlight-color:\s*transparent;\s*\}/g,
    /button\s*\{\s*color:\s*inherit;\s*\}/g,
  ];

  for (const contract of resetContracts) {
    const matches = [...source.matchAll(contract)];

    assert.equal(matches.length, 1);
    assert.ok(matches[0].index > resetLayer.start);
    assert.ok(matches[0].index < resetLayer.end);
  }
});
