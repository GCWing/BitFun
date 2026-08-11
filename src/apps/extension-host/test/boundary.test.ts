import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, readdir, rm } from "node:fs/promises"
import path from "node:path"
import { tmpdir } from "node:os"

const extensionHostDirectory = path.resolve(import.meta.dir, "..")

describe("standalone package boundary", () => {
  test("imports only public OpenCode packages", async () => {
    const packageJson = await Bun.file(path.join(extensionHostDirectory, "package.json")).json()
    const files = [
      ...new Bun.Glob("src/**/*.ts").scanSync({ cwd: extensionHostDirectory }),
      ...new Bun.Glob("script/**/*.ts").scanSync({ cwd: extensionHostDirectory }),
    ]
    const resolvedImports = (
      await Promise.all(
        files.map(async (file) => {
          const source = await Bun.file(path.join(extensionHostDirectory, file)).text()
          return Array.from(
            source.matchAll(/(?:from\s+|import\s*\(\s*|import\s+)["']([^"']+)["']/g),
            (match) => match[1],
          ).flatMap((specifier) => (specifier ? [{ file, specifier }] : []))
        }),
      )
    ).flat()
    const invalidOpenCodeImports = resolvedImports.filter(
      (item) =>
        item.specifier.startsWith("@opencode-ai/") &&
        item.specifier !== "@opencode-ai/plugin" &&
        item.specifier !== "@opencode-ai/sdk" &&
        !item.specifier.startsWith("@opencode-ai/sdk/"),
    )
    const escapedRelativeImports = resolvedImports.filter(
      (item) =>
        item.specifier.startsWith(".") &&
        !path
          .resolve(path.dirname(path.join(extensionHostDirectory, item.file)), item.specifier)
          .startsWith(`${extensionHostDirectory}${path.sep}`),
    )

    expect(invalidOpenCodeImports).toEqual([])
    expect(escapedRelativeImports).toEqual([])
    expect(packageJson.dependencies["@opencode-ai/plugin"]).toBe("1.17.18")
    expect(packageJson.dependencies["@opencode-ai/sdk"]).toBe("1.17.18")
    expect(Object.values({ ...packageJson.dependencies, ...packageJson.devDependencies })).not.toContain(
      expect.stringContaining("workspace:"),
    )
  })

  test("packs, installs, and builds outside the repository", async () => {
    const root = await mkdtemp(path.join(tmpdir(), "opencode-extension-host-pack-"))
    const archiveDirectory = path.join(root, "archive")
    const extractedDirectory = path.join(root, "extracted")
    await Promise.all([mkdir(archiveDirectory), mkdir(extractedDirectory)])

    try {
      await command(
        [process.execPath, "pm", "pack", "--destination", archiveDirectory, "--ignore-scripts", "--quiet"],
        extensionHostDirectory,
      )
      const archive = path.join(
        archiveDirectory,
        (await readdir(archiveDirectory)).find((file) => file.endsWith(".tgz")) ?? "missing.tgz",
      )
      expect(await Bun.file(archive).exists()).toBe(true)
      await command(["tar", "-xzf", archive, "-C", extractedDirectory], extensionHostDirectory)

      const standaloneDirectory = path.join(extractedDirectory, "package")
      expect(await Bun.file(path.join(standaloneDirectory, "protocol.schema.json")).exists()).toBe(true)
      await command([process.execPath, "install", "--ignore-scripts"], standaloneDirectory)
      await command([process.execPath, "run", "build"], standaloneDirectory)
      expect(await Bun.file(path.join(standaloneDirectory, "dist", "extension-host.js")).exists()).toBe(true)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  }, 30_000)
})

async function command(cmd: string[], cwd: string) {
  const child = Bun.spawn({ cmd, cwd, stdin: "ignore", stdout: "pipe", stderr: "pipe" })
  const [code, stdout, stderr] = await Promise.all([
    child.exited,
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
  ])
  if (code === 0) return stdout
  throw new Error(`${cmd.join(" ")} failed with status ${code}\n${stderr || stdout}`)
}
