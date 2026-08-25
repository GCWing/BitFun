import { useLayoutEffect, type PropsWithChildren } from 'react';
import { useAppearance } from '@/infrastructure/appearance';

const DESIGN_SYSTEM_ROOT_ATTRIBUTES = {
  'data-bf-design-system-root': '',
  'data-contrast': 'standard',
  'data-density': 'compact',
} as const;

/**
 * Maps the product-owned appearance selection onto the independent design
 * system's DOM contract. The attributes live on the document root so package
 * components rendered through portals inherit the same tokens as scene UI.
 */
export function DesignSystemThemeBridge({ children }: PropsWithChildren) {
  const colorScheme = useAppearance().current?.mode ?? 'light';

  useLayoutEffect(() => {
    const root = document.documentElement;
    const attributes = {
      ...DESIGN_SYSTEM_ROOT_ATTRIBUTES,
      'data-color-scheme': colorScheme,
    } as const;
    const previousValues = new Map(
      Object.keys(attributes).map(name => [name, root.getAttribute(name)]),
    );

    Object.entries(attributes).forEach(([name, value]) => {
      root.setAttribute(name, value);
    });

    return () => {
      previousValues.forEach((value, name) => {
        if (value === null) {
          root.removeAttribute(name);
        } else {
          root.setAttribute(name, value);
        }
      });
    };
  }, [colorScheme]);

  return children;
}
