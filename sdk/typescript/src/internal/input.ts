import type { Input } from "../types.js";

export function normalizeInput(input: Input): {
  prompt: string;
  images: string[];
} {
  if (typeof input === "string") {
    return { prompt: input, images: [] };
  }

  const promptParts: string[] = [];
  const images: string[] = [];
  for (const item of input) {
    if (item.type === "text") {
      promptParts.push(item.text);
    } else if (item.type === "local_image") {
      images.push(item.path);
    }
  }
  return { prompt: promptParts.join("\n\n"), images };
}
