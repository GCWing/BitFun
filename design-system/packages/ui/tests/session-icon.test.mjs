import assert from "node:assert/strict";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { SessionIcon } from "../dist/index.js";

const expectedPath = "M12 21.5069C18.5625 21.5069 23.3574 17.503 23.3574 11.9951C23.3574 6.46777 18.5528 2.49316 12 2.49316C5.4375 2.49316 0.642578 6.46777 0.642578 11.9951C0.642578 13.8213 1.17969 15.5205 2.11719 16.8975C2.57617 17.5811 2.74219 18.0596 2.74219 18.46C2.74219 18.9776 2.58594 19.3975 2.13672 19.7881C1.36523 20.4424 1.76563 21.5069 2.78125 21.5069C4.00196 21.5069 5.35938 21.087 6.36524 20.3936C8.01563 21.1162 9.93946 21.5069 12 21.5069ZM12 19.9346C10.1348 19.9346 8.46485 19.583 7.04883 18.9483C6.42383 18.6748 5.97461 18.753 5.37891 19.1045C4.95899 19.3682 4.4707 19.5928 3.97266 19.7002C4.17774 19.3584 4.31446 18.9678 4.31446 18.46C4.31446 17.7373 4.05078 16.9463 3.42578 16.0088C2.64453 14.876 2.21485 13.4991 2.21485 11.9951C2.21485 7.41504 6.25781 4.06543 12 4.06543C17.7422 4.06543 21.7852 7.41504 21.7852 11.9951C21.7852 16.5752 17.7422 19.9346 12 19.9346Z";

test("SessionIcon preserves the supplied geometry with semantic color", () => {
  const markup = renderToStaticMarkup(createElement(SessionIcon));
  const renderedPath = markup.match(/<path d="([^"]+)"/)?.[1];

  assert.match(markup, /viewBox="0 0 24 24"/);
  assert.equal(renderedPath, expectedPath);
  assert.match(markup, /fill="currentColor"/);
  assert.doesNotMatch(markup, /black/i);
  assert.doesNotMatch(markup, /fill-opacity/i);
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
