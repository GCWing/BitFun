import assert from "node:assert/strict";
import test from "node:test";
import {
  interpolateMessage,
  resolveDesignLabLocale,
  resolveFirstDesignLabLocale,
  translateFromCatalog,
} from "../src/i18n/core.mjs";

test("normalizes shared locale aliases and browser locale variants", () => {
  assert.equal(resolveDesignLabLocale("en"), "en-US");
  assert.equal(resolveDesignLabLocale("en_GB"), "en-US");
  assert.equal(resolveDesignLabLocale("zh-Hans"), "zh-CN");
  assert.equal(resolveDesignLabLocale("zh-Hant-TW"), "zh-TW");
  assert.equal(resolveDesignLabLocale("zh-HK"), "zh-TW");
  assert.equal(resolveDesignLabLocale("fr-FR"), null);
});

test("chooses the first supported locale and falls back to en-US", () => {
  assert.equal(
    resolveFirstDesignLabLocale(["fr-FR", "zh-Hant"]),
    "zh-TW",
  );
  assert.equal(resolveFirstDesignLabLocale(["de-DE"]), "en-US");
});

test("interpolates known parameters without deleting unresolved placeholders", () => {
  assert.equal(
    interpolateMessage("Showing {visible} of {total}", { visible: 4, total: 12 }),
    "Showing 4 of 12",
  );
  assert.equal(
    interpolateMessage("Hello {name} from {place}", { name: "BitFun" }),
    "Hello BitFun from {place}",
  );
});

test("uses the English message before returning the missing key", () => {
  const catalog = {
    "en-US": { greeting: "Hello {name}" },
    "zh-CN": {},
    "zh-TW": {},
  };

  assert.equal(
    translateFromCatalog(catalog, "zh-CN", "greeting", { name: "BitFun" }),
    "Hello BitFun",
  );
  assert.equal(
    translateFromCatalog(catalog, "zh-CN", "missing"),
    "missing",
  );
});
