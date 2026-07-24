export const MOUSE_GLOW_STORAGE_KEY = 'bitfun:appearance:mouse-glow-enabled';
export const DEFAULT_MOUSE_GLOW_ENABLED = true;

const MOUSE_GLOW_OVERLAY_ID = 'bitfun-mouse-glow-overlay';
const AUTOMATIC_SURFACE_CLASS_PATTERN =
  /(?:^|[-_])(card|panel|dialog|modal|surface|frame)(?:$|[-_])/i;
const EXCLUDED_SURFACE_TAGS = new Set([
  'CANVAS',
  'HTML',
  'IFRAME',
  'INPUT',
  'LABEL',
  'OPTION',
  'SELECT',
  'SVG',
  'TEXTAREA',
  'VIDEO',
]);

type MouseGlowListener = () => void;

export class MouseGlowService {
  private enabled = DEFAULT_MOUSE_GLOW_ENABLED;
  private initialized = false;
  private frameId: number | null = null;
  private pointerX = 0;
  private pointerY = 0;
  private pendingElements: HTMLElement[] | null = null;
  private pendingSurface: HTMLElement | null = null;
  private activeSurface: HTMLElement | null = null;
  private overlay: HTMLDivElement | null = null;
  private reducedMotionQuery: MediaQueryList | null = null;
  private readonly listeners = new Set<MouseGlowListener>();

  initialize = (): void => {
    if (this.initialized || typeof window === 'undefined' || typeof document === 'undefined') {
      return;
    }

    this.initialized = true;
    this.enabled = this.readStoredPreference();
    this.reducedMotionQuery = window.matchMedia?.('(prefers-reduced-motion: reduce)') ?? null;
    this.overlay = this.ensureOverlay();

    this.applyEnabledState();
    window.addEventListener('pointermove', this.handlePointerMove, { passive: true });
    window.addEventListener('pointerout', this.handlePointerOut, { passive: true });
    window.addEventListener('resize', this.handleViewportChange, { passive: true });
    window.addEventListener('scroll', this.handleViewportChange, { capture: true, passive: true });
    window.addEventListener('storage', this.handleStorage);
    this.reducedMotionQuery?.addEventListener?.('change', this.handleReducedMotionChange);
  };

  dispose = (): void => {
    if (!this.initialized || typeof window === 'undefined' || typeof document === 'undefined') {
      return;
    }

    window.removeEventListener('pointermove', this.handlePointerMove);
    window.removeEventListener('pointerout', this.handlePointerOut);
    window.removeEventListener('resize', this.handleViewportChange);
    window.removeEventListener('scroll', this.handleViewportChange, true);
    window.removeEventListener('storage', this.handleStorage);
    this.reducedMotionQuery?.removeEventListener?.('change', this.handleReducedMotionChange);
    this.resetPointerState();
    this.overlay?.remove();
    this.overlay = null;
    document.documentElement.removeAttribute('data-mouse-glow-enabled');
    this.reducedMotionQuery = null;
    this.initialized = false;
    this.enabled = DEFAULT_MOUSE_GLOW_ENABLED;
  };

  getEnabled = (): boolean => this.enabled;

  subscribe = (listener: MouseGlowListener): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  setEnabled = (enabled: boolean): void => {
    this.initialize();
    if (this.enabled === enabled) {
      return;
    }

    this.enabled = enabled;
    this.applyEnabledState();
    this.writeStoredPreference(enabled);
    this.emit();
  };

  private readonly handlePointerMove = (event: PointerEvent): void => {
    if (
      !this.enabled
      || this.reducedMotionQuery?.matches
      || event.pointerType === 'touch'
    ) {
      return;
    }

    this.pointerX = event.clientX;
    this.pointerY = event.clientY;
    const path = event.composedPath?.() ?? [];
    const elements = path.filter(
      (item): item is HTMLElement => item instanceof HTMLElement
    );
    if (this.activeSurface && !elements.includes(this.activeSurface)) {
      this.deactivateSurface();
    }
    this.pendingElements = elements;
    this.pendingSurface = null;
    this.scheduleFrame();
  };

