/**
 * BitFun Installer build script.
 *
 * Steps:
 * 1. Build BitFun main app (optional).
 * 2. Prepare installer payload from built app binaries.
 * 3. Build installer app (Tauri).
 *
 * Usage:
 *   node scripts/build-installer.cjs [--skip-app-build] [--dev] [--mode fast|release]
 *   node scripts/build-installer.cjs --fast   # same as --mode fast
 */

const { execSync } = require("child_process");
const { createHash } = require("crypto");
const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const BITFUN_ROOT = path.resolve(ROOT, "..");
const PAYLOAD_DIR = path.join(ROOT, "src-tauri", "payload");

const rawArgs = process.argv.slice(2);
const skipAppBuild = rawArgs.includes("--skip-app-build");
const isDev = rawArgs.includes("--dev");
const showHelp = rawArgs.includes("--help") || rawArgs.includes("-h");
const STRICT_PAYLOAD_VALIDATION = !isDev;
const MIN_APP_EXE_BYTES = 5 * 1024 * 1024;

function getMode(args) {
  if (args.includes("--fast")) return "fast";
  const modeFlagIndex = args.indexOf("--mode");
  if (modeFlagIndex >= 0 && args[modeFlagIndex + 1]) {
    return args[modeFlagIndex + 1].trim();
  }
  return "release";
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
BitFun Installer build script

Usage:
  node scripts/build-installer.cjs [options]

Options:
  --mode <fast|release>  Build mode (default: release)
  --fast                 Alias for --mode fast
  --skip-app-build       Skip building main BitFun app
  --dev                  Run installer with tauri dev instead of tauri build
                         and allow placeholder payload fallback
  --help, -h             Show this help
`);
  process.exit(0);
}

function getMainAppBuildCommand(mode) {
  if (mode === "fast") {
    return "npm run desktop:build:release-fast";
  }
  return "npm run desktop:build:exe";
}

function getInstallerBuildCommand(mode, devMode) {
  if (devMode) return "npm run tauri:dev";
  if (mode === "fast") return "npm run tauri:build:exe:fast";
  return "npm run tauri:build:exe";
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
  if (fileName === ".cargo-lock") return false;

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

function getCandidateAppExePaths(mode) {
  const preferredProfiles =
    mode === "fast"
      ? ["release-fast", "release", "debug"]
      : ["release", "release-fast", "debug"];

  const candidates = [];
  for (const profile of preferredProfiles) {
    candidates.push(
      path.join(
        BITFUN_ROOT,
        "src",
        "apps",
        "desktop",
        "target",
        profile,
        "bitfun-desktop.exe"
      ),
      path.join(
        BITFUN_ROOT,
        "src",
        "apps",
        "desktop",
        "target",
        profile,
        "BitFun.exe"
      ),
      path.join(BITFUN_ROOT, "target", profile, "bitfun-desktop.exe"),
      path.join(BITFUN_ROOT, "target", profile, "BitFun.exe")
    );
  }

  return candidates;
}

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

// Step 1: Build main BitFun app.
if (!skipAppBuild) {
  log("Step 1: Building BitFun main application...");
  run(getMainAppBuildCommand(buildMode), BITFUN_ROOT);
} else {
  log("Step 1: Skipped (--skip-app-build)");
}

// Step 2: Prepare payload.
log("Step 2: Preparing installer payload...");

const possiblePaths = getCandidateAppExePaths(buildMode);
let appExePath = null;
for (const p of possiblePaths) {
  if (fs.existsSync(p)) {
    appExePath = p;
    break;
  }
}

if (!appExePath && STRICT_PAYLOAD_VALIDATION) {
  error(
    "Could not find built BitFun executable for payload. Build the desktop app first or run with --dev for local debug."
  );
}

if (appExePath) {
  ensureCleanDir(PAYLOAD_DIR);

  const manifest = {
    generatedAt: new Date().toISOString(),
    mode: buildMode,
    sourceExe: appExePath,
    files: [],
  };

  const destExe = path.join(PAYLOAD_DIR, "BitFun.exe");
  writeFileWithManifest(appExePath, destExe, manifest, PAYLOAD_DIR);
  log(`Copied: ${appExePath} -> ${destExe}`);

  const exeSize = fs.statSync(destExe).size;
  if (STRICT_PAYLOAD_VALIDATION && exeSize < MIN_APP_EXE_BYTES) {
    error(
      `BitFun.exe in payload is unexpectedly small (${exeSize} bytes). Refusing to continue.`
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

  const runtimeDirs = ["resources", "locales", "swiftshader"];
  for (const dirName of runtimeDirs) {
    const srcDir = path.join(releaseDir, dirName);
    if (!fs.existsSync(srcDir) || !fs.statSync(srcDir).isDirectory()) {
      continue;
    }
    const destDir = path.join(PAYLOAD_DIR, dirName);
    copyDirRecursiveWithManifest(srcDir, destDir, manifest, PAYLOAD_DIR);
    log(`Copied runtime directory: ${dirName}`);
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
      "bitfun-installer.exe"
    )}`
  );
}
