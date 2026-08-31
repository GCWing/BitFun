import {
  useEffect,
  useLayoutEffect,
  useState,
  type PropsWithChildren,
} from 'react';
import { useAppearance } from '@/infrastructure/appearance';

const DESIGN_SYSTEM_ROOT_ATTRIBUTES = {
  'data-bf-design-system-root': '',
  'data-density': 'compact',
} as const;
const HIGH_CONTRAST_MEDIA_QUERY = '(prefers-contrast: more), (forced-colors: active)';

function readHighContrastPreference(): boolean {
  return typeof window !== 'undefined'
    && typeof window.matchMedia === 'function'
    && window.matchMedia(HIGH_CONTRAST_MEDIA_QUERY).matches;
}

function useHighContrastPreference(): boolean {
  const [highContrast, setHighContrast] = useState(readHighContrastPreference);

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return undefined;
    const media = window.matchMedia(HIGH_CONTRAST_MEDIA_QUERY);
    const update = () => setHighContrast(media.matches);
    update();
    media.addEventListener('change', update);
    return () => media.removeEventListener('change', update);
  }, []);

  return highContrast;
}

/**
 * Maps the product-owned appearance selection onto the independent design
 * system's DOM contract. The attributes live on the document root so package
 * components rendered through portals inherit the same tokens as scene UI.
 */
export function DesignSystemThemeBridge({ children }: PropsWithChildren) {
  const colorScheme = useAppearance().current?.mode ?? 'light';
  const contrast = useHighContrastPreference() ? 'high' : 'standard';

  useLayoutEffect(() => {
    const root = document.documentElement;
    const attributes = {
      ...DESIGN_SYSTEM_ROOT_ATTRIBUTES,
      'data-color-scheme': colorScheme,
      'data-contrast': contrast,
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
  }, [colorScheme, contrast]);

  return children;
}