  private readonly handlePointerOut = (event: PointerEvent): void => {
    const relatedTarget = event.relatedTarget;
    if (relatedTarget === null || relatedTarget instanceof HTMLIFrameElement) {
      this.resetPointerState();
      return;
    }

    if (
      !(relatedTarget instanceof Node)
      || (this.activeSurface && !this.activeSurface.contains(relatedTarget))
    ) {
      this.deactivateSurface();
    }
  };

  private readonly handleViewportChange = (): void => {
    if (!this.enabled || this.reducedMotionQuery?.matches) {
      return;
    }

    const pointerTarget = document.elementFromPoint?.(this.pointerX, this.pointerY) ?? null;
    this.pendingElements = pointerTarget
      ? this.getAncestorElements(pointerTarget)
      : null;
    this.pendingSurface = pointerTarget ? null : this.activeSurface;
    this.scheduleFrame();
  };

  private readonly handleStorage = (event: StorageEvent): void => {
    if (event.key !== MOUSE_GLOW_STORAGE_KEY) {
      return;
    }

    const enabled = this.parseStoredPreference(event.newValue);
    if (enabled === this.enabled) {
      return;
    }

    this.enabled = enabled;
    this.applyEnabledState();
    this.emit();
  };

  private readonly handleReducedMotionChange = (): void => {
    if (this.reducedMotionQuery?.matches) {
      this.resetPointerState();
    }
  };

  private applyEnabledState(): void {
    document.documentElement.toggleAttribute('data-mouse-glow-enabled', this.enabled);
    if (!this.enabled) {
      this.resetPointerState();
    }
  }

  private resetPointerState(): void {
    if (this.frameId !== null) {
      window.cancelAnimationFrame(this.frameId);
      this.frameId = null;
    }
    this.pendingElements = null;
    this.pendingSurface = null;
    this.deactivateSurface();
  }

  private deactivateSurface(): void {
    this.pendingElements = null;
    this.pendingSurface = null;
    this.activeSurface = null;
    this.overlay?.removeAttribute('data-active');
  }

  private scheduleFrame(): void {
    if (this.frameId !== null) {
      return;
    }

    this.frameId = window.requestAnimationFrame(() => {
      this.frameId = null;
      const surface = this.pendingElements
        ? this.findSurface(this.pendingElements)
        : this.pendingSurface;
      this.pendingElements = null;
      this.updateOverlay(surface);
    });
  }

  private updateOverlay(surface: HTMLElement | null): void {
    const overlay = this.overlay;
    if (!overlay || !surface?.isConnected) {
      this.activeSurface = null;
      overlay?.removeAttribute('data-active');
      return;
    }

    const rect = surface.getBoundingClientRect();
    if (
      rect.width <= 0
      || rect.height <= 0
      || rect.bottom < 0
      || rect.right < 0
      || rect.top > window.innerHeight
      || rect.left > window.innerWidth
    ) {
      this.activeSurface = null;
      overlay.removeAttribute('data-active');
      return;
    }

    const style = window.getComputedStyle(surface);
    this.activeSurface = surface;
    overlay.style.width = `${rect.width}px`;
    overlay.style.height = `${rect.height}px`;
    overlay.style.borderRadius = style.borderRadius;
    overlay.style.transform = `translate3d(${rect.left}px, ${rect.top}px, 0)`;
    overlay.style.setProperty('--mouse-glow-local-x', `${this.pointerX - rect.left}px`);
    overlay.style.setProperty('--mouse-glow-local-y', `${this.pointerY - rect.top}px`);
    overlay.setAttribute('data-active', '');
  }

  private ensureOverlay(): HTMLDivElement {
    const existing = document.getElementById(MOUSE_GLOW_OVERLAY_ID);
    if (existing instanceof HTMLDivElement) {
      return existing;
    }

    const overlay = document.createElement('div');
    overlay.id = MOUSE_GLOW_OVERLAY_ID;
    overlay.className = 'bitfun-mouse-glow-overlay';
    overlay.setAttribute('aria-hidden', 'true');
    document.body.appendChild(overlay);
    return overlay;
  }

