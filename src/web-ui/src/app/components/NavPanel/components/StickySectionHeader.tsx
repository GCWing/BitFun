import React, { useEffect, useRef, useState } from 'react';

interface StickySectionHeaderProps {
  children: React.ReactNode;
  scrollRootRef: React.RefObject<HTMLDivElement | null>;
}

/**
 * Keeps one section header in the DOM while exposing whether it has crossed
 * the top edge of its scroll container. CSS owns the sticky positioning; the
 * observer state is only for the docked divider and appearance contract.
 */
const StickySectionHeader: React.FC<StickySectionHeaderProps> = ({
  children,
  scrollRootRef,
}) => {
  const sentinelRef = useRef<HTMLSpanElement | null>(null);
  const [isStuck, setIsStuck] = useState(false);

  useEffect(() => {
    const scrollRoot = scrollRootRef.current;
    const sentinel = sentinelRef.current;
    if (!scrollRoot || !sentinel || typeof IntersectionObserver === 'undefined') {
      return;
    }

    const observer = new IntersectionObserver(([entry]) => {
      if (!entry) return;
      const rootTop = entry.rootBounds?.top ?? scrollRoot.getBoundingClientRect().top;
      const nextIsStuck = !entry.isIntersecting && entry.boundingClientRect.top < rootTop;
      setIsStuck(current => current === nextIsStuck ? current : nextIsStuck);
    }, {
      root: scrollRoot,
      threshold: [0, 1],
    });

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [scrollRootRef]);

  return (
    <>
      <span
        ref={sentinelRef}
        className="bitfun-nav-panel__sticky-section-sentinel"
        aria-hidden="true"
      />
      <div
        className={`bitfun-nav-panel__sticky-section-header${isStuck ? ' is-stuck' : ''}`}
        data-bf-component="nav-panel"
        data-bf-part="stickySectionHeader"
        data-bf-state={isStuck ? 'stuck' : undefined}
        data-testid="nav-sessions-sticky-header"
      >
        {children}
      </div>
    </>
  );
};

export default React.memo(StickySectionHeader);
