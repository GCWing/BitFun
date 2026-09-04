const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const {
  REQUIRED_PAYLOAD_FILES,
  getExpectedAppProfile,
  getPayloadSourceExeName,
  resolveAppExePath,
  shouldCopySiblingRuntimeFile,
  validateRequiredPayloadFiles,
} = require("./build-installer.cjs");

const installerRoot = path.resolve(__dirname, "..");

test("installer modes select only their exact Cargo profile", () => {
  assert.equal(getExpectedAppProfile("release"), "release");
  assert.equal(getExpectedAppProfile("fast"), "release-fast");

  const releasePath = resolveAppExePath("release", [], {});
  assert.match(releasePath, /target[\\/]release[\\/]openbitfun-desktop\.exe$/);
  assert.doesNotMatch(releasePath, /release-fast|debug/);
});

test("an explicit desktop executable wins over target directory discovery", () => {
  const explicit = path.join(os.tmpdir(), "custom-target", "openbitfun-desktop.exe");
  assert.equal(
    resolveAppExePath("release", ["--app-exe", explicit], {
      CARGO_TARGET_DIR: "ignored-target",
    }),
    path.normalize(explicit)
  );
});

test("payload manifests record only the canonical executable name", () => {
  const appExe = path.join(
    os.tmpdir(),
    "openbitfun-build-root",
    "target",
    "release",
    "openbitfun-desktop.exe"
  );
  assert.equal(getPayloadSourceExeName(appExe), "openbitfun-desktop.exe");
  assert.equal(path.isAbsolute(getPayloadSourceExeName(appExe)), false);
});

test("payload validation requires every desktop runtime surface", () => {
  const payload = fs.mkdtempSync(path.join(os.tmpdir(), "openbitfun-installer-payload-"));
  const manifest = { files: [] };

  for (const relativePath of REQUIRED_PAYLOAD_FILES) {
    const diskPath = path.join(payload, ...relativePath.split("/"));
    fs.mkdirSync(path.dirname(diskPath), { recursive: true });
    fs.writeFileSync(diskPath, relativePath);
    manifest.files.push({ path: relativePath });
  }

  assert.doesNotThrow(() => validateRequiredPayloadFiles(payload, manifest));

  fs.rmSync(path.join(payload, "frontend"), { recursive: true, force: true });
  assert.throws(
    () => validateRequiredPayloadFiles(payload, manifest),
    /frontend\/dist\/index\.html/
  );
});

test("Cargo implementation locks are not copied into the installer", () => {
  assert.equal(shouldCopySiblingRuntimeFile(".cargo-lock", "openbitfun-desktop.exe"), false);
  assert.equal(
    shouldCopySiblingRuntimeFile(".cargo-artifact-lock", "openbitfun-desktop.exe"),
    false
  );
  assert.equal(
    shouldCopySiblingRuntimeFile(".cargo-build-lock", "openbitfun-desktop.exe"),
    false
  );
});

test("Rust and JavaScript Tauri package lines stay pinned and aligned", () => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(installerRoot, "package.json"), "utf8")
  );
  const cargoToml = fs.readFileSync(
    path.join(installerRoot, "src-tauri", "Cargo.toml"),
    "utf8"
  );
  const rustTauri = cargoToml.match(/tauri = \{ version = "=([^"]+)"/)?.[1];
  const rustDialog = cargoToml.match(/tauri-plugin-dialog = "=([^"]+)"/)?.[1];
  const jsTauri = packageJson.dependencies["@tauri-apps/api"];
  const jsDialog = packageJson.dependencies["@tauri-apps/plugin-dialog"];

  assert.match(jsTauri, /^\d+\.\d+\.\d+$/);
  assert.match(jsDialog, /^\d+\.\d+\.\d+$/);
  assert.equal(minorLine(rustTauri), minorLine(jsTauri));
  assert.equal(minorLine(rustDialog), minorLine(jsDialog));
});

test("all Installer validators share the required runtime file contract", () => {
  const buildRs = fs.readFileSync(path.join(installerRoot, "src-tauri", "build.rs"), "utf8");
  const commandsRs = fs.readFileSync(
    path.join(installerRoot, "src-tauri", "src", "installer", "commands.rs"),
    "utf8"
  );
  for (const relativePath of REQUIRED_PAYLOAD_FILES) {
    assert.match(buildRs, new RegExp(escapeRegExp(relativePath)));
    if (relativePath === "openbitfun-desktop.exe") {
      assert.match(commandsRs, /MAIN_APP_EXE/);
    } else {
      assert.match(commandsRs, new RegExp(escapeRegExp(relativePath)));
    }
  }
});

function minorLine(version) {
  assert.match(version, /^\d+\.\d+\.\d+$/);
  return version.split(".").slice(0, 2).join(".");
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
