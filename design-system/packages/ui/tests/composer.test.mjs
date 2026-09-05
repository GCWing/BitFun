import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  Composer,
  ComposerContextBar,
  ComposerDivider,
  ComposerToolbar,
} from "../dist/index.js";

test("Composer exposes independent context, editor, and toolbar regions", () => {
  const context = createElement(ComposerContextBar, {
    leading: createElement("button", null, "Computer"),
    trailing: createElement("button", null, "Mode"),
  });
  const toolbar = createElement(ComposerToolbar, {
    leading: createElement("button", null, "Add"),
    trailing: createElement("button", null, "Send"),
  });
  const markup = renderToStaticMarkup(
    createElement(
      Composer,
      {
        "aria-label": "Message composer",
        contextBar: context,
        toolbar,
      },
      createElement("textarea", { placeholder: "How can I help?" }),
    ),
  );

  assert.match(markup, /data-openbitfun-component="composer"/);
  assert.match(markup, /<fieldset/);
  assert.match(markup, /data-has-context="true"/);
  assert.match(markup, /data-openbitfun-part="context"/);
  assert.match(markup, /data-openbitfun-part="context-bar"/);
  assert.match(markup, /data-openbitfun-part="surface"/);
  assert.match(markup, /data-openbitfun-part="editor"/);
  assert.match(markup, /data-openbitfun-part="toolbar"/);
  assert.match(markup, /data-openbitfun-part="toolbar-bar"/);
  assert.match(markup, /<textarea placeholder="How can I help\?"><\/textarea>/);
});

test("Composer normalizes invalid and disabled state semantics", () => {
  const markup = renderToStaticMarkup(
    createElement(
      Composer,
      { disabled: true, invalid: true },
      createElement("textarea"),
    ),
  );

  assert.match(markup, /aria-disabled="true"/);
  assert.match(markup, /aria-invalid="true"/);
  assert.match(markup, /data-disabled="true"/);
  assert.match(markup, /data-invalid="true"/);
  assert.match(markup, /disabled=""/);
});

test("ComposerDivider is decorative and follows the composer separator contract", () => {
  const markup = renderToStaticMarkup(createElement(ComposerDivider));

  assert.match(markup, /aria-hidden="true"/);
  assert.match(markup, /data-openbitfun-part="divider"/);
});

test("Composer styling uses only shared public token layers", async () => {
  const styles = await readFile(
    new URL("../src/components/Composer/Composer.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /--openbitfun-control-composer-context-offset/);
  assert.match(styles, /--openbitfun-control-composer-min-block-size/);
  assert.match(styles, /--openbitfun-color-surface-raised/);
  assert.match(styles, /--openbitfun-color-field-border-focus/);
  assert.match(styles, /--openbitfun-shadow-composer/);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}/i);
});
