import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";
import { createHash } from "node:crypto";
import { Icon, iconNames, canonicalIconNames, iconAliases } from "../dist/index.js";

test("Icon exposes the complete named catalog without duplicate names", () => {
  assert.equal(iconNames.length, 66);
  assert.equal(canonicalIconNames.length, 63);
  assert.deepEqual(Object.keys(iconAliases).sort(), ["circle", "download"]);
  for (const name of ["thinking", "git", "duplicate", "chevron-left", "selected", "delete", "waitlist-message", "creative", "ultimate", "standard", "minimal", "arrow-down", "unselected"]) {
    assert.ok(canonicalIconNames.includes(name), name);
  }
  assert.equal(new Set(iconNames).size, iconNames.length);
  assert.ok(iconNames.includes("search"));
  assert.ok(iconNames.includes("commit"));
  assert.ok(iconNames.includes("sidebar-right"));
  assert.ok(iconNames.includes("chevron-up"));
  assert.ok(iconNames.includes("refresh"));
});

test("Icon is decorative by default and owns its exact asset source", () => {
  const markup = renderToStaticMarkup(createElement(Icon, { name: "search" }));

  assert.match(markup, /data-bf-component="icon"/);
  assert.match(markup, /data-bf-name="search"/);
  assert.match(markup, /data-size="lg"/);
  assert.match(markup, /aria-hidden="true"/);
  assert.match(markup, /mask-image:url/);
  assert.doesNotMatch(markup, /<svg/);
});

test("Icon exposes semantic size, tone, and accessible label independently", () => {
  const markup = renderToStaticMarkup(createElement(Icon, {
    label: "Successful",
    name: "check-circle",
    size: "sm",
    tone: "success",
  }));

  assert.match(markup, /role="img"/);
  assert.match(markup, /aria-label="Successful"/);
  assert.doesNotMatch(markup, /aria-hidden/);
  assert.match(markup, /data-size="sm"/);
  assert.match(markup, /data-bf-tone="success"/);
});

test("Icon styles consume only public geometry and semantic color tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-control-icon-size2xs/);
  assert.match(styles, /--bf-control-icon-size-lg/);
  assert.match(styles, /--bf-color-content-primary/);
  assert.match(styles, /--bf-color-status-success-content/);
  assert.match(styles, /mask-size:contain/);
});

test("Icon mask assets are color-agnostic", async () => {
  const assetDirectory = new URL("../src/components/Icon/assets/", import.meta.url);
  const assetNames = (await readdir(assetDirectory)).filter((name) => name.endsWith(".svg"));

  assert.equal(assetNames.length, 64);
  for (const assetName of assetNames) {
    const source = await readFile(new URL(assetName, assetDirectory), "utf8");
    assert.match(source, /(?:fill|stroke)="currentColor"/i, `${assetName} must use currentColor`);
    assert.doesNotMatch(
      source,
      /\b(?:fill|stroke)="(?:black|white|#[0-9a-f]{3,8}|rgba?\()/i,
      `${assetName} must not own a color`,
    );
  }
});

test("Icon preserves all reviewed asset geometry and opacity", async () => {
  const assets = new URL("../src/components/Icon/assets/", import.meta.url);
  const fingerprints = JSON.parse(await readFile(new URL("fixtures/icon-assets.json", import.meta.url), "utf8"));
  assert.equal(fingerprints.length, 64);
  assert.equal(new Set(fingerprints.map(entry => entry.node)).size, 64);
  assert.deepEqual((await readdir(assets)).filter(name => name.endsWith(".svg")).sort(), fingerprints.map(entry => entry.asset).sort());
  for (const entry of fingerprints) {
    const source = (await readFile(new URL(entry.asset, assets), "utf8")).replaceAll("\r\n", "\n").trim();
    assert.equal(createHash("sha256").update(source).digest("hex"), entry.sha256, `${entry.name}: review geometry, viewBox and opacity before updating its fingerprint`);
    assert.ok(iconNames.includes(entry.name), `${entry.name} is not registered`);
  }
});

test("compatibility aliases share the canonical mask without duplicating assets", () => {
  for (const [alias, canonical] of [["download", "arrow-down"], ["circle", "unselected"]]) {
    const renderMask = name => renderToStaticMarkup(createElement(Icon, { name })).match(/style="([^"]+)"/)?.[1];
    assert.equal(renderMask(alias), renderMask(canonical));
    assert.ok(!canonicalIconNames.includes(alias));
  }
  assert.ok(!canonicalIconNames.includes("turn"));
});

test("published Icon masks contain the current asset attributes for every catalog entry", async () => {
  const assets = new URL("../src/components/Icon/assets/", import.meta.url);
  const catalog = JSON.parse(await readFile(new URL("fixtures/icon-assets.json", import.meta.url), "utf8"));
  const attributes = svg => [...svg.matchAll(/\b([\w:-]+)=["']([^"']*)["']/g)]
    .map(match => [match[1], match[2].trim().replace(/\s+/g, " ")]);

  for (const entry of catalog) {
    const markup = renderToStaticMarkup(createElement(Icon, { name: entry.name }));
    const mask = markup.match(/mask-image:url\(&quot;(.*?)&quot;\)/)?.[1];
    assert.ok(mask, `${entry.name} must render an asset mask`);
    assert.match(mask, /^data:image\/svg\+xml,/, `${entry.name} must include its published asset`);
    const svg = decodeURIComponent(mask.slice(mask.indexOf(",") + 1))
      .replaceAll("&#x27;", "'").replaceAll("&amp;", "&");
    const source = await readFile(new URL(entry.asset, assets), "utf8");
    assert.deepEqual(attributes(svg), attributes(source), `${entry.name} must not ship stale or miswired geometry`);
  }
});

test("Combobox constrains catalog glyphs in both value and indicator slots", async () => {
  const source = await readFile(new URL("../src/components/Combobox/Combobox.module.css", import.meta.url), "utf8");
  for (const slot of ["valueLeading", "indicator"]) {
    assert.match(source, new RegExp(`\\.${slot} > \\[data-bf-component="icon"\\]\\s*\\{\\s*inline-size: 100%;\\s*block-size: 100%;`));
  }
});
