import { Terminal, type IBufferCell, type IBufferLine } from '@xterm/headless';
import type {
  BackgroundCommandOutputMetadata,
  ReadBackgroundCommandOutputResponse,
} from '@/infrastructure/api/service-api/AgentAPI';

export interface TerminalProjection {
  text: string;
  ansi: string;
  /** Coordinate-addressed screens retain source columns; logs may reflow. */
  screenCols?: number;
  incomplete: boolean;
  unknownGeometry: boolean;
}

export function sourceTerminalSize(metadata: BackgroundCommandOutputMetadata) {
  if (!metadata.tty) return { cols: 80, rows: 24 };
  // Both legacy local ExecCommand and SSH ExecCommand use a fixed 80x24 PTY.
  const size = metadata.terminalSize === undefined
    ? { cols: 80, rows: 24 }
    : metadata.terminalSize;
  return size && Number.isInteger(size.cols) && Number.isInteger(size.rows)
    && size.cols >= 2 && size.cols <= 4096 && size.rows >= 1 && size.rows <= 4096
    ? size : null;
}

function cellStyle(cell: IBufferCell): string {
  const codes: number[] = [0];
  if (cell.isBold()) codes.push(1);
  if (cell.isDim()) codes.push(2);
  if (cell.isItalic()) codes.push(3);
  if (cell.isUnderline()) codes.push(4);
  if (cell.isInverse()) codes.push(7);
  if (cell.isInvisible()) codes.push(8);
  if (cell.isStrikethrough()) codes.push(9);
  for (const foreground of [true, false]) {
    const value = foreground ? cell.getFgColor() : cell.getBgColor();
    const rgb = foreground ? cell.isFgRGB() : cell.isBgRGB();
    const palette = foreground ? cell.isFgPalette() : cell.isBgPalette();
    if (rgb) codes.push(foreground ? 38 : 48, 2, value >>> 16 & 255, value >>> 8 & 255, value & 255);
    else if (palette) codes.push(foreground ? 38 : 48, 5, value);
  }
  return `\x1b[${codes.join(';')}m`;
}

function visibleCellCount(line: IBufferLine): number {
  let end = line.length;
  while (end > 0) {
    const cell = line.getCell(end - 1)!;
    if ((cell.getChars() && cell.getChars() !== ' ') || cell.getWidth() === 0
      || !cell.isBgDefault() || cell.isInverse() || cell.isUnderline() || cell.isStrikethrough()) break;
    end--;
  }
  return end;
}

/** One ordered output stream. Keep the parser alive across response/chunk boundaries. */
export class BackgroundTerminalReplay {
  private terminal: Terminal | null = null;
  private geometryKey = '';
  private cursor: number | undefined;
  private incomplete = false;
  private coordinateScreen = false;
  private raw = '';
  private disposed = false;

  get rawOutput() { return this.raw; }

  async accept(response: ReadBackgroundCommandOutputResponse): Promise<TerminalProjection> {
    if (this.disposed) throw new Error('Terminal replay is disposed');
    const size = sourceTerminalSize(response.metadata);
    const key = `${response.metadata.tty}:${size?.cols}:${size?.rows}`;
    const reset = response.reset || response.snapshot != null || this.cursor === undefined;
    if (!reset && key === this.geometryKey && response.cursor <= this.cursor!) return this.project(!size);
    if (reset || key !== this.geometryKey) {
      this.terminal?.dispose();
      this.terminal = null;
      this.coordinateScreen = false;
      this.incomplete = response.metadata.truncatedFromStart || (!reset && key !== this.geometryKey);
      this.raw = '';
      this.geometryKey = key;
      if (size) {
        const terminal = new Terminal({ ...size, scrollback: 5000, allowProposedApi: true,
          convertEol: !response.metadata.tty });
        this.terminal = terminal;
        // Edge-column rewrites are ConPTY's delayed-wrap correction, not a grid.
        // Other explicit cursor addressing keeps the screen's column geometry.
        for (const final of ['H', 'f']) terminal.parser.registerCsiHandler({ final }, params => {
          const col = typeof params[1] === 'number' ? params[1] || 1 : 1;
          const buffer = terminal.buffer.active;
          if (buffer.type === 'normal' && col !== terminal.cols
            && (buffer.baseY > 0 || buffer.cursorY > 0
              || buffer.getLine(0)?.translateToString(true))) this.coordinateScreen = true;
          return false;
        });
        for (const final of ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'd']) {
          terminal.parser.registerCsiHandler({ final }, () => {
            if (terminal.buffer.active.type === 'normal') this.coordinateScreen = true;
            return false;
          });
        }
      }
    }
    const incoming = response.snapshot ?? response.chunks.join('');
    this.raw += incoming;
    // Bound only the diagnostic copy. Never discard the live parser's state.
    const limit = response.metadata.retainedLimitBytes;
    if (limit > 0 && this.raw.length > limit) {
      this.raw = this.raw.slice(-limit);
      if (/^[\uDC00-\uDFFF]/.test(this.raw)) this.raw = this.raw.slice(1);
    }
    if (this.terminal && incoming && !this.disposed) {
      await new Promise<void>(resolve => this.terminal!.write(incoming, resolve));
    }
    this.cursor = response.cursor;
    return this.project(!size);
  }

  private project(unknownGeometry: boolean): TerminalProjection {
    const result: TerminalProjection = { text: '', ansi: '', incomplete: this.incomplete, unknownGeometry };
    if (!this.terminal || this.disposed) return result;
    const terminal = this.terminal;
    const buffer = terminal.buffer.active;
    const grid = buffer.type === 'alternate' || this.coordinateScreen;
    if (grid) result.screenCols = terminal.cols;
    let end = buffer.length;
    while (end > 0 && visibleCellCount(buffer.getLine(end - 1)!) === 0) end--;
    let lastStyle = '';
    for (let row = 0; row < end; row++) {
      const line = buffer.getLine(row)!;
      if (row > 0 && (grid || !line.isWrapped)) {
        result.text += '\n';
        result.ansi += '\r\n';
      }
      const continues = !grid && buffer.getLine(row + 1)?.isWrapped;
      let lastCell = continues ? terminal.cols : visibleCellCount(line);
      // A wide glyph can wrap before the last column, leaving an unwritten
      // padding cell. It is not a space in the logical log line.
      if (continues) {
        while (lastCell > 0) {
          const cell = line.getCell(lastCell - 1)!;
          if (cell.getChars() || cell.getWidth() !== 1) break;
          lastCell--;
        }
      }
      for (let col = 0; col < lastCell; col++) {
        const cell = line.getCell(col)!;
        if (cell.getWidth() === 0) continue;
        const style = cellStyle(cell);
        if (style !== lastStyle) { result.ansi += style; lastStyle = style; }
        const chars = cell.getChars() || ' ';
        result.text += chars;
        result.ansi += chars;
      }
    }
    if (result.ansi) result.ansi += '\x1b[0m';
    return result;
  }

  dispose() {
    this.disposed = true;
    this.terminal?.dispose();
    this.terminal = null;
  }
}
