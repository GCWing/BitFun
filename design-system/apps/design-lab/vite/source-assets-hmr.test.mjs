import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { createServer, loadConfigFromFile } from "vite";
import { watchSourcePlugin } from "../../../tooling/vite/watch-source.mjs";

const uiSourceDirectory = fileURLToPath(new URL("../../../packages/ui/src/", import.meta.url));

async function eventually(check, message) {
  const deadline = Date.now() + 5000;
  do {
    if (await check()) return;
    await delay(25);
  } while (Date.now() < deadline);
  assert.fail(message);
}

function inlineSvg(module) {
  const value = JSON.parse(module.code.match(/export default (".*")/)?.[1] ?? "null");
  assert.match(value, /^data:image\/svg\+xml,/);
  return decodeURIComponent(value.slice(value.indexOf(",") + 1));
}

function geometry(svg) {
  return [...svg.matchAll(/\bd=["']([^"']+)["']/g)].map(match => match[1]);
}

test("editing an external UI SVG invalidates its inline module without a restart", async () => {
  const fixture = await mkdtemp(path.join(tmpdir(), "bitfun-ui-asset-hmr-"));
  let server;
  try {
    const root = path.join(fixture, "app");
    const source = path.join(fixture, "ui", "src");
    const assets = path.join(source, "components", "Icon", "assets");
    const asset = path.join(assets, "thinking.svg");
    await mkdir(root, { recursive: true });
    await mkdir(assets, { recursive: true });
    const before = await readFile(path.join(uiSourceDirectory, "components/Icon/assets/thinking.svg"), "utf8");
    const after = await readFile(path.join(uiSourceDirectory, "components/Icon/assets/git.svg"), "utf8");
    await writeFile(asset, before);
    server = await createServer({
      configFile: false,
      root,
      logLevel: "silent",
      plugins: [watchSourcePlugin(source)],
      optimizeDeps: { noDiscovery: true, include: [] },
      server: { middlewareMode: true, hmr: false },
    });
    const url = `/@fs/${asset.replaceAll("\\", "/")}?import`;
    assert.deepEqual(geometry(inlineSvg(await server.transformRequest(url))), geometry(before));

    await eventually(
      () => server.watcher.getWatched()[assets]?.includes("thinking.svg"),
      "UI asset directory must be watched outside the application root",
    );
    let changed = false;
    server.watcher.on("change", file => {
      if (path.normalize(file) === asset) changed = true;
    });
    await writeFile(asset, after);
    await eventually(() => changed, "SVG edit must reach the Vite watcher");
    await eventually(
      async () => JSON.stringify(geometry(inlineSvg(await server.transformRequest(url)))) === JSON.stringify(geometry(after)),
      "The same imported URL must return the new SVG geometry after editing",
    );
  } finally {
    await server?.close();
    await rm(fixture, { recursive: true, force: true });
  }
});

test("Design Lab registers shared UI source watching only for development", async () => {
  const loaded = await loadConfigFromFile(
    { command: "serve", mode: "development" },
    fileURLToPath(new URL("../vite.config.ts", import.meta.url)),
  );
  const plugins = loaded.config.plugins.flat(Infinity);
  const watcher = plugins.find(plugin => plugin?.name === "bitfun:watch-ui-source");
  assert.ok(watcher, "The real application config must install the source watcher");
  assert.equal(watcher.apply, "serve");
  const watched = [];
  watcher.configureServer({ watcher: { add: directory => watched.push(directory) } });
  assert.deepEqual(watched.map(directory => path.resolve(directory)), [path.resolve(uiSourceDirectory)]);
});
