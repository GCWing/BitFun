import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  ChatComposer,
  ChatComposerActionButton,
  ChatComposerContent,
  ChatComposerEndActions,
  ChatComposerQueue,
  ChatComposerQueueAttachmentBadge,
  ChatComposerQueueHeader,
  ChatComposerQueueItem,
  ChatComposerQueueItemActions,
  ChatComposerQueueItemContent,
  ChatComposerQueueList,
  ChatComposerQueueTitle,
  ChatComposerStartActions,
} from "../dist/flow-chat.js";

test("ChatComposerActionButton shares one stable action and icon geometry", () => {
  const markup = renderToStaticMarkup(
    createElement(
      "div",
      null,
      createElement(ChatComposerActionButton, {
        "aria-expanded": true,
        "aria-label": "Add context",
        icon: createElement("svg", { "data-icon": "plus" }),
        variant: "fill",
      }),
      createElement(ChatComposerActionButton, {
        "aria-label": "Send",
        icon: createElement("svg", { "data-icon": "arrow-up" }),
        variant: "primary",
      }),
    ),
  );

  assert.equal((markup.match(/data-bf-role="composer-action"/g) ?? []).length, 2);
  assert.equal((markup.match(/data-bf-shape="circle"/g) ?? []).length, 2);
  assert.match(markup, /data-bf-variant="fill"/);
  assert.match(markup, /data-bf-variant="primary"/);
});

test("ChatComposer publishes stable context, content, and action slots", () => {
  const markup = renderToStaticMarkup(
    createElement(ChatComposer, {
      contextBar: createElement("div", null, "This computer · BitFun"),
      endActions: createElement("button", { type: "button" }, "Send"),
      layout: "compact",
      startActions: createElement("button", { type: "button" }, "Add"),
      children: createElement("textarea", { "aria-label": "Message" }),
    }),
  );

  assert.match(markup, /data-bf-component="chat-composer"/);
  assert.match(markup, /data-has-context="true"/);
  assert.match(markup, /data-bf-part="contextBar"/);
  const surface = markup.match(/<div[^>]+data-bf-part="surface"[^>]*>/)?.[0];
  assert.ok(surface);
  assert.match(surface, /data-bf-layout="compact"/);
  assert.match(markup, /data-bf-part="startActions"/);
  assert.match(markup, /data-bf-part="content"/);
  assert.match(markup, /data-bf-part="endActions"/);
  assert.match(markup, /aria-label="Message"/);
});

test("ChatComposer exposes expanded, busy, and disabled state without owning editor behavior", () => {
  const markup = renderToStaticMarkup(
    createElement(ChatComposer, {
      busy: true,
      disabled: true,
      layout: "expanded",
      children: createElement("div", { contentEditable: true }),
    }),
  );

  assert.match(markup, /aria-busy="true"/);
  assert.match(markup, /aria-disabled="true"/);
  assert.match(markup, /data-bf-state="busy disabled"/);
  assert.match(markup, /data-bf-layout="expanded"/);
  assert.doesNotMatch(markup, /data-bf-part="contextBar"/);
  assert.match(markup, /contenteditable="true"/);
});

test("ChatComposer exposes a neutral pending-message queue with per-message attachment counts", () => {
  const queue = createElement(
    ChatComposerQueue,
    { "aria-label": "Wait for sending" },
    createElement(
      ChatComposerQueueHeader,
      null,
      createElement("svg", { "aria-hidden": true }),
      createElement(ChatComposerQueueTitle, { count: 13 }, "Wait for sending"),
    ),
    createElement(
      ChatComposerQueueList,
      null,
      createElement(
        ChatComposerQueueItem,
        { state: "default" },
        createElement(
          ChatComposerQueueItemContent,
          null,
          "Help me turn these photos into a painted scene.",
        ),
        createElement(ChatComposerQueueAttachmentBadge, {
          count: 3,
          label: "3 image attachments",
        }),
        createElement(
          ChatComposerQueueItemActions,
          null,
          createElement("button", { "aria-label": "Send now", type: "button" }),
        ),
      ),
    ),
  );
  const markup = renderToStaticMarkup(
    createElement(ChatComposer, {
      children: createElement("textarea", { "aria-label": "Message" }),
      layout: "compact",
      queue,
    }),
  );

  assert.match(markup, /data-bf-part="body"/);
  assert.match(markup, /data-bf-part="queue"/);
  assert.match(markup, /data-bf-component="chat-composer-queue"/);
  assert.match(markup, /data-bf-part="title"[^>]*>.*Wait for sending.*13/s);
  const attachmentBadge = markup.match(
    /<span[^>]*data-bf-part="attachmentCount"[^>]*>3<\/span>/,
  )?.[0];
  assert.ok(attachmentBadge);
  assert.match(attachmentBadge, /aria-label="3 image attachments"/);
  assert.match(markup, /aria-label="Send now"/);
});

test("ChatComposer queue keeps four message rows visible before scrolling", async () => {
  const styles = await readFile(
    new URL("../src/flow-chat/composer/ChatComposerQueue.module.css", import.meta.url),
    "utf8",
  );
  const listRule = styles.match(/\.list\s*\{[^}]*\}/s)?.[0];

  assert.ok(listRule);
  const maxBlockSize = listRule.match(/max-block-size:\s*calc\((.*?)\);/s)?.[1];
  assert.ok(maxBlockSize);
  assert.equal(
    (maxBlockSize.match(/--bf-control-chat-composer-control-height/g) ?? []).length,
    4,
  );
  assert.equal((maxBlockSize.match(/--bf-space-1/g) ?? []).length, 3);
  assert.match(listRule, /overflow-y:\s*auto/);
  assert.match(listRule, /overscroll-behavior-y:\s*contain/);
});

