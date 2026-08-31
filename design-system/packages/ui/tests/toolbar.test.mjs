import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  Toolbar,
  ToolbarBadge,
  ToolbarGroup,
  ToolbarSeparator,
} from "../dist/index.js";

test("Toolbar keeps leading, centered, and trailing regions independent", () => {
  const markup = renderToStaticMarkup(
    createElement(Toolbar, {
      center: createElement(ToolbarGroup, null,
        createElement(ToolbarBadge, null, "18"),
        createElement("strong", null, "Continue"),
      ),
      leading: createElement("button", null, "Activity"),
      trailing: createElement(ToolbarGroup, null,
        createElement(ToolbarSeparator),
        createElement("button", null, "Search"),
      ),
    }),
  );

  assert.match(markup, /data-bf-component="toolbar"/);
  assert.match(markup, /data-has-center="true"/);
  assert.match(markup, /data-bf-part="leading"/);
  assert.match(markup, /data-bf-part="center"/);
  assert.match(markup, /data-bf-part="trailing"/);
  assert.match(markup, /data-bf-part="badge"/);
  assert.match(markup, /data-bf-part="value">18<\/span>/);
  assert.match(markup, /data-bf-part="separator"/);
  assert.match(markup, /aria-hidden="true"/);
});

test("Toolbar exposes compact sizing and leading overflow without imposing product semantics", () => {
  const markup = renderToStaticMarkup(
    createElement(Toolbar, {
      "aria-label": "Workspace views",
      leading: createElement("div", null, "Tabs"),
      leadingOverflow: "scroll",
      role: "toolbar",
      size: "md",
      trailing: createElement("div", null, "Actions"),
    }),
  );

  assert.match(markup, /role="toolbar"/);
  assert.match(markup, /aria-label="Workspace views"/);
  assert.match(markup, /data-size="md"/);
  assert.match(markup, /data-overflow="scroll"/);
  assert.doesNotMatch(markup, /data-has-center="true"/);
});

test("Toolbar styles bind geometry and overflow treatment to public tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-layout-toolbar-sm-height/);
  assert.match(styles, /--bf-layout-toolbar-md-height/);
  assert.match(styles, /--bf-layout-toolbar-content-gap/);
  assert.match(styles, /--bf-layout-toolbar-badge-size/);
  assert.match(styles, /--bf-layout-toolbar-overflow-fade-extent/);
  assert.match(styles, /overflow-x:auto/);
  assert.match(styles, /mask-image:linear-gradient/);
});
