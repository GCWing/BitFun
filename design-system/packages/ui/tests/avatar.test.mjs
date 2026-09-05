import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import test from "node:test";
import { Avatar, AvatarGroup } from "../dist/index.js";

test("Avatar renders text fallback with a stable anatomy contract", () => {
  const markup = renderToStaticMarkup(createElement(Avatar, {
    children: "BF",
    shape: "square",
    size: "sm",
  }));

  assert.match(markup, /data-openbitfun-component="avatar"/);
  assert.match(markup, /data-openbitfun-shape="square"/);
  assert.match(markup, /data-size="sm"/);
  assert.match(markup, /data-openbitfun-part="text"/);
});

test("AvatarGroup reports overflow", () => {
  const markup = renderToStaticMarkup(createElement(
    AvatarGroup,
    { maxCount: 1 },
    createElement(Avatar, null, "A"),
    createElement(Avatar, null, "B"),
  ));

  assert.match(markup, /data-openbitfun-component="avatar-group"/);
  assert.match(markup, /\+1/);
});
