export const MOUSE_GLOW_STORAGE_KEY = 'bitfun:appearance:mouse-glow-enabled';
export const DEFAULT_MOUSE_GLOW_ENABLED = true;

const MOUSE_GLOW_OVERLAY_ID = 'bitfun-mouse-glow-overlay';
// Detection is computed-style first. These class patterns are fallbacks only for
// borderless visual surfaces and floating-layer placement, not an allowlist.
const AUTOMATIC_SURFACE_CLASS_PATTERN =
  /(?:^|[-_])(card|panel|dialog|modal|surface|frame)(?:$|[-_])/i;
const DIVIDER_CLASS_PATTERN =
  /(?:^|[-_])(divider|separator|splitter|rule)(?:$|[-_])/i;
const FLOATING_LAYER_CLASS_PATTERN =
  /(?:^|[-_])(dialog|dropdown|flyout|menu|modal|overlay|popover|popup)(?:$|[-_])/i;
const FLOATING_LAYER_ROLES = new Set(['dialog', 'listbox', 'menu', 'tree']);
const EXCLUDED_SURFACE_TAGS = new Set([
  'HTML',
  'IFRAME',
  'OPTION',
]);

type MouseGlowListener = () => void;
type BorderSideName = 'top' | 'right' | 'bottom' | 'left';

interface VisibleBorderSide {
  name: BorderSideName;
  width: number;
}

