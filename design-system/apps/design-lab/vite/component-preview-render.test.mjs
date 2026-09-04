import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { after, before, test } from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer } from "vite";
import react from "@vitejs/plugin-react";
import { componentRegistry } from "@openbitfun/ui/registry";

let server;
let render;

before(async () => {
  // Load actual Lab modules without a browser, generated inspector, or mock components.
  server = await createServer({
    root: fileURLToPath(new URL("..", import.meta.url)),
    configFile: false,
    plugins: [react()],
    server: { middlewareMode: true, hmr: false },
    optimizeDeps: { noDiscovery: true, include: [] },
    appType: "custom",
  });
  const { ComponentDetailPage } = await server.ssrLoadModule("/src/pages/ComponentDetailPage.tsx");
  const { I18nContext } = await server.ssrLoadModule("/src/i18n/I18nProvider.tsx");
  const { messages } = await server.ssrLoadModule("/src/i18n/messages.ts");
  const { translateFromCatalog } = await server.ssrLoadModule("/src/i18n/core.mjs");
  render = (name) => {
    const component = componentRegistry.find(item => item.name === name) ?? { name, states: ["default"], props: [], tokens: [], category: "form", description: "", maturity: "stable" };
    const html = renderToStaticMarkup(createElement(I18nContext.Provider, {
      value: { locale: "zh-CN", setLocale() {}, t: (key, params) => translateFromCatalog(messages, "zh-CN", key, params) },
    }, createElement(ComponentDetailPage, {
      component, colorScheme: "light", contrast: "standard", density: "comfortable",
      tokenOverrides: {}, onBack() {}, onInspectTokens() {},
    })));
    // Exclude inspector controls, which legitimately contain switches.
    const preview = html.split('id="component-workbench"')[1]?.split('<section class="component-code-panel')[0];
    assert.ok(preview, `Missing preview panel: ${name}`);
    return { html, preview };
  };
});

after(async () => { await server?.close(); });

test("Textarea renders multiline fields and the registered input states, not switches", () => {
  const { html, preview } = render("Textarea");
  assert.equal((preview.match(/data-openbitfun-component="textarea"/g) ?? []).length, 5);
  assert.equal((preview.match(/<textarea\b/g) ?? []).length, 5);
  assert.match(preview, /lab-state-hover/);
  assert.match(preview, /lab-state-focus-visible/);
  assert.match(preview, /aria-invalid="true"/);
  assert.match(preview, /<textarea[^>]*disabled=""/);
  assert.match(preview, /data-openbitfun-part="count"/);
  assert.doesNotMatch(preview, /data-openbitfun-component="switch"|>Switch<|>关闭<|>开启</);
  assert.match(html, /import \{ Textarea \}/);
});

test("other components formerly using the fallback render their own state specimens", () => {
  for (const name of ["Alert", "Avatar", "Checkbox", "NumberInput", "Radio", "Empty"]) {
    const { preview, html } = render(name);
    const componentId = name.replace(/([a-z])([A-Z])/g, "$1-$2").toLowerCase();
    assert.match(preview, new RegExp(`data-openbitfun-component="${componentId}"`), name);
    assert.match(preview, new RegExp(`data-state-count="${componentRegistry.find(item => item.name === name).states.length}"`), name);
    assert.doesNotMatch(preview, /data-openbitfun-component="switch"|>Switch</, name);
    assert.doesNotMatch(html, /import \{ Switch \}/, name);
  }
  assert.match(render("Alert").preview, /data-openbitfun-tone="error"/);
  assert.match(render("Avatar").preview, /<img/);
});

test("LauncherButton renders every interaction state with the catalog mic", () => {
  const { html, preview } = render("LauncherButton");

  assert.equal(
    (preview.match(/data-openbitfun-component="launcher-button"/g) ?? []).length,
    5,
  );
  assert.match(preview, /data-openbitfun-name="mic"/);
  assert.match(preview, /data-openbitfun-preview-state="hover"/);
  assert.match(preview, /data-openbitfun-preview-state="active"/);
  assert.match(preview, /<button[^>]*disabled=""/);
  assert.match(html, /import \{ Icon, LauncherButton \}/);
});

test("FieldGroup states use independent full-width stages with distinct labels", () => {
  const { preview } = render("FieldGroup");
  assert.equal((preview.match(/class="component-surface-state-list__item"/g) ?? []).length, 3);
  assert.equal((preview.match(/class="component-surface-state-list__preview"/g) ?? []).length, 3);
  assert.match(preview, /无背景表面/);
  assert.match(preview, /带分隔线/);
  assert.match(preview, /data-dividers="true"/);
  assert.doesNotMatch(preview, /component-preview-matrix/);
});

test("Switch is explicit and unknown components never silently become switches", () => {
  assert.match(render("Switch").preview, /data-openbitfun-component="switch"/);
  const { preview } = render("UnregisteredExample");
  assert.match(preview, /此组件尚未实现预览/);
  assert.doesNotMatch(preview, /data-openbitfun-component="switch"/);
});

test("Icon details include real mixed-icon compositions at every button size", () => {
  const { preview } = render("Icon");
  assert.match(preview, /component-icon-composition/);
  for (const size of ["xs", "sm", "md", "lg"]) {
    assert.match(preview, new RegExp(`Button / ${size}`));
    assert.match(preview, new RegExp(`aria-label="SVG / ${size}"`));
    assert.match(preview, new RegExp(`aria-label="Icon / ${size}"`));
  }
  assert.match(preview, /data-openbitfun-component="tab-group"/);
  assert.match(preview, /data-openbitfun-component="input"/);
  assert.match(preview, /component-icon-catalog/);
});
