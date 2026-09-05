import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import test from "node:test";
import { Alert } from "../dist/index.js";

test("Alert exposes semantic tone and public anatomy", () => {
  const markup = renderToStaticMarkup(createElement(Alert, {
    description: "Reconnect to continue.",
    message: "The remote host is offline.",
    showIcon: false,
    title: "Connection unavailable",
    tone: "warning",
  }));

  assert.match(markup, /data-openbitfun-component="alert"/);
  assert.match(markup, /data-openbitfun-tone="warning"/);
  assert.match(markup, /data-openbitfun-part="title"/);
  assert.match(markup, /data-openbitfun-part="message"/);
  assert.match(markup, /data-openbitfun-part="description"/);
});