  private getAncestorElements(element: Element): HTMLElement[] {
    const elements: HTMLElement[] = [];
    let current: Element | null = element;
    while (current) {
      if (current instanceof HTMLElement) {
        elements.push(current);
      }
      current = current.parentElement;
    }
    return elements;
  }

  private findSurface(elements: HTMLElement[]): HTMLElement | null {
    if (elements.some(element => element.hasAttribute('data-mouse-glow-ignore'))) {
      return null;
    }

    const explicitSurface = elements.find(element =>
      element.hasAttribute('data-mouse-glow-surface')
    );
    if (explicitSurface) {
      return explicitSurface;
    }

    const semanticSurface = elements.find(element =>
      this.hasSemanticSurfaceClass(element) && this.isAutomaticSurface(element, true)
    );
    if (semanticSurface) {
      return semanticSurface;
    }

    return elements.find(element => this.isAutomaticSurface(element, false)) ?? null;
  }

  private isAutomaticSurface(element: HTMLElement, hasSemanticClass: boolean): boolean {
    if (
      element === document.body
      || element === this.overlay
      || EXCLUDED_SURFACE_TAGS.has(element.tagName)
      || element.getAttribute('aria-hidden') === 'true'
    ) {
      return false;
    }

    if ((element.tagName === 'A' || element.tagName === 'BUTTON') && !hasSemanticClass) {
      return false;
    }

    const rect = element.getBoundingClientRect();
    if (rect.width < 64 || rect.height < 32) {
      return false;
    }

    const style = window.getComputedStyle(element);
    if (
      style.display === 'none'
      || style.display === 'contents'
      || style.visibility === 'hidden'
      || (style.opacity !== '' && Number(style.opacity) === 0)
    ) {
      return false;
    }

    const visibleBorderSides = [
      [style.borderTopWidth, style.borderTopStyle, style.borderTopColor],
      [style.borderRightWidth, style.borderRightStyle, style.borderRightColor],
      [style.borderBottomWidth, style.borderBottomStyle, style.borderBottomColor],
      [style.borderLeftWidth, style.borderLeftStyle, style.borderLeftColor],
    ].filter(([width, borderStyle, color]) =>
      parseFloat(width) > 0
      && borderStyle !== 'none'
      && !this.isTransparentColor(color)
    ).length;

    if (visibleBorderSides >= 2) {
      return true;
    }

    const hasRoundedCorners = parseFloat(style.borderRadius) > 0;
    const hasBackground =
      style.backgroundImage !== 'none' || !this.isTransparentColor(style.backgroundColor);

    return hasSemanticClass && hasRoundedCorners && hasBackground;
  }

  private hasSemanticSurfaceClass(element: HTMLElement): boolean {
    return Array.from(element.classList).some(className =>
      !className.includes('__') && AUTOMATIC_SURFACE_CLASS_PATTERN.test(className)
    );
  }

  private isTransparentColor(color: string): boolean {
    const normalizedColor = color.replace(/\s+/g, '');
    return (
      normalizedColor === 'transparent'
      || normalizedColor === ''
      || /^rgba\([^,]+,[^,]+,[^,]+,0(?:\.0+)?\)$/i.test(normalizedColor)
      || /^rgba?\([^)]*\/0(?:\.0+)?%?\)$/i.test(normalizedColor)
    );
  }

  private readStoredPreference(): boolean {
    try {
      return this.parseStoredPreference(window.localStorage.getItem(MOUSE_GLOW_STORAGE_KEY));
    } catch {
      return DEFAULT_MOUSE_GLOW_ENABLED;
    }
  }

  private parseStoredPreference(value: string | null): boolean {
    if (value === null) {
      return DEFAULT_MOUSE_GLOW_ENABLED;
    }
    return value !== 'false';
  }

  private writeStoredPreference(enabled: boolean): void {
    try {
      window.localStorage.setItem(MOUSE_GLOW_STORAGE_KEY, String(enabled));
    } catch {
      // Keep the in-memory preference when storage is unavailable.
    }
  }

  private emit(): void {
    this.listeners.forEach(listener => listener());
  }
}

export const mouseGlowService = new MouseGlowService();
