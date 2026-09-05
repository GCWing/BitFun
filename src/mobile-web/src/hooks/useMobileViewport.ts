import { useLayoutEffect } from 'react';

/** Keep browser toolbar/keyboard geometry separate from the layout breakpoint. */
export function mobileViewportInsets(layoutHeight: number, height: number, top: number, scale: number) {
  // Pinch zoom is a reading operation, not a keyboard or layout change.
  if (Math.abs(scale - 1) > 0.01) return null;
  return { height, top: Math.max(0, top), bottom: Math.max(0, layoutHeight - height - top) };
}

export function useMobileViewport() {
  useLayoutEffect(() => {
    const viewport = window.visualViewport;
    if (!viewport) return;
    const root = document.documentElement;
    const names = ['--mobile-viewport-height', '--mobile-viewport-top', '--mobile-viewport-bottom'];
    const previous = names.map(name => root.style.getPropertyValue(name));
    const update = () => {
      // Safari can shrink innerHeight while the 100dvh app shell stays tall.
      // Measure the shell's layout height so its fixed controls clear the keyboard.
      const visible = mobileViewportInsets(root.getBoundingClientRect().height, viewport.height, viewport.offsetTop, viewport.scale);
      if (!visible) return;
      [visible.height, visible.top, visible.bottom].forEach((value, index) => {
        root.style.setProperty(names[index], `${value}px`);
      });
    };
    update();
    viewport.addEventListener('resize', update);
    viewport.addEventListener('scroll', update);
    window.addEventListener('resize', update);
    return () => {
      viewport.removeEventListener('resize', update);
      viewport.removeEventListener('scroll', update);
      window.removeEventListener('resize', update);
      names.forEach((name, index) => {
        if (previous[index]) root.style.setProperty(name, previous[index]);
        else root.style.removeProperty(name);
      });
    };
  }, []);
}
