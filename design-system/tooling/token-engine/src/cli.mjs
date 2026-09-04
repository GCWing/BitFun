#!/usr/bin/env node

import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import {
  createTokenArtifacts,
  mergeTokenDocuments,
  renderCss,
  resolveTokens,
} from "./index.mjs";

function readArguments(argv) {
  const options = {
    inputs: [],
    layer: "openbitfun.tokens",
    prefix: "openbitfun",
    selector: ":root",
  };

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];
    if (argument === "--input" && value) {
      options.inputs.push(value);
      index += 1;
    } else if (argument === "--out-dir" && value) {
      options.outDir = value;
      index += 1;
    } else if (argument === "--selector" && value) {
      options.selector = value;
      index += 1;
    } else if (argument === "--layer" && value) {
      options.layer = value;
      index += 1;
    } else if (argument === "--prefix" && value) {
      options.prefix = value;
      index += 1;
    } else {
      throw new Error(`Unknown or incomplete argument "${argument}".`);
    }
  }

  if (options.inputs.length === 0 || !options.outDir) {
    throw new Error("Usage: token-engine --input <file> [--input <file>] --out-dir <directory>.");
  }

  return options;
}

const options = readArguments(process.argv.slice(2));
const documents = await Promise.all(
  options.inputs.map(async (input) => JSON.parse(await readFile(input, "utf8"))),
);
const tokens = resolveTokens(mergeTokenDocuments(...documents));
const artifacts = createTokenArtifacts(tokens, { prefix: options.prefix });
const outputDirectory = path.resolve(options.outDir);

await mkdir(outputDirectory, { recursive: true });
await Promise.all([
  writeFile(
    path.join(outputDirectory, "tokens.css"),
    renderCss(tokens, {
      layer: options.layer,
      prefix: options.prefix,
      selector: options.selector,
    }),
  ),
  writeFile(path.join(outputDirectory, "tokens.json"), artifacts.json),
  writeFile(path.join(outputDirectory, "index.js"), artifacts.javascript),
  writeFile(path.join(outputDirectory, "index.d.ts"), artifacts.typescript),
]);
