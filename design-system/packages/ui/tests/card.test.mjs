import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  Card,
  CardBody,
  CardFooter,
  CardHeader,
  CardMedia,
} from "../dist/index.js";

test("Card keeps media, header, body, and footer regions independent", () => {
  const markup = renderToStaticMarkup(
    createElement(Card, {
      appearance: "raised",
      clip: true,
      gap: "md",
      padding: "none",
      radius: "lg",
    },
    createElement(CardMedia, null, createElement("img", { alt: "Preview", src: "/preview.png" })),
    createElement(CardHeader, {
      actions: createElement("button", null, "More"),
      contentAlign: "center",
      description: "One item",
      leading: createElement("svg", { "data-icon": "folder" }),
      padding: "md",
      title: "Projects",
    }),
    createElement(CardBody, { align: "end", padding: "md" }, "Card content"),
    createElement(CardFooter, { align: "between", padding: "md" },
      createElement("span", null, "Status"),
      createElement("button", null, "Open"),
    )),
  );

  assert.match(markup, /data-bf-component="card"/);
  assert.match(markup, /data-appearance="raised"/);
  assert.match(markup, /data-clip="true"/);
  assert.match(markup, /data-gap="md"/);
  assert.match(markup, /data-radius="lg"/);
  assert.match(markup, /data-bf-part="media"/);
  assert.match(markup, /data-bf-part="header"/);
  assert.match(markup, /data-content-align="center"/);
  assert.match(markup, /data-bf-part="header-content"/);
  assert.match(markup, /data-bf-part="leading"/);
  assert.match(markup, /data-bf-part="title">Projects/);
  assert.match(markup, /data-bf-part="description">One item/);
  assert.match(markup, /data-bf-part="actions"/);
  assert.match(markup, /data-bf-part="body"/);
  assert.match(markup, /data-align="end"[^>]+data-bf-part="body"/);
  assert.match(markup, /data-bf-part="footer"/);
  assert.match(markup, /data-align="between"/);
});

test("Card defaults remain a non-interactive surface contract", () => {
  const markup = renderToStaticMarkup(createElement(Card, null, "Content"));

  assert.match(markup, /data-appearance="subtle"/);
  assert.match(markup, /data-padding="none"/);
  assert.match(markup, /data-gap="none"/);
  assert.match(markup, /data-radius="md"/);
  assert.match(markup, /data-clip="false"/);
  assert.doesNotMatch(markup, /role="button"/);
  assert.doesNotMatch(markup, /tabindex=/);
});

test("Card styles use public surface, spacing, radius, and elevation tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-color-surface-raised/);
  assert.match(styles, /--bf-color-surface-tertiary/);
  assert.match(styles, /--bf-color-action-neutral-surface/);
  assert.match(styles, /--bf-shadow-overlay/);
  assert.match(styles, /--bf-layout-card-padding-sm/);
  assert.match(styles, /--bf-layout-card-radius-lg/);
  assert.match(styles, /--bf-layout-card-media-min-block-size/);
  assert.match(styles, /object-fit:cover/);
});
