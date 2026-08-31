import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Dialog and Sheet compose the shared overlay kernel and compound anatomy", async () => {
  const source = await readFile(
    new URL("../src/components/Dialog/Dialog.tsx", import.meta.url),
    "utf8",
  );

  assert.match(source, /<Portal target=\{resolvedPortalHost\}>/);
  assert.match(source, /useDismissibleLayer/);
  assert.match(source, /useFocusScope/);
  assert.match(source, /useScrollLock/);
  assert.match(source, /usePresence/);
  assert.match(source, /kind="dialog"/);
  assert.match(source, /kind="sheet"/);
  for (const part of [
    "overlay",
    "surface",
    "header",
    "heading",
    "title",
    "description",
    "header-actions",
    "close",
    "body",
    "footer",
  ]) {
    assert.match(source, new RegExp(`data-bf-part=\\"${part}\\"`));
  }
  assert.match(source, /aria-describedby=\{ariaDescribedBy \?\? \(hasDescription \? descriptionId : undefined\)\}/);
  assert.match(source, /aria-labelledby=\{ariaLabelledBy \?\? \(!ariaLabel && hasTitle \? titleId : undefined\)\}/);
});

test("Dialog public API has canonical axes and no legacy escape hatches", async () => {
  const source = await readFile(
    new URL("../src/components/Dialog/Dialog.tsx", import.meta.url),
    "utf8",
  );

  assert.match(source, /export type DialogSize = "sm" \| "md" \| "lg" \| "xl" \| "2xl"/);
  assert.match(source, /export type SheetPlacement = "left" \| "right" \| "bottom"/);
  assert.match(source, /onOpenChange: \(open: false, reason: DialogCloseReason\) => void/);
  assert.doesNotMatch(source, /contentClassName|overlayClassName|portalContainer|portalled|resizable|draggable/);
});

test("Dialog geometry and typography use public design tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  for (const token of [
    "--bf-overlay-dialog-viewport-gutter",
    "--bf-overlay-dialog-backdrop-blur",
    "--bf-overlay-dialog-surface-radius",
    "--bf-overlay-dialog-header-padding-inline",
    "--bf-overlay-dialog-footer-padding-inline",
    "--bf-overlay-dialog-max-inline-size-xxlarge",
    "--bf-type-heading-dialog-font-size",
    "--bf-type-heading-dialog-font-weight",
    "--bf-color-overlay-scrim",
    "--bf-color-surface-raised",
    "--bf-shadow-overlay",
  ]) {
    assert.match(styles, new RegExp(token));
  }
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});
