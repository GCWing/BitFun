import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Modal owns a portaled, stacked focus and dismissal contract", async () => {
  const source = await readFile(
    new URL("../src/components/Modal/Modal.tsx", import.meta.url),
    "utf8",
  );

  assert.match(source, /createPortal\(/);
  assert.match(source, /const modalStacks = new WeakMap<Document, symbol\[\]>/);
  assert.match(source, /isTopModal\(ownerDocument, identityRef\.current\)/);
  assert.match(source, /event\.key !== "Escape"/);
  assert.match(source, /event\.key !== "Tab"/);
  assert.match(source, /previousFocusRef\.current\?\.isConnected/);
  assert.match(source, /data-bf-part="overlay"/);
  assert.match(source, /data-bf-part="dialog"/);
  assert.match(source, /data-bf-part="content"/);
});

test("Modal geometry and surface styling use public design tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-overlay-modal-viewport-gutter/);
  assert.match(styles, /--bf-overlay-modal-max-inline-size-wide/);
  assert.match(styles, /--bf-color-overlay-scrim/);
  assert.match(styles, /--bf-color-surface-raised/);
  assert.match(styles, /--bf-radius-4xl/);
  assert.match(styles, /--bf-shadow-overlay/);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});
