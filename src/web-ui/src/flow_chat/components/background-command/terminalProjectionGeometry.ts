/** Match the DOM renderer's canvas rounding without feeding its rounded
 * canvas-height/row-count estimate back into the next row-count calculation. */
export function fitProjectionRows(height: number, deviceCellHeight: number, dpr: number): number | undefined {
  if (![height, deviceCellHeight, dpr].every(Number.isFinite)
    || height <= 0 || deviceCellHeight <= 0 || dpr <= 0) return undefined;
  const rowHeight = deviceCellHeight / dpr;
  let rows = Math.max(1, Math.floor(height / rowHeight));
  if (Math.round((rows + 1) * rowHeight) <= height) rows++;
  if (rows > 1 && Math.round(rows * rowHeight) > height) rows--;
  return rows;
}