test("ChatComposer queue reveals row actions on hover and keyboard focus", async () => {
  const styles = await readFile(
    new URL("../src/flow-chat/composer/ChatComposerQueue.module.css", import.meta.url),
    "utf8",
  );

  assert.match(styles, /@media \(hover: hover\) and \(pointer: fine\)/);
  assert.match(
    styles,
    /\.actions\s*\{[^}]*pointer-events:\s*none;[^}]*opacity:\s*0;/s,
  );
  assert.match(
    styles,
    /\.item:hover \.actions,\s*\.item:focus-within \.actions\s*\{[^}]*pointer-events:\s*auto;[^}]*opacity:\s*1;/s,
  );
});

test("ChatComposer queue promotes message text from secondary to primary on interaction", async () => {
  const styles = await readFile(
    new URL("../src/flow-chat/composer/ChatComposerQueue.module.css", import.meta.url),
    "utf8",
  );
  const contentRule = styles.match(/\.content\s*\{[^}]*\}/s)?.[0];

  assert.ok(contentRule);
  assert.match(contentRule, /color:\s*var\(--bf-color-content-secondary\)/);
  assert.match(
    styles,
    /\.item:hover \.content,\s*\.item:focus-within \.content\s*\{[^}]*color:\s*var\(--bf-color-content-primary\)/s,
  );
});

test("ChatComposer resolves compound slots into the same stable anatomy", () => {
  const markup = renderToStaticMarkup(
    createElement(
      ChatComposer,
      { layout: "expanded" },
      createElement(
        ChatComposerContent,
        null,
        createElement("div", { "data-editor": true }, "Draft"),
      ),
      createElement(
        ChatComposerStartActions,
        null,
        createElement("button", { type: "button" }, "Add"),
      ),
      createElement(
        ChatComposerEndActions,
        null,
        createElement("button", { type: "button" }, "Send"),
      ),
    ),
  );

  assert.match(markup, /data-bf-part="content"[^>]*><div data-editor="true">Draft<\/div>/);
  assert.match(markup, /data-bf-part="startActions"[^>]*><button type="button">Add<\/button>/);
  assert.match(markup, /data-bf-part="endActions"[^>]*><button type="button">Send<\/button>/);
  assert.equal((markup.match(/>Draft</g) ?? []).length, 1);
});

test("ChatComposer geometry is driven by public system and semantic tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-control-height-md/);
  assert.match(styles, /--bf-control-chat-composer-compact-gap/);
  assert.match(styles, /--bf-control-chat-composer-compact-height/);
  assert.match(styles, /--bf-control-chat-composer-compact-padding-block/);
  assert.match(styles, /--bf-control-chat-composer-compact-padding-inline/);
  assert.match(styles, /--bf-control-chat-composer-compact-track-height/);
  assert.match(styles, /--bf-control-chat-composer-action-icon-size/);
  assert.match(styles, /--bf-control-chat-composer-control-height/);
  assert.match(styles, /--bf-space-8/);
  assert.match(styles, /--bf-radius-2xl/);
  assert.match(styles, /--bf-color-surface-panel/);
  assert.match(styles, /--bf-color-surface-subtle/);
  assert.match(styles, /--bf-color-surface-raised/);
  const contextBackgroundRule = styles.match(
    /\.\w+\[data-has-context=(?:"true"|true)\][^{]*\{[^}]*\}/,
  );
  assert.ok(contextBackgroundRule);
  assert.match(
    contextBackgroundRule[0],
    /background:\s*color-mix\(in srgb,\s*var\(--bf-color-surface-panel\)\s*94%,\s*var\(--bf-color-content-primary\)\s*6%\)/,
  );
  assert.doesNotMatch(contextBackgroundRule[0], /\btransparent\b/);
  assert.match(styles, /--bf-color-action-neutral-border/);
  assert.match(styles, /--bf-shadow-base/);
  assert.match(
    styles,
    /border:\s*var\(--bf-border-width-default\)\s+solid\s+var\(--bf-color-action-neutral-border\)/,
  );
  assert.match(styles, /min-block-size:\s*var\(--bf-control-height-md\)/);
  assert.match(styles, /grid-template-areas:\s*"start content end"/);
  assert.match(
    styles,
    /grid-template-rows:\s*var\(--bf-control-chat-composer-compact-track-height\)/,
  );
  assert.match(styles, /"content content"\s*"start end"/);
  assert.match(
    styles,
    /\[data-bf-layout=(?:"compact"|compact)\][^{]*\{[^}]*block-size:\s*var\(--bf-control-chat-composer-compact-height\)[^}]*border-radius:\s*var\(--bf-radius-pill\)/,
  );
  assert.match(
    styles,
    /\[data-bf-layout=(?:"compact"|compact)\][^{]*\.\w+\s*\{[^}]*block-size:\s*var\(--bf-control-chat-composer-compact-track-height\)[^}]*align-self:\s*center/,
  );
});
