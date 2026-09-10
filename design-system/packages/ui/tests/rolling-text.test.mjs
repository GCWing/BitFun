import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { RollingText, TabGroup } from "../dist/index.js";

test("RollingText renders static, complete text on the server without an outgoing copy", () => {
  const markup = renderToStaticMarkup(createElement(RollingText, { transitionKey: "a" }, "Full session title"));
  assert.match(markup, /data-openbitfun-component="rolling-text"/);
  assert.match(markup, /data-transitioning="false"/);
  assert.match(markup, />Full session title</);
  assert.doesNotMatch(markup, /data-rolling-text-part="outgoing"|aria-live/);
  assert.equal((markup.match(/data-overflow-behavior="marquee"/g) ?? []).length, 1);
});

test("TabGroup opts plain labels into replacement without nesting overflow containers", () => {
  const markup = renderToStaticMarkup(createElement(TabGroup, {
    items: [
      { value: "slot", label: "Session", labelTransitionKey: "session-a" },
      { value: "settings", label: "Settings" },
      { value: "rich", label: createElement("strong", null, "Rich label"), labelTransitionKey: "rich" },
    ],
  }));
  assert.equal((markup.match(/data-openbitfun-component="rolling-text"/g) ?? []).length, 1);
  assert.equal((markup.match(/data-overflow-behavior="marquee"/g) ?? []).length, 3);
  assert.equal((markup.match(/role="tab"/g) ?? []).length, 3);
  assert.match(markup, /<strong>Rich label<\/strong>/);
});

test("replacement uses shared spatial-motion tokens and reduced-motion styling", async () => {
  const styles = await readFile(new URL("../src/components/RollingText/RollingText.module.css", import.meta.url), "utf8");
  assert.match(styles, /--openbitfun-motion-duration-content-swap/);
  assert.match(styles, /--openbitfun-motion-easing-smooth/);
  assert.match(styles, /prefers-reduced-motion: reduce/);
});
