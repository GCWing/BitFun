import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ConfirmDialog } from "../dist/index.js";

test("ConfirmDialog composes semantic content and actions on Modal", () => {
  const markup = renderToStaticMarkup(createElement(ConfirmDialog, {
    cancelText: "Cancel",
    confirmDanger: true,
    confirmText: "Delete",
    isOpen: true,
    message: "This cannot be undone.",
    onClose: () => undefined,
    onConfirm: () => undefined,
    portalled: false,
    preventScroll: false,
    preview: "/workspace/project",
    secondaryText: "Archive",
    showCloseButton: false,
    title: "Delete project?",
    type: "error",
  }));

  assert.match(markup, /role="alertdialog"/);
  assert.match(markup, /data-bf-component="confirm-dialog"/);
  assert.match(markup, /data-bf-part="message"[^>]*>This cannot be undone/);
  assert.match(markup, /data-bf-part="icon"[^>]+data-bf-status="danger"/);
  assert.match(markup, /data-bf-part="preview"/);
  assert.match(markup, /<pre>\/workspace\/project<\/pre>/);
  assert.match(markup, />Cancel<\/span><\/span><\/button>/);
  assert.match(markup, />Archive<\/span><\/span><\/button>/);
  assert.match(markup, />Delete<\/span><\/span><\/button>/);
  assert.match(markup, /data-bf-tone="danger"/);
  assert.match(markup, /data-bf-variant="primary"/);
});

test("ConfirmDialog can omit the icon and cancel action", () => {
  const markup = renderToStaticMarkup(createElement(ConfirmDialog, {
    confirmText: "OK",
    icon: false,
    isOpen: true,
    message: "Complete",
    onClose: () => undefined,
    onConfirm: () => undefined,
    portalled: false,
    preventScroll: false,
    showCancel: false,
    title: "Finished",
    type: "success",
  }));

  assert.doesNotMatch(markup, /data-bf-part="icon"/);
  assert.doesNotMatch(markup, />Cancel<\/span>/);
  assert.match(markup, />OK<\/span>/);
});

test("ConfirmDialog owns async pending and dismissal guards", async () => {
  const source = await readFile(
    new URL("../src/components/ConfirmDialog/ConfirmDialog.tsx", import.meta.url),
    "utf8",
  );

  assert.match(source, /pendingAction/);
  assert.match(source, /typeof result\.then === "function"/);
  assert.match(source, /loading=\{pendingAction === "confirm"\}/);
  assert.match(source, /loading=\{pendingAction === "secondary"\}/);
  assert.match(source, /closeOnEscape=\{!busy && closeOnEscape\}/);
  assert.match(source, /closeOnOverlayClick=\{!busy && closeOnOverlayClick\}/);
  assert.match(source, /onActionError\?\.\(error, actionName\)/);
});

test("ConfirmDialog styles use public status, layout, and typography tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-layout-confirm-dialog-content-gap/);
  assert.match(styles, /--bf-layout-confirm-dialog-icon-size/);
  assert.match(styles, /--bf-layout-confirm-dialog-preview-padding-inline/);
  assert.match(styles, /--bf-color-status-warning-content/);
  assert.match(styles, /--bf-color-status-danger-surface/);
  assert.match(styles, /--bf-font-family-mono/);
});
