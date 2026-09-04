import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  Menu,
  MenuItem,
  MenuSection,
  MenuSeparator,
} from "../dist/index.js";

test("Menu composes grouped items, heading actions, and separators with native roles", () => {
  const markup = renderToStaticMarkup(
    createElement(
      Menu,
      { "aria-label": "Sessions", scrollbarVisibility: "always" },
      createElement(
        MenuSection,
        {
          actions: [{ icon: createElement("svg"), id: "add", label: "Add session" }],
          title: "Sessions",
        },
        createElement(MenuItem, null, "First session"),
        createElement(MenuItem, { checked: true, role: "menuitemcheckbox" }, "Pinned"),
      ),
      createElement(MenuSeparator),
      createElement(MenuSection, { "aria-label": "More" },
        createElement(MenuItem, { disabled: true }, "Disabled session"),
      ),
    ),
  );

  assert.match(markup, /data-openbitfun-component="menu"/);
  assert.match(markup, /role="menu"/);
  assert.match(markup, /data-openbitfun-scrollbar-visibility="always"/);
  assert.match(markup, /aria-labelledby="[^"]+"[^>]+role="group"/);
  assert.match(markup, /data-openbitfun-part="heading-actions"/);
  assert.match(markup, /aria-label="Add session"/);
  assert.equal((markup.match(/role="menuitem"/g) ?? []).length, 3);
  assert.match(markup, /aria-checked="true"[^>]+role="menuitemcheckbox"/);
  assert.match(markup, /role="separator"/);
});

test("Menu owns roving focus and standard single-level navigation keys", async () => {
  const source = await readFile(
    new URL("../src/components/Menu/Menu.tsx", import.meta.url),
    "utf8",
  );

  assert.match(source, /querySelectorAll<HTMLButtonElement>\("\[data-openbitfun-menu-item\]"\)/);
  assert.match(source, /case "ArrowDown"/);
  assert.match(source, /case "ArrowUp"/);
  assert.match(source, /case "Home"/);
  assert.match(source, /case "End"/);
  assert.match(source, /label\.startsWith\(query\)/);
  assert.match(source, /autoFocusFirstItem/);
});

test("Menu keeps roving focus scoped to items owned by the current menu", async () => {
  const source = await readFile(
    new URL("../src/components/Menu/Menu.tsx", import.meta.url),
    "utf8",
  );

  assert.match(source, /item\.closest\('\[role="menu"\]'\) === root/);
});

test("Menu styling uses only public surface, geometry, action, and scrollbar tokens", async () => {
  const styles = await readFile(
    new URL("../src/components/Menu/Menu.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /--openbitfun-overlay-menu-inline-size/);
  assert.match(styles, /--openbitfun-overlay-menu-item-height/);
  assert.match(styles, /\.list\s*\{[^}]*gap: calc\(var\(--openbitfun-space-1\) \/ 2\)/);
  assert.match(styles, /\.items\s*\{[^}]*gap: calc\(var\(--openbitfun-space-1\) \/ 2\)/);
  assert.match(styles, /\.separator\s*\{[^}]*margin-block: calc\(/);
  assert.match(styles, /--openbitfun-color-surface-panel/);
  assert.match(styles, /--openbitfun-shadow-menu/);
  assert.match(styles, /--openbitfun-overlay-menu-scrollbar-gap/);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});

test("Menu reserves space inside its scroll viewport for focus rings on all edges", async () => {
  const styles = await readFile(new URL("../src/components/Menu/Menu.module.css", import.meta.url), "utf8");
  const scrollStyles = await readFile(new URL("../src/components/ScrollArea/ScrollArea.module.css", import.meta.url), "utf8");
  const itemStyles = await readFile(new URL("../src/components/ActionItem/ActionItem.module.css", import.meta.url), "utf8");
  assert.match(styles, /\.list\s*\{[^}]*padding:\s*var\(--openbitfun-focus-width\)/);
  assert.match(itemStyles, /\.root:has\(\.trigger:focus-visible\)\s*\{[^}]*box-shadow:\s*0 0 0 var\(--openbitfun-focus-width\)/);
  // Keep clipping and scrolling; the content, not the viewport, owns the gutter.
  assert.match(scrollStyles, /data-openbitfun-orientation="vertical"\]\s*\{[^}]*overflow-x:\s*hidden;[^}]*overflow-y:\s*auto/);
  assert.doesNotMatch(styles, /overflow[^:]*:\s*visible/);
});

test("Menu keeps equal item insets while its scrollbar stays on the surface edge", async () => {
  const styles = await readFile(new URL("../src/components/Menu/Menu.module.css", import.meta.url), "utf8");

  assert.match(styles, /\.root\s*\{[^}]*padding-inline:\s*var\(--openbitfun-overlay-menu-surface-padding\) 0/);
  assert.match(styles, /\.viewport\s*\{[^}]*padding-inline-end:\s*var\(--openbitfun-overlay-menu-scrollbar-gap\);[^}]*scrollbar-gutter:\s*auto/);
  assert.match(
    styles,
    /\.list\s*\{[^}]*padding-inline-end:\s*calc\(\s*var\(--openbitfun-overlay-menu-surface-padding\)\s*\+ var\(--openbitfun-focus-width\)\s*- var\(--openbitfun-overlay-menu-scrollbar-gap\)\s*\)/,
  );
  assert.doesNotMatch(styles, /scrollbar-gutter:\s*stable/);
});
