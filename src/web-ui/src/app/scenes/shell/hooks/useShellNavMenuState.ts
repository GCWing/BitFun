import { useCallback, useEffect, useRef, useState } from 'react';

interface MenuPosition {
  top: number;
  left: number;
}

interface UseShellNavMenuStateReturn {
  menuOpen: boolean;
  setMenuOpen: React.Dispatch<React.SetStateAction<boolean>>;
  workspaceMenuOpen: boolean;
  setWorkspaceMenuOpen: React.Dispatch<React.SetStateAction<boolean>>;
  workspaceMenuPosition: MenuPosition | null;
  menuRef: React.RefObject<HTMLDivElement>;
  workspaceMenuRef: React.RefObject<HTMLDivElement>;
  workspaceTriggerRef: React.RefObject<HTMLButtonElement>;
}

export function useShellNavMenuState(
  hasMultipleWorkspaces: boolean,
): UseShellNavMenuStateReturn {
  const [menuOpen, setMenuOpen] = useState(false);
  const [workspaceMenuOpen, setWorkspaceMenuOpen] = useState(false);
  const [workspaceMenuPosition, setWorkspaceMenuPosition] = useState<MenuPosition | null>(null);

  const menuRef = useRef<HTMLDivElement>(null);
  const workspaceMenuRef = useRef<HTMLDivElement>(null);
  const workspaceTriggerRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!menuOpen && !workspaceMenuOpen) {
      return;
    }

    const handleMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (
        target &&
        (menuRef.current?.contains(target) ||
          workspaceMenuRef.current?.contains(target) ||
          workspaceTriggerRef.current?.contains(target))
      ) {
        return;
      }

      setMenuOpen(false);
      setWorkspaceMenuOpen(false);
    };

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setMenuOpen(false);
        setWorkspaceMenuOpen(false);
      }
    };

    document.addEventListener('mousedown', handleMouseDown);
    document.addEventListener('keydown', handleEscape);

    return () => {
      document.removeEventListener('mousedown', handleMouseDown);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [menuOpen, workspaceMenuOpen]);

  useEffect(() => {
    if (!hasMultipleWorkspaces && workspaceMenuOpen) {
      setWorkspaceMenuOpen(false);
    }
  }, [hasMultipleWorkspaces, workspaceMenuOpen]);

  const updateWorkspaceMenuPosition = useCallback(() => {
    const trigger = workspaceTriggerRef.current;
    if (!trigger) {
      return;
    }

    const rect = trigger.getBoundingClientRect();
    const viewportPadding = 8;
    const estimatedWidth = 220;
    const maxLeft = window.innerWidth - estimatedWidth - viewportPadding;

    setWorkspaceMenuPosition({
      top: Math.max(viewportPadding, rect.bottom + 6),
      left: Math.max(viewportPadding, Math.min(rect.left, maxLeft)),
    });
  }, []);

  useEffect(() => {
    if (!workspaceMenuOpen) {
      return;
    }

    updateWorkspaceMenuPosition();

    const handleViewportChange = () => updateWorkspaceMenuPosition();
    window.addEventListener('resize', handleViewportChange);
    window.addEventListener('scroll', handleViewportChange, true);

    return () => {
      window.removeEventListener('resize', handleViewportChange);
      window.removeEventListener('scroll', handleViewportChange, true);
    };
  }, [updateWorkspaceMenuPosition, workspaceMenuOpen]);

  return {
    menuOpen,
    setMenuOpen,
    workspaceMenuOpen,
    setWorkspaceMenuOpen,
    workspaceMenuPosition,
    menuRef,
    workspaceMenuRef,
    workspaceTriggerRef,
  };
}
