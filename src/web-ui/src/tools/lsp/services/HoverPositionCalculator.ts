/**
 * Hover position calculator.
 *
 * Uses Monaco's `getScrolledVisiblePosition` to anchor to the visible cursor
 * position (scroll-aware), and then applies simple viewport-aware flipping and
 * clamping (similar to Floating UI / VSCode hover behavior).
 */

import * as monaco from 'monaco-editor';

export interface PositionResult {
  /** X in viewport coordinates. */
  x: number;
  /** Y in viewport coordinates. */
  y: number;
  /** Whether the panel is placed above the cursor. */
  isAbove: boolean;
  /** Whether the panel is placed to the left of the cursor. */
  isLeft: boolean;
  /** Max allowed width. */
  maxWidth: number;
  /** Max allowed height. */
  maxHeight: number;
}

export interface PositionCalculatorOptions {
  /** Estimated hover width (default: 480px). */
  estimatedWidth?: number;
  /** Estimated hover height (default: 200px). */
  estimatedHeight?: number;
  /** Vertical offset from cursor (default: 10px). */
  verticalOffset?: number;
  /** Horizontal offset from cursor (default: 0px). */
  horizontalOffset?: number;
  /** Margin from viewport edges (default: 20px). */
  margin?: number;
  /** Max width (default: 600px). */
  maxWidth?: number;
  /** Max height (default: 400px). */
  maxHeight?: number;
  /** Prefer placing above (default: false). */
  preferAbove?: boolean;
}

/**
 * Hover Position Calculator
 */
export class HoverPositionCalculator {
  private editor: monaco.editor.IStandaloneCodeEditor;
  private options: Required<PositionCalculatorOptions>;

  constructor(
    editor: monaco.editor.IStandaloneCodeEditor,
    options: PositionCalculatorOptions = {}
  ) {
    this.editor = editor;
    this.options = {
      estimatedWidth: options.estimatedWidth ?? 480,
      estimatedHeight: options.estimatedHeight ?? 200,
      verticalOffset: options.verticalOffset ?? 10,
      horizontalOffset: options.horizontalOffset ?? 0,
      margin: options.margin ?? 20,
      maxWidth: options.maxWidth ?? 600,
      maxHeight: options.maxHeight ?? 400,
      preferAbove: options.preferAbove ?? false
    };
  }

  /**
   * Compute best position for an overlay anchored at a Monaco position.
   */
  public calculate(
    position: monaco.Position,
    contentWidth?: number,
    contentHeight?: number
  ): PositionResult | null {
    const model = this.editor.getModel();
    if (!model) return null;

    const width = contentWidth ?? this.options.estimatedWidth;
    const height = contentHeight ?? this.options.estimatedHeight;

    const editorDom = this.editor.getDomNode();
    if (!editorDom) return null;

    const editorRect = editorDom.getBoundingClientRect();

    const visiblePosition = this.editor.getScrolledVisiblePosition(position);
    if (!visiblePosition) return null;

    const cursorX = editorRect.left + visiblePosition.left;
    const cursorY = editorRect.top + visiblePosition.top;

    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;

    const lineHeight = this.editor.getOption(monaco.editor.EditorOption.lineHeight);

    const spaceBelow = viewportHeight - (cursorY + lineHeight);
    const spaceAbove = cursorY;
    const spaceRight = viewportWidth - cursorX;
    const spaceLeft = cursorX;

    let isAbove = this.options.preferAbove;
    let isLeft = false;

    if (!this.options.preferAbove) {
      if (spaceBelow < height + this.options.margin) {
        if (spaceAbove > height + this.options.margin) {
          isAbove = true;
        }
      }
    } else {
      if (spaceAbove < height + this.options.margin) {
        if (spaceBelow > height + this.options.margin) {
          isAbove = false;
        }
      }
    }

    if (spaceRight < width + this.options.margin) {
      if (spaceLeft > width + this.options.margin) {
        isLeft = true;
      }
    }

    let x = cursorX + this.options.horizontalOffset;
    let y: number;

    if (isAbove) {
      y = cursorY - height - this.options.verticalOffset;
    } else {
      y = cursorY + lineHeight + this.options.verticalOffset;
    }

    if (isLeft) {
      x = cursorX - width - this.options.horizontalOffset;
    }

    if (x < this.options.margin) {
      x = this.options.margin;
    } else if (x + width > viewportWidth - this.options.margin) {
      x = viewportWidth - width - this.options.margin;
    }

    if (y < this.options.margin) {
      y = this.options.margin;
    } else if (y + height > viewportHeight - this.options.margin) {
      y = viewportHeight - height - this.options.margin;
    }

    const maxWidth = Math.min(
      this.options.maxWidth,
      viewportWidth - 2 * this.options.margin
    );

    const maxHeight = Math.min(
      this.options.maxHeight,
      isAbove ? spaceAbove - this.options.margin : spaceBelow - this.options.margin
    );

    return {
      x,
      y,
      isAbove,
      isLeft,
      maxWidth,
      maxHeight
    };
  }

  /**
   * Compute a Monaco ContentWidget position.
   */
  public calculateForContentWidget(
    position: monaco.Position
  ): monaco.editor.IContentWidgetPosition {
    return {
      position: position,
      preference: [
        monaco.editor.ContentWidgetPositionPreference.BELOW,
        monaco.editor.ContentWidgetPositionPreference.ABOVE
      ]
    };
  }

  /**
   * Update options.
   */
  public updateOptions(options: Partial<PositionCalculatorOptions>): void {
    Object.assign(this.options, options);
  }
}

/** Utility: detect whether an element is out of the viewport. */
export function isOutOfViewport(element: HTMLElement): {
  top: boolean;
  right: boolean;
  bottom: boolean;
  left: boolean;
} {
  const rect = element.getBoundingClientRect();
  
  return {
    top: rect.top < 0,
    right: rect.right > window.innerWidth,
    bottom: rect.bottom > window.innerHeight,
    left: rect.left < 0
  };
}

/** Utility: adjust an element position to keep it inside the viewport. */
export function adjustToViewport(element: HTMLElement, margin: number = 20): void {
  const rect = element.getBoundingClientRect();
  const style = element.style;
  
  let left = parseInt(style.left) || rect.left;
  let top = parseInt(style.top) || rect.top;

  if (rect.right > window.innerWidth - margin) {
    left = window.innerWidth - rect.width - margin;
  }

  if (rect.left < margin) {
    left = margin;
  }

  if (rect.bottom > window.innerHeight - margin) {
    top = window.innerHeight - rect.height - margin;
  }

  if (rect.top < margin) {
    top = margin;
  }

  style.left = `${left}px`;
  style.top = `${top}px`;
}


