import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { LoadingState, Spinner } from "../dist/index.js";

test("Spinner exposes canonical matrix and bar presentations", () => {
  const matrix = renderToStaticMarkup(createElement(Spinner, { size: "sm" }));
  const bars = renderToStaticMarkup(createElement(Spinner, {
    "aria-label": "Loading",
    variant: "bars",
  }));

  assert.match(matrix, /data-bf-component="spinner"/);
  assert.match(matrix, /data-variant="matrix"/);
  assert.equal((matrix.match(/aria-hidden="true"/g) ?? []).length, 10);
  assert.match(bars, /role="status"/);
  assert.match(bars, /aria-label="Loading"/);
  assert.equal((bars.match(/class=/g) ?? []).length, 4);
});

test("LoadingState composes Spinner with optional support copy", () => {
  const markup = renderToStaticMarkup(createElement(LoadingState, null, "Loading project"));
  assert.match(markup, /data-bf-component="loading-state"/);
  assert.match(markup, /data-bf-component="spinner"/);
  assert.match(markup, /data-bf-part="label">Loading project/);
});

test("Spinner styling uses public system, motion, and semantic tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");
  assert.match(styles, /--bf-layout-spinner-matrix-cell-md/);
  assert.match(styles, /--bf-layout-spinner-matrix-gap-md/);
  assert.match(styles, /--bf-motion-duration-loop/);
  assert.match(styles, /--bf-color-content-muted/);
});
