/**
 * OpenBitFun Installer build script.
 *
 * Steps:
 * 1. Build OpenBitFun main app (optional).
 * 2. Prepare installer payload from built app binaries.
 * 3. Build installer app (Tauri).
 *
 * Usage:
 *   node scripts/build-installer.cjs [--skip-app-build] [--dev] [--mode fast|release] [--app-exe path]
 *   node scripts/build-installer.cjs --fast   # same as --mode fast
 */

const { execSync } = require("child_process");
const { createHash } = require("crypto");
const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const OPENBITFUN_ROOT = path.resolve(ROOT, "..");
const PAYLOAD_DIR = path.join(ROOT, "src-tauri", "payload");

const rawArgs = process.argv.slice(2);
const skipAppBuild = rawArgs.includes("--skip-app-build");
const isDev = rawArgs.includes("--dev");
const showHelp = rawArgs.includes("--help") || rawArgs.includes("-h");
const STRICT_PAYLOAD_VALIDATION = !isDev;
const MIN_APP_EXE_BYTES = 5 * 1024 * 1024;
const REQUIRED_PAYLOAD_FILES = [
  "openbitfun-desktop.exe",
  "frontend/dist/index.html",
  "mobile-web/dist/index.html",
  "resources/ext-host/extension-host.js",
  "resources/worker_host.js",
  "flashgrep/flashgrep-x86_64-pc-windows-msvc.exe",
];

function getMode(args) {
  if (args.includes("--fast")) return "fast";
  const modeFlagIndex = args.indexOf("--mode");
  if (modeFlagIndex >= 0 && args[modeFlagIndex + 1]) {
    return args[modeFlagIndex + 1].trim();
  }
  return "release";
}

