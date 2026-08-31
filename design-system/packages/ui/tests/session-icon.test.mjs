import assert from "node:assert/strict";
import test from "node:test";
import { readFile } from "node:fs/promises";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { SessionIcon } from "../dist/index.js";


test("SessionIcon preserves the shared catalog geometry and opacity", async () => {
  const source = await readFile(new URL("../src/components/Icon/assets/session.svg", import.meta.url), "utf8");
  const expectedPath = source.match(/\bd="([^"]+)"/)?.[1];
  assert.ok(expectedPath);
  const markup = renderToStaticMarkup(createElement(SessionIcon));
  const renderedPath = markup.match(/<path d="([^"]+)"/)?.[1];

  assert.match(markup, /viewBox="0 0 24 24"/);
  assert.equal(renderedPath, expectedPath);
  assert.match(markup, /fill="currentColor"/);
  assert.doesNotMatch(markup, /black/i);
  assert.match(markup, /fill-opacity="0.8"/);
});

test("SessionIcon accepts size and standard SVG properties", () => {
  const markup = renderToStaticMarkup(createElement(SessionIcon, {
    "aria-label": "Session",
    className: "session-icon",
    "data-owner": "design-system",
    size: 18,
    width: 20,
  }));

  assert.match(markup, /width="20"/);
  assert.match(markup, /height="18"/);
  assert.match(markup, /class="session-icon"/);
  assert.match(markup, /aria-label="Session"/);
  assert.match(markup, /data-owner="design-system"/);
  assert.doesNotMatch(markup, /size=/);
});
