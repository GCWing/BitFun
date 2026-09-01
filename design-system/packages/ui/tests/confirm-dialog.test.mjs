import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const sourceUrl = new URL("../src/components/ConfirmDialog/ConfirmDialog.tsx", import.meta.url);

test("ConfirmDialog composes semantic content and actions on Dialog", async () => {
  const source = await readFile(sourceUrl, "utf8");

  assert.match(source, /<Dialog/);
  assert.match(source, /role="alertdialog"/);
  assert.match(source, /<DialogHeader>/);
  assert.match(source, /<DialogTitle>\{title\}<\/DialogTitle>/);
  assert.match(source, /<DialogBody>/);
  assert.match(source, /<DialogFooter>/);
  assert.match(source, /data-bf-component="confirm-dialog" data-bf-part="content"/);
  assert.match(source, /data-bf-part="messageRow"/);
  assert.match(source, /data-bf-part="preview"/);
  assert.match(source, /data-bf-status=\{type === "error" \? "danger" : type\}/);
  assert.match(source, /tone=\{confirmDanger \|\| type === "error" \? "danger" : "neutral"\}/);
});

test("ConfirmDialog uses the canonical controlled API without geometry or portal escape hatches", async () => {
  const source = await readFile(sourceUrl, "utf8");

  assert.match(source, /open: boolean/);
  assert.match(source, /onOpenChange: \(open: false, reason: ConfirmDialogCloseReason\) => void/);
  assert.match(source, /closeOnPointerOutside/);
  assert.doesNotMatch(source, /\bisOpen\b|\bonClose\b|previewMaxHeight|dialogClassName|overlayClassName|portalContainer|portalled|preventScroll/);
});

test("ConfirmDialog owns async pending and dismissal guards", async () => {
  const source = await readFile(sourceUrl, "utf8");

  assert.match(source, /pendingAction/);
  assert.match(source, /typeof result\.then === "function"/);
  assert.match(source, /loading=\{pendingAction === "confirm"\}/);
  assert.match(source, /loading=\{pendingAction === "secondary"\}/);
  assert.match(source, /closeOnEscape=\{!busy && closeOnEscape\}/);
  assert.match(source, /closeOnPointerOutside=\{!busy && closeOnPointerOutside\}/);
  assert.match(source, /onActionError\?\.\(error, actionName\)/);
  assert.match(source, /onOpenChange\(false, reason\)/);
  assert.match(source, /designSystem\.messages\.confirmCancel/);
  assert.match(source, /designSystem\.messages\.confirmAction/);
});

test("ConfirmDialog styles use public status, layout, and typography tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-layout-confirm-dialog-content-gap/);
  assert.match(styles, /--bf-layout-confirm-dialog-icon-size/);
  assert.match(styles, /--bf-layout-confirm-dialog-preview-max-block-size/);
  assert.match(styles, /--bf-layout-confirm-dialog-preview-padding-inline/);
  assert.match(styles, /--bf-color-status-warning-content/);
  assert.match(styles, /--bf-color-status-danger-surface/);
  assert.match(styles, /--bf-font-family-mono/);
});