function getArgValue(args, flag) {
  const flagIndex = args.indexOf(flag);
  if (flagIndex < 0) return null;
  const value = args[flagIndex + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}`);
  }
  return value.trim();
}

const buildMode = getMode(rawArgs);
const validModes = new Set(["fast", "release"]);

function log(msg) {
  console.log(`\x1b[36m[installer]\x1b[0m ${msg}`);
}

function error(msg) {
  console.error(`\x1b[31m[installer]\x1b[0m ${msg}`);
  process.exit(1);
}

function run(cmd, cwd = ROOT) {
  log(`> ${cmd}`);
  try {
    execSync(cmd, { cwd, stdio: "inherit" });
  } catch (_e) {
    error(`Command failed: ${cmd}`);
  }
}

function printHelpAndExit() {
  console.log(`
OpenBitFun Installer build script

Usage:
  node scripts/build-installer.cjs [options]

Options:
  --mode <fast|release>  Build mode (default: release)
  --fast                 Alias for --mode fast
  --skip-app-build       Skip building main OpenBitFun app
  --app-exe <path>       Exact desktop executable to package. Required when the
                         desired build is outside the active Cargo target dir.
  --dev                  Run installer with tauri dev instead of tauri build
                         and allow placeholder payload fallback
  --help, -h             Show this help
`);
  process.exit(0);
}

function getMainAppBuildCommand(mode) {
  if (mode === "fast") {
    return "pnpm run desktop:build:release-fast";
  }
  return "pnpm run desktop:build:exe";
}

function getInstallerBuildCommand(mode, devMode) {
  if (devMode) return "pnpm run tauri:dev";
  if (mode === "fast") return "pnpm run tauri:build:exe:fast";
  return "pnpm run tauri:build:exe";
}

function ensureCleanDir(dir) {
  if (fs.existsSync(dir)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
  fs.mkdirSync(dir, { recursive: true });
}

function sha256File(filePath) {
  const content = fs.readFileSync(filePath);
  return createHash("sha256").update(content).digest("hex");
}

function writeFileWithManifest(src, dest, manifest, payloadRoot) {
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.copyFileSync(src, dest);
  const size = fs.statSync(dest).size;
  const rel = path.relative(payloadRoot, dest).replace(/\\/g, "/");
  manifest.files.push({
    path: rel,
    size,
    sha256: sha256File(dest),
  });
}

function copyDirRecursiveWithManifest(srcDir, destDir, manifest, payloadRoot) {
  fs.mkdirSync(destDir, { recursive: true });
  for (const entry of fs.readdirSync(srcDir, { withFileTypes: true })) {
    const src = path.join(srcDir, entry.name);
    const dest = path.join(destDir, entry.name);
    if (entry.isDirectory()) {
      copyDirRecursiveWithManifest(src, dest, manifest, payloadRoot);
      continue;
    }
    writeFileWithManifest(src, dest, manifest, payloadRoot);
  }
}

function shouldCopySiblingRuntimeFile(fileName, appExeBaseName) {
  if (fileName === appExeBaseName) return false;
  if (fileName.startsWith(".cargo-")) return false;

  const lower = fileName.toLowerCase();
  if (
    lower.endsWith(".pdb") ||
    lower.endsWith(".d") ||
    lower.endsWith(".exp") ||
    lower.endsWith(".lib") ||
    lower.endsWith(".ilk")
  ) {
    return false;
  }

  return true;
}

function getExpectedAppProfile(mode) {
  return mode === "fast" ? "release-fast" : "release";
}

function resolveCargoTargetDir(env = process.env) {
  const configured = env.CARGO_TARGET_DIR;
  if (!configured) return path.join(OPENBITFUN_ROOT, "target");
  return path.isAbsolute(configured)
    ? path.normalize(configured)
    : path.resolve(OPENBITFUN_ROOT, configured);
}

function resolveAppExePath(mode, args = rawArgs, env = process.env) {
  const configured = getArgValue(args, "--app-exe") || env.OPENBITFUN_INSTALLER_APP_EXE;
  if (configured) {
    return path.isAbsolute(configured)
      ? path.normalize(configured)
      : path.resolve(OPENBITFUN_ROOT, configured);
  }

  return path.join(
    resolveCargoTargetDir(env),
    getExpectedAppProfile(mode),
    "openbitfun-desktop.exe"
  );
}

function getPayloadSourceExeName(appExePath) {
  return path.basename(appExePath);
}

function validateRequiredPayloadFiles(payloadDir, manifest) {
  const manifestPaths = new Set(manifest.files.map((entry) => entry.path));
  const missing = REQUIRED_PAYLOAD_FILES.filter((relativePath) => {
    const diskPath = path.join(payloadDir, ...relativePath.split("/"));
    return !manifestPaths.has(relativePath) || !fs.existsSync(diskPath);
  });
  if (missing.length > 0) {
    throw new Error(
      `Installer payload is incomplete. Missing required files: ${missing.join(", ")}`
    );
  }
}

function main() {
  if (showHelp) {
    printHelpAndExit();
  }

  if (!validModes.has(buildMode)) {
    error(`Invalid mode "${buildMode}". Supported: fast, release`);
  }

log(`Build mode: ${buildMode}`);
if (isDev) {
  log("Installer run mode: dev");
} else {
  log("Installer run mode: release (strict payload validation)");
}

// Step 1: Build main OpenBitFun app.
if (!skipAppBuild) {
  log("Step 1: Building OpenBitFun main application...");
  run(getMainAppBuildCommand(buildMode), OPENBITFUN_ROOT);
} else {
  log("Step 1: Skipped (--skip-app-build)");
}

// Step 2: Prepare payload.
log("Step 2: Preparing installer payload...");

  const expectedAppExePath = resolveAppExePath(buildMode);
  const appExePath = fs.existsSync(expectedAppExePath) ? expectedAppExePath : null;

  if (!appExePath && STRICT_PAYLOAD_VALIDATION) {
    error(
      `Could not find the exact ${getExpectedAppProfile(buildMode)} OpenBitFun executable at ${expectedAppExePath}. ` +
        "Build that profile first, set OPENBITFUN_INSTALLER_APP_EXE, or pass --app-exe."
    );
  }

if (appExePath) {
  ensureCleanDir(PAYLOAD_DIR);

  const manifest = {
    generatedAt: new Date().toISOString(),
    mode: buildMode,
    sourceExe: getPayloadSourceExeName(appExePath),
    files: [],
  };

  const destExe = path.join(PAYLOAD_DIR, "openbitfun-desktop.exe");
  writeFileWithManifest(appExePath, destExe, manifest, PAYLOAD_DIR);
  log(`Copied: ${appExePath} -> ${destExe}`);

  const exeSize = fs.statSync(destExe).size;
  if (STRICT_PAYLOAD_VALIDATION && exeSize < MIN_APP_EXE_BYTES) {
    error(
      `openbitfun-desktop.exe in payload is unexpectedly small (${exeSize} bytes). Refusing to continue.`
    );
  }

  const releaseDir = path.dirname(appExePath);
  const appExeBaseName = path.basename(appExePath);
  const siblingFiles = fs
    .readdirSync(releaseDir, { withFileTypes: true })
    .filter((e) => e.isFile())
    .map((e) => e.name)
    .filter((file) => shouldCopySiblingRuntimeFile(file, appExeBaseName));

  for (const file of siblingFiles) {
    const src = path.join(releaseDir, file);
    const dest = path.join(PAYLOAD_DIR, file);
    writeFileWithManifest(src, dest, manifest, PAYLOAD_DIR);
    log(`Copied runtime file: ${file}`);
  }

  // Keep installer payload aligned with the desktop app's runtime lookup paths.
  // `mobile-web` may be emitted as a sibling directory in no-bundle builds.
  const runtimeDirs = [
    "resources",
    "locales",
    "swiftshader",
    "mobile-web",
    "frontend",
    "flashgrep",
  ];
  for (const dirName of runtimeDirs) {
    const srcDir = path.join(releaseDir, dirName);
    if (!fs.existsSync(srcDir) || !fs.statSync(srcDir).isDirectory()) {
      continue;
    }
    const destDir = path.join(PAYLOAD_DIR, dirName);
    copyDirRecursiveWithManifest(srcDir, destDir, manifest, PAYLOAD_DIR);
    log(`Copied runtime directory: ${dirName}`);
  }

  if (STRICT_PAYLOAD_VALIDATION) {
    try {
      validateRequiredPayloadFiles(PAYLOAD_DIR, manifest);
    } catch (validationError) {
      error(validationError.message);
    }
  }

  const manifestPath = path.join(PAYLOAD_DIR, "payload-manifest.json");
  fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
  log(`Wrote payload manifest: ${manifestPath}`);

  if (STRICT_PAYLOAD_VALIDATION && manifest.files.length === 0) {
    error("Payload manifest has no files. Refusing to build installer.");
  }
} else {
  log("No app executable found. Payload directory will be empty (dev-only fallback).");
  ensureCleanDir(PAYLOAD_DIR);
}

// Step 3: Build installer.
log("Step 3: Building installer...");
run(getInstallerBuildCommand(buildMode, isDev));

const installerTargetProfile = isDev
  ? "debug"
  : buildMode === "fast"
    ? "release-fast"
    : "release";
log("Installer build complete.");
if (isDev) {
  log(
    `Output directory: ${path.join(
      ROOT,
      "src-tauri",
      "target",
      installerTargetProfile
    )}`
  );
} else {
  log(
    `Output: ${path.join(
      ROOT,
      "src-tauri",
      "target",
      installerTargetProfile,
      "openbitfun-installer.exe"
    )}`
  );
}
}

if (require.main === module) {
  main();
}

module.exports = {
  REQUIRED_PAYLOAD_FILES,
  getExpectedAppProfile,
  getPayloadSourceExeName,
  resolveAppExePath,
  resolveCargoTargetDir,
  shouldCopySiblingRuntimeFile,
  validateRequiredPayloadFiles,
};
