import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Modal owns a portaled, stacked focus and dismissal contract", async () => {
  const source = await readFile(
    new URL("../src/components/Modal/Modal.tsx", import.meta.url),
    "utf8",
  );

  assert.match(source, /createPortal\(/);
  assert.match(source, /portalled && resolvedPortalContainer/);
  assert.match(source, /const modalStacks = new WeakMap<Document, symbol\[\]>/);
  assert.match(source, /isTopModal\(ownerDocument, identityRef\.current\)/);
  assert.match(source, /event\.key !== "Escape"/);
  assert.match(source, /isImeOwnedKeyboardEvent\(event\)/);
  assert.match(source, /event\.key !== "Tab"/);
  assert.match(source, /previousFocusRef\.current\?\.isConnected/);
  assert.match(source, /initialFocusRef\?\.current/);
  assert.match(source, /if \(trapFocus\) ownerDocument\.addEventListener\("keydown", handleFocusTrap\)/);
  assert.match(source, /pointerStartedOnOverlayRef\.current/);
  assert.match(source, /requestClose\("overlay"\)/);
  assert.match(source, /requestClose\("close-button"\)/);
  assert.match(source, /onClose\("escape-key"\)/);
  assert.match(source, /data-bf-part="overlay"/);
  assert.match(source, /data-bf-part="dialog"/);
  assert.match(source, /data-bf-part="content"/);
  assert.match(source, /aria-describedby=\{resolvedDescribedBy\}/);
  assert.match(source, /data-bf-part="description"/);
  assert.match(source, /data-bf-part="headerActions"/);
  assert.match(source, /data-bf-part="footer"/);
  assert.match(source, /data-bf-has-footer=/);
  assert.match(source, /data-bf-show-scrollbar=/);
});

test("Modal geometry and surface styling use public design tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-overlay-modal-viewport-gutter/);
  assert.match(styles, /--bf-overlay-modal-backdrop-blur/);
  assert.match(styles, /--bf-overlay-modal-surface-radius/);
  assert.match(styles, /--bf-overlay-modal-header-padding-inline/);
  assert.match(styles, /--bf-type-heading-dialog-font-size/);
  assert.match(styles, /--bf-type-heading-dialog-font-weight/);
  assert.match(styles, /--bf-scrollbar-width/);
  assert.match(styles, /--bf-scrollbar-radius/);
  assert.match(styles, /--bf-color-scrollbar-thumb/);
  assert.match(styles, /--bf-color-scrollbar-thumb-hover/);
  assert.match(styles, /--bf-overlay-modal-max-inline-size-wide/);
  assert.match(styles, /--bf-color-overlay-scrim/);
  assert.match(styles, /--bf-color-surface-raised/);
  assert.match(styles, /--bf-radius-4xl/);
  assert.match(styles, /--bf-shadow-overlay/);
  assert.match(styles, /--bf-overlay-modal-footer-content-inset/);
  assert.match(styles, /--bf-overlay-modal-footer-height/);
  assert.match(styles, /--bf-overlay-modal-footer-fade-extent/);
  assert.match(styles, /--bf-overlay-modal-footer-blur/);
  assert.match(styles, /backdrop-filter:\s*var\(--bf-overlay-modal-footer-blur\)/);
  assert.match(styles, /--bf-overlay-modal-footer-action-min-width/);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});
