import sharp from "sharp";

const input = process.argv[2] ?? "png/logo-dark.png";
const output = process.argv[3] ?? "src/apps/desktop/icons/app-icon-rounded-source.png";
const logoOutput = process.argv[4] ?? "src/apps/desktop/icons/Logo-ICON.png";
const hicolorRoot = "src/apps/desktop/icons/hicolor";

const size = 1024;
const radius = Math.round(size * 0.22);
const roundedRect = Buffer.from(
  `<svg width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
    <rect x="0" y="0" width="${size}" height="${size}" rx="${radius}" ry="${radius}" fill="#fff"/>
  </svg>`,
);

const roundedIcon = await sharp(input)
  .resize(size, size, { fit: "cover", position: "center" })
  .ensureAlpha()
  .composite([{ input: roundedRect, blend: "dest-in" }])
  .png()
  .toBuffer();

await sharp(roundedIcon).toFile(output);
await sharp(roundedIcon).resize(512, 512).png().toFile(logoOutput);

for (const iconSize of [16, 32, 48, 64, 96, 128, 256, 512]) {
  await sharp(roundedIcon)
    .resize(iconSize, iconSize)
    .png()
    .toFile(`${hicolorRoot}/${iconSize}x${iconSize}/apps/bitfun-desktop.png`);
}
