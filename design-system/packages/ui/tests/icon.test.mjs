import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { Icon, iconNames } from "../dist/index.js";

test("Icon exposes the complete named catalog without duplicate names", () => {
  assert.equal(iconNames.length, 54);
  assert.equal(new Set(iconNames).size, iconNames.length);
  assert.ok(iconNames.includes("search"));
  assert.ok(iconNames.includes("commit"));
  assert.ok(iconNames.includes("sidebar-right"));
});

test("Icon is decorative by default and owns its exact asset source", () => {
  const markup = renderToStaticMarkup(createElement(Icon, { name: "search" }));

  assert.match(markup, /data-bf-component="icon"/);
  assert.match(markup, /data-bf-name="search"/);
  assert.match(markup, /data-size="lg"/);
  assert.match(markup, /aria-hidden="true"/);
  assert.match(markup, /mask-image:url/);
  assert.doesNotMatch(markup, /<svg/);
});

test("Icon exposes semantic size, tone, and accessible label independently", () => {
  const markup = renderToStaticMarkup(createElement(Icon, {
    label: "Successful",
    name: "check-circle",
    size: "sm",
    tone: "success",
  }));

  assert.match(markup, /role="img"/);
  assert.match(markup, /aria-label="Successful"/);
  assert.doesNotMatch(markup, /aria-hidden/);
  assert.match(markup, /data-size="sm"/);
  assert.match(markup, /data-bf-tone="success"/);
});

test("Icon styles consume only public geometry and semantic color tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-control-icon-size2xs/);
  assert.match(styles, /--bf-control-icon-size-lg/);
  assert.match(styles, /--bf-color-content-primary/);
  assert.match(styles, /--bf-color-status-success-content/);
  assert.match(styles, /mask-size:contain/);
});
