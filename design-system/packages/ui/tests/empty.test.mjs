import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import test from "node:test";
import { Empty } from "../dist/index.js";

test("Empty exposes independent media, copy, and action regions", () => {
  const markup = renderToStaticMarkup(createElement(Empty, {
    actions: createElement("button", null, "Create"),
    description: "Create an item to get started.",
    imageSize: "sm",
    title: "No items",
  }));

  assert.match(markup, /data-bf-component="empty"/);
  assert.match(markup, /data-bf-part="media"/);
  assert.match(markup, /data-size="sm"/);
  assert.match(markup, /data-bf-part="title"/);
  assert.match(markup, /data-bf-part="description"/);
  assert.match(markup, /data-bf-part="actions"/);
});
