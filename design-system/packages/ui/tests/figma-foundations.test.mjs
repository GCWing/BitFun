import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  Heading,
  Input,
  KeyHint,
  ListItem,
  MediaThumbnail,
  NavigationList,
  NavigationListSection,
  PromptComposer,
  Search,
} from "../dist/index.js";

test("Input preserves native input semantics for the Figma normal scene", () => {
  const markup = renderToStaticMarkup(createElement(Input, {
    "aria-label": "Description",
    defaultValue: "BitFun is an AI-driven programming environment.",
  }));

  assert.match(markup, /data-bf-component="input"/);
  assert.match(markup, /<input[^>]+aria-label="Description"/);
  assert.doesNotMatch(markup, /data-variant="search"/);
});

test("Search owns the Figma search scene and composes its shortcut label", () => {
  const markup = renderToStaticMarkup(createElement(Search, {
    "aria-label": "Search",
    placeholder: "Search",
    trailingContent: createElement(KeyHint, null, "⌘ K"),
  }));

  assert.match(markup, /data-bf-component="search"/);
  assert.match(markup, /<input[^>]+aria-label="Search"[^>]+type="search"/);
  assert.match(markup, /data-bf-part="leadingIcon"/);
  assert.match(markup, /data-bf-component="key-hint"/);
});

test("ListItem keeps end actions beside the main button", () => {
  const markup = renderToStaticMarkup(createElement(ListItem, {
    actions: createElement("button", { "aria-label": "More", type: "button" }, "More"),
    leadingIcon: createElement("svg", { "data-icon": "user" }),
    selected: true,
  }, "AI Assistant"));

  assert.match(markup, /data-bf-component="list-item"/);
  assert.match(markup, /data-selected="true"/);
  assert.match(markup, /AI Assistant/);
  assert.match(markup, /<\/button><span[^>]+data-bf-part="actions"><button/);
});

test("Heading uses the requested semantic level and Figma presentation", () => {
  const markup = renderToStaticMarkup(createElement(Heading, {
    description: "Interface language and visual appearance",
    level: 1,
    title: "Appearance",
    variant: "page",
  }));

  assert.match(markup, /data-bf-component="heading"/);
  assert.match(markup, /data-variant="page"/);
  assert.match(markup, /<h1[^>]*>Appearance<\/h1>/);
  assert.match(markup, /Interface language and visual appearance/);
});

test("MediaThumbnail keeps meaningful image alternative text in every presentation", () => {
  const markup = renderToStaticMarkup(createElement(MediaThumbnail, {
    alt: "BitFun homepage",
    presentation: "stacked",
    src: "/homepage.png",
  }));

  assert.match(markup, /data-bf-component="media-thumbnail"/);
  assert.match(markup, /data-bf-presentation="stacked"/);
  assert.match(markup, /<img alt="BitFun homepage"/);
});

test("NavigationList exposes grouped links through a native navigation landmark", () => {
  const markup = renderToStaticMarkup(createElement(
    NavigationList,
    { "aria-label": "Settings" },
    createElement(
      NavigationListSection,
      { title: "General" },
      createElement(ListItem, { selected: true }, "Appearance"),
    ),
  ));

  assert.match(markup, /<nav[^>]+aria-label="Settings"/);
  assert.match(markup, /data-bf-component="navigation-list-section"/);
  assert.match(markup, />General</);
  assert.match(markup, />Appearance</);
});

test("PromptComposer preserves native textarea semantics and sibling control slots", () => {
  const markup = renderToStaticMarkup(createElement(PromptComposer, {
    "aria-label": "Prompt",
    endControls: createElement("button", { type: "button" }, "Send"),
    placeholder: "How can I help you...",
    startControls: createElement("button", { type: "button" }, "Add"),
  }));

  assert.match(markup, /data-bf-component="prompt-composer"/);
  assert.match(markup, /<textarea[^>]+aria-label="Prompt"/);
  assert.match(markup, /data-bf-part="start-controls"><button/);
  assert.match(markup, /data-bf-part="end-controls"><button/);
});

test("new Figma foundations consume generated semantic tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  for (const token of [
    "--bf-control-search-height",
    "--bf-control-key-hint-icon-size",
    "--bf-control-list-item-height",
    "--bf-control-media-thumbnail-width",
    "--bf-control-navigation-list-width",
    "--bf-control-prompt-composer-height",
    "--bf-font-size-page-title",
    "--bf-font-size-hero",
  ]) {
    assert.match(styles, new RegExp(token));
  }
});