interface OverlayGeometry {
  height: number;
  left: number;
  top: number;
  width: number;
}

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
    if (this.overlay) {
      this.overlay.hidden = true;
      this.overlay.removeAttribute('data-active');
      this.overlay.removeAttribute('data-divider');
    }
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
      this.deactivateSurface();
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
      this.deactivateSurface();
      return;
    }

    const style = window.getComputedStyle(surface);
    const dividerGeometry = this.getDividerGeometry(surface, rect, style);
    const surfaceGeometry = dividerGeometry ?? rect;
    const overlayHost = this.findOverlayHost(surface);
    if (overlay.parentElement !== overlayHost) {
      overlayHost.appendChild(overlay);
    }
    const geometry = this.getVisibleOverlayGeometry(
      surfaceGeometry,
      surface,
      overlayHost,
      style,
    );
    const overlayPosition = this.getOverlayPosition(geometry, overlayHost);
    this.activeSurface = surface;
    overlay.toggleAttribute('data-divider', dividerGeometry !== null);
    overlay.toggleAttribute('data-local-position', overlayPosition.isLocal);
    overlay.style.width = `${geometry.width}px`;
    overlay.style.height = `${geometry.height}px`;
    overlay.style.borderRadius = dividerGeometry ? '0px' : style.borderRadius;
    overlay.style.transform =
      `translate3d(${overlayPosition.left}px, ${overlayPosition.top}px, 0)`;
    overlay.style.setProperty('--mouse-glow-local-x', `${this.pointerX - geometry.left}px`);
    overlay.style.setProperty('--mouse-glow-local-y', `${this.pointerY - geometry.top}px`);
    overlay.hidden = false;
    overlay.setAttribute('data-active', '');
  }

  private getVisibleOverlayGeometry(
    geometry: OverlayGeometry,
    surface: HTMLElement,
    host: HTMLElement,
    style: CSSStyleDeclaration,
  ): OverlayGeometry {
    if (host !== surface || !this.isFloatingLayer(surface)) {
      return geometry;
    }

    const borderTopWidth = parseFloat(style.borderTopWidth) || 0;
    const borderRightWidth = parseFloat(style.borderRightWidth) || 0;
    const borderBottomWidth = parseFloat(style.borderBottomWidth) || 0;
    const borderLeftWidth = parseFloat(style.borderLeftWidth) || 0;
    return {
      height: Math.max(geometry.height - borderTopWidth - borderBottomWidth, 1),
      left: geometry.left + borderLeftWidth,
      top: geometry.top + borderTopWidth,
      width: Math.max(geometry.width - borderLeftWidth - borderRightWidth, 1),
    };
  }

  private getOverlayPosition(
    geometry: OverlayGeometry,
    host: HTMLElement,
  ): { isLocal: boolean; left: number; top: number } {
    if (host === document.body) {
      return { isLocal: false, left: geometry.left, top: geometry.top };
    }

    const hostStyle = window.getComputedStyle(host);
    if (hostStyle.position === 'static') {
      return { isLocal: false, left: geometry.left, top: geometry.top };
    }

    const hostRect = host.getBoundingClientRect();
    const borderLeftWidth = parseFloat(hostStyle.borderLeftWidth) || 0;
    const borderTopWidth = parseFloat(hostStyle.borderTopWidth) || 0;
    return {
      isLocal: true,
      left: geometry.left - hostRect.left - borderLeftWidth + host.scrollLeft,
      top: geometry.top - hostRect.top - borderTopWidth + host.scrollTop,
    };
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
    overlay.hidden = true;
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
    if (
      elements.some(element =>
        element.hasAttribute('data-mouse-glow-ignore')
        || this.isResizeInteractionElement(element)
      )
    ) {
      return null;
    }

    // The event path is ordered from the deepest target outward, so the first
    // matching visual boundary is also the most specific one under the pointer.
    return elements.find(element => {
      if (this.isDividerSurface(element)) {
        return true;
      }
      if (element.hasAttribute('data-mouse-glow-surface')) {
        return true;
      }
      const hasSemanticClass = this.hasSemanticSurfaceClass(element);
      return this.isAutomaticSurface(element, hasSemanticClass);
    }) ?? null;
  }

  private isAutomaticSurface(element: HTMLElement, hasSemanticClass: boolean): boolean {
    if (
      element === document.body
      || element === this.overlay
      || EXCLUDED_SURFACE_TAGS.has(element.tagName)
    ) {
      return false;
    }

    const rect = element.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {
      return false;
    }

    const style = window.getComputedStyle(element);
    if (!this.isVisibleElement(style)) {
      return false;
    }

    if (this.getVisibleBorderSides(style).length >= 2) {
      return true;
    }

    if (this.hasVisibleOutline(style) || this.hasInsetBorderShadow(style)) {
      return true;
    }

    const hasRoundedCorners = parseFloat(style.borderRadius) > 0;
    const hasBackground =
      style.backgroundImage !== 'none' || !this.isTransparentColor(style.backgroundColor);

    return hasSemanticClass && hasRoundedCorners && hasBackground;
  }

  private isDividerSurface(element: HTMLElement): boolean {
    if (
      element === document.body
      || element === this.overlay
      || EXCLUDED_SURFACE_TAGS.has(element.tagName)
    ) {
      return false;
    }

    const rect = element.getBoundingClientRect();
    const style = window.getComputedStyle(element);
    if (!this.isVisibleElement(style)) {
      return false;
    }

    const borderSides = this.getVisibleBorderSides(style);
    if (borderSides.length === 1) {
      return true;
    }

    return this.hasDividerSemantics(element) && this.isLineLikeGeometry(rect);
  }

  private getDividerGeometry(
    element: HTMLElement,
    rect: DOMRect,
    style: CSSStyleDeclaration,
  ): OverlayGeometry | null {
    const borderSides = this.getVisibleBorderSides(style);
    if (borderSides.length === 1) {
      const [borderSide] = borderSides;
      const thickness = Math.max(borderSide.width, 1);
      switch (borderSide.name) {
        case 'top':
          return { height: thickness, left: rect.left, top: rect.top, width: rect.width };
        case 'right':
          return {
            height: rect.height,
            left: rect.right - thickness,
            top: rect.top,
            width: thickness,
          };
        case 'bottom':
          return {
            height: thickness,
            left: rect.left,
            top: rect.bottom - thickness,
            width: rect.width,
          };
        case 'left':
          return { height: rect.height, left: rect.left, top: rect.top, width: thickness };
      }
    }

    if (this.hasDividerSemantics(element)) {
      return {
        height: Math.max(rect.height, 1),
        left: rect.left,
        top: rect.top,
        width: Math.max(rect.width, 1),
      };
    }

    return null;
  }

  private getVisibleBorderSides(style: CSSStyleDeclaration): VisibleBorderSide[] {
    const sides: Array<[BorderSideName, string, string, string]> = [
      ['top', style.borderTopWidth, style.borderTopStyle, style.borderTopColor],
      ['right', style.borderRightWidth, style.borderRightStyle, style.borderRightColor],
      ['bottom', style.borderBottomWidth, style.borderBottomStyle, style.borderBottomColor],
      ['left', style.borderLeftWidth, style.borderLeftStyle, style.borderLeftColor],
    ];

    return sides.flatMap(([name, width, borderStyle, color]) => {
      const numericWidth = parseFloat(width);
      return (
        numericWidth > 0
        && borderStyle !== 'none'
        && !this.isTransparentColor(color)
      )
        ? [{ name, width: numericWidth }]
        : [];
    });
  }

  private hasDividerSemantics(element: HTMLElement): boolean {
    return (
      element.tagName === 'HR'
      || element.getAttribute('role') === 'separator'
      || Array.from(element.classList).some(className => DIVIDER_CLASS_PATTERN.test(className))
    );
  }

  private isLineLikeGeometry(rect: DOMRect): boolean {
    const isHorizontalLine = rect.width > rect.height * 2 && rect.height <= 4;
    const isVerticalLine = rect.height > rect.width * 2 && rect.width <= 4;
    return isHorizontalLine || isVerticalLine;
  }

  private hasVisibleOutline(style: CSSStyleDeclaration): boolean {
    return (
      parseFloat(style.outlineWidth) > 0
      && style.outlineStyle !== 'none'
      && !this.isTransparentColor(style.outlineColor)
    );
  }

  private hasInsetBorderShadow(style: CSSStyleDeclaration): boolean {
    return style.boxShadow !== 'none' && /\binset\b/i.test(style.boxShadow);
  }

  private isResizeInteractionElement(element: HTMLElement): boolean {
    return /(?:^|-)resize$/i.test(window.getComputedStyle(element).cursor);
  }

  private findOverlayHost(surface: HTMLElement): HTMLElement {
    if (this.isFloatingLayer(surface)) {
      return surface;
    }

    let current = surface.parentElement;
    while (current && current !== document.body) {
      if (this.isFloatingLayer(current)) {
        return current;
      }
      if (this.createsStackingContext(current)) {
        return current;
      }
      current = current.parentElement;
    }
    return document.body;
  }

  private isFloatingLayer(element: HTMLElement): boolean {
    const role = element.getAttribute('role');
    if (
      (role && FLOATING_LAYER_ROLES.has(role))
      || element.getAttribute('aria-modal') === 'true'
      || element.hasAttribute('popover')
    ) {
      return true;
    }

    const style = window.getComputedStyle(element);
    const isPositionedLayer = style.position === 'absolute' || style.position === 'fixed';
    return (
      isPositionedLayer
      && Array.from(element.classList).some(className =>
        FLOATING_LAYER_CLASS_PATTERN.test(className)
      )
    );
  }

  private createsStackingContext(element: HTMLElement): boolean {
    const style = window.getComputedStyle(element);
    const positionedWithZIndex =
      style.position !== 'static' && style.zIndex !== 'auto';

    return (
      positionedWithZIndex
      || style.position === 'fixed'
      || style.position === 'sticky'
      || style.isolation === 'isolate'
      || (style.opacity !== '' && style.opacity !== '1')
      || (style.mixBlendMode !== '' && style.mixBlendMode !== 'normal')
      || style.willChange.includes('opacity')
    );
  }

  private isVisibleElement(style: CSSStyleDeclaration): boolean {
    return (
      style.display !== 'none'
      && style.display !== 'contents'
      && style.visibility !== 'hidden'
      && (style.opacity === '' || Number(style.opacity) !== 0)
    );
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
