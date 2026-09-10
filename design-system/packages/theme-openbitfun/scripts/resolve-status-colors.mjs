// Status authoring stays linked to its semantic anchors. Publish concrete colors
// because renderer payloads (including Mermaid and plugins) also consume them.
export function resolveStatusColors(tokens) {
  return Object.fromEntries(Object.entries(tokens).map(([name, token]) => {
    if (!name.startsWith("color.status.") || !token.value.startsWith("color-mix(")) {
      return [name, token];
    }

    const match = /^color-mix\(in srgb, (#[0-9a-f]{6}) (\d+)%, (#[0-9a-f]{6}|transparent)\)$/i.exec(token.value);
    if (!match || Number(match[2]) > 100) {
      throw new Error(`Unsupported status color mix for ${name}: ${token.value}`);
    }
    const channels = (hex) => [1, 3, 5].map((offset) => Number.parseInt(hex.slice(offset, offset + 2), 16));
    const foreground = channels(match[1]);
    const weight = Number(match[2]) / 100;
    const value = match[3] === "transparent"
      ? `rgba(${foreground.join(", ")}, ${weight})`
      : `#${channels(match[3]).map((channel, index) =>
        Math.round(foreground[index] * weight + channel * (1 - weight)).toString(16).padStart(2, "0"),
      ).join("")}`;

    return [name, { ...token, value }];
  }));
}
