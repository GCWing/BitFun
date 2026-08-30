import { useEffect, useId, useLayoutEffect, useRef, useState, type ComponentType, type HTMLAttributes, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";
import { Menu, MenuItem, MenuSeparator, type MenuProps, type MenuItemRole } from "./Menu";
import { Icon } from "../Icon";
import { classNames } from "../../internal/classNames";
import { isImeOwnedKeyboardEvent } from "../../internal/ime";
import { resolveLayerPortal, useAnchoredLayer, type LayerPlacement, type PortalTarget } from "../../internal/useAnchoredLayer";
import { useSubmenuIntent } from "../../internal/useSubmenuIntent";
import styles from "./MenuPopover.module.css";

export interface MenuEntry {
  id: string;
  label: string;
  icon?: ReactNode;
  shortcut?: ReactNode;
  disabled?: boolean;
  separator?: boolean;
  submenu?: readonly MenuEntry[];
  role?: MenuItemRole;
  checked?: boolean;
  tone?: "neutral" | "danger";
  /** Dispatch only: the host owns async work and error presentation. */
  onSelect?: () => void;
}

export interface MenuPopoverProps extends Omit<MenuProps, "children"> {
  items: readonly MenuEntry[];
  open: boolean;
  onClose: () => void;
  anchorRef?: RefObject<HTMLElement | null>;
  position?: { x: number; y: number };
  placement?: LayerPlacement;
  portalContainer?: PortalTarget;
  /** Hosts that already own an overlay root can render in place. */
  portalled?: boolean;
  /** Stable wrappers must forward all props (and refs for root/item/separator). */
  parts?: MenuPopoverParts;
}

export interface MenuPopoverParts {
  root?: typeof Menu;
  item?: typeof MenuItem;
  separator?: typeof MenuSeparator;
  icon?: ComponentType<HTMLAttributes<HTMLSpanElement>>;
  label?: ComponentType<HTMLAttributes<HTMLSpanElement>>;
  shortcut?: ComponentType<HTMLAttributes<HTMLSpanElement>>;
  submenuArrow?: ComponentType<HTMLAttributes<HTMLSpanElement>>;
  submenu?: ComponentType<HTMLAttributes<HTMLDivElement>>;
}

function ownItems(menu: HTMLElement) {
  return Array.from(menu.querySelectorAll<HTMLButtonElement>("[data-bf-menu-item]"))
    .filter(item => item.closest('[role="menu"]') === menu && !item.disabled && item.getAttribute("aria-disabled") !== "true");
}

/** Anchored/coordinate menu with nested navigation, safe pointer corridors and focus return. */
export function MenuPopover({ items, open, onClose, anchorRef, position, placement = "bottom", portalContainer, portalled = true, autoFocusFirstItem = true, ...props }: MenuPopoverProps) {
  const markerRef = useRef<HTMLSpanElement>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  const wasOpen = useRef(false);
  const treeId = useId();
  const [present, setPresent] = useState(open);
  const [phase, setPhase] = useState("entering");
  const [target, setTarget] = useState<Element | DocumentFragment | null>(null);
  const resolvedAnchor = anchorRef ?? markerRef;
  const restoreFocus = () => {
    const target = previousFocus.current;
    if (target?.isConnected && !target.matches(":disabled")) target.focus();
  };
  const close = () => { restoreFocus(); onClose(); };

  useLayoutEffect(() => {
    setTarget(resolveLayerPortal(portalContainer, markerRef.current ?? resolvedAnchor.current));
  }, [portalContainer, resolvedAnchor, open]);

  useLayoutEffect(() => {
    const doc = markerRef.current?.ownerDocument;
    if (open && !wasOpen.current) {
      previousFocus.current = doc?.activeElement as HTMLElement | null;
      setPresent(true);
    } else if (!open && wasOpen.current && doc?.activeElement?.closest("[data-bf-menu-tree]")?.getAttribute("data-bf-menu-tree") === treeId) {
      restoreFocus();
    }
    wasOpen.current = open;
  }, [open, treeId]);

  useEffect(() => {
    const view = markerRef.current?.ownerDocument.defaultView;
    if (!view) return;
    let first = 0;
    let second = 0;
    let timer = 0;
    if (open) {
      setPhase("entering");
      first = view.requestAnimationFrame(() => {
        second = view.requestAnimationFrame(() => setPhase("entered"));
      });
    } else {
      setPhase("exiting");
      timer = view.setTimeout(() => setPresent(false), 100);
    }
    return () => { view.cancelAnimationFrame(first); view.cancelAnimationFrame(second); view.clearTimeout(timer); };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const doc = markerRef.current?.ownerDocument;
    const outside = (event: MouseEvent) => {
      const target = event.target as Element;
      if (target.closest?.("[data-bf-menu-tree]")?.getAttribute("data-bf-menu-tree") !== treeId && !anchorRef?.current?.contains(target)) onClose();
    };
    doc?.addEventListener("mousedown", outside, true);
    doc?.addEventListener("contextmenu", outside, true);
    return () => { doc?.removeEventListener("mousedown", outside, true); doc?.removeEventListener("contextmenu", outside, true); };
  }, [open, onClose, treeId, anchorRef]);

  const content = present ? <MenuLevel {...props} items={items} open={open} phase={phase} treeId={treeId} onClose={close} anchorRef={resolvedAnchor} position={position} placement={placement} autoFocusFirstItem={autoFocusFirstItem} portalTarget={portalled ? target : null} /> : null;
  return <><span ref={markerRef} hidden />{portalled ? target && createPortal(content, target) : content}</>;
}

interface MenuLevelProps extends Omit<MenuProps, "children"> {
  items: readonly MenuEntry[];
  open: boolean;
  phase: string;
  treeId: string;
  onClose: () => void;
  onBack?: () => void;
  anchorRef: RefObject<HTMLElement | null>;
  position?: { x: number; y: number };
  placement: LayerPlacement;
  portalTarget: Element | DocumentFragment | null;
  menuRef?: RefObject<HTMLDivElement | null>;
  parts?: MenuPopoverParts;
}

function MenuLevel({ items, open, phase, treeId, onClose, onBack, anchorRef, position, placement, portalTarget, menuRef: externalRef, autoFocusFirstItem, className, style, parts, ...props }: MenuLevelProps) {
  const MenuSurface = parts?.root ?? Menu;
  const Item = parts?.item ?? MenuItem;
  const Separator = parts?.separator ?? MenuSeparator;
  const Leading = parts?.icon ?? "span";
  const Label = parts?.label ?? "span";
  const Shortcut = parts?.shortcut ?? "span";
  const Arrow = parts?.submenuArrow ?? "span";
  const SubmenuBoundary = parts?.submenu ?? "div";
  const localRef = useRef<HTMLDivElement>(null);
  const menuRef = externalRef ?? localRef;
  const submenuRef = useRef<HTMLDivElement>(null);
  const submenuAnchor = useRef<HTMLElement | null>(null);
  const keyboardOpen = useRef(false);
  const [activeId, setActiveId] = useState<string | null>(null);
  const submenuId = useId();
  const intent = useSubmenuIntent({ activeId, onActiveIdChange: setActiveId, parentRef: menuRef, submenuRef, enabled: open, openDelayMs: 150, closeDelayMs: 300, switchDelayMs: 300, tolerance: 50 });
  // Keep geometry until presence unmounts the menu, including its exit transition.
  const layout = useAnchoredLayer({ open: true, anchorRef, layerRef: menuRef, placement, point: position });
  const activeEntry = items.find(item => item.id === activeId && !item.disabled && item.submenu?.length);
  const openSubmenu = (item: MenuEntry, trigger: HTMLElement, keyboard: boolean) => {
    if (item.disabled || !item.submenu?.length) return;
    submenuAnchor.current = trigger;
    keyboardOpen.current = keyboard;
    intent.openNow(item.id);
    if (keyboard && activeId === item.id && submenuRef.current) ownItems(submenuRef.current)[0]?.focus();
  };
  const activate = (item: MenuEntry) => {
    if (item.disabled || item.separator) return;
    onClose();
    item.onSelect?.();
  };

  useEffect(() => { if (!open) intent.closeNow(); }, [open, intent.closeNow]);
  useEffect(() => {
    if (!open) return;
    const menu = menuRef.current;
    const doc = menu?.ownerDocument;
    const keyboard = (event: KeyboardEvent) => {
      if (!menu || doc?.activeElement?.closest('[role="menu"]') !== menu || isImeOwnedKeyboardEvent(event)) return;
      const enabled = ownItems(menu);
      const current = enabled.indexOf(doc.activeElement as HTMLButtonElement);
      const button = enabled[current];
      const item = items.find(entry => entry.id === button?.getAttribute("data-menu-id"));
      let index: number | undefined;
      switch (event.key) {
        case "ArrowDown": index = (current + 1) % enabled.length; break;
        case "ArrowUp": index = (current - 1 + enabled.length) % enabled.length; break;
        case "Home": index = 0; break;
        case "End": index = enabled.length - 1; break;
        case "Escape": event.preventDefault(); event.stopPropagation(); onClose(); return;
        case "Tab": onClose(); return;
        case "ArrowLeft":
          if (onBack) { event.preventDefault(); event.stopPropagation(); onBack(); }
          return;
        case "ArrowRight":
          if (item?.submenu?.length && button) { event.preventDefault(); event.stopPropagation(); openSubmenu(item, button, true); }
          return;
        case "Enter":
        case " ":
          event.preventDefault(); event.stopPropagation();
          if (item && button) { if (item.submenu?.length) openSubmenu(item, button, true); else activate(item); }
          return;
        default:
          if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
            for (let offset = 1; offset <= enabled.length; offset++) {
              const candidate = (current + offset) % enabled.length;
              if (enabled[candidate]?.textContent?.trim().toLowerCase().startsWith(event.key.toLowerCase())) { index = candidate; break; }
            }
          }
      }
      if (index !== undefined) { event.preventDefault(); event.stopPropagation(); enabled[index]?.focus(); enabled[index]?.scrollIntoView?.({ block: "nearest" }); }
    };
    doc?.addEventListener("keydown", keyboard, true);
    return () => doc?.removeEventListener("keydown", keyboard, true);
  });

  const submenu = activeEntry ? <SubmenuBoundary className={styles.submenuBoundary}><MenuLevel key={activeEntry.id} id={submenuId} aria-label={activeEntry.label} menuRef={submenuRef} items={activeEntry.submenu!} open={open} phase={phase} treeId={treeId} onClose={onClose} parts={parts}
    onBack={() => { intent.closeNow(); submenuAnchor.current?.focus(); }} anchorRef={submenuAnchor} placement="right" portalTarget={portalTarget} autoFocusFirstItem={keyboardOpen.current}
    onPointerEnter={intent.keepOpen} onPointerLeave={intent.requestClose} /></SubmenuBoundary> : null;

  return <>
    <MenuSurface {...props} ref={node => { (menuRef as { current: HTMLDivElement | null }).current = node; }} className={classNames(styles.popup, className)} autoFocusFirstItem={open && autoFocusFirstItem && Boolean(layout)} tabIndex={-1}
      style={{ ...layout?.style, ...style, visibility: layout ? undefined : "hidden" }} data-bf-menu-tree={treeId} data-placement={layout?.placement ?? placement} data-state={phase}
      aria-hidden={!open || undefined} {...(!open ? { inert: "" } : {})} onContextMenu={event => event.preventDefault()}>
      {items.map(item => item.separator ? <Separator key={item.id} /> : <Item key={item.id} data-menu-id={item.id} leading={item.icon ? <Leading>{item.icon}</Leading> : undefined} shortcut={item.shortcut ? <Shortcut>{item.shortcut}</Shortcut> : undefined} tone={item.tone} role={item.role} checked={item.checked}
        disabled={item.disabled} aria-disabled={item.disabled || undefined} aria-haspopup={item.submenu?.length ? "menu" : undefined} aria-expanded={item.submenu?.length ? activeEntry?.id === item.id : undefined}
        aria-controls={activeEntry?.id === item.id ? submenuId : undefined} metadata={item.submenu?.length ? <Arrow><Icon name="chevron-right" size="sm" /></Arrow> : undefined}
        onClick={event => { event.stopPropagation(); if (item.submenu?.length) openSubmenu(item, event.currentTarget, true); else activate(item); }}
        onPointerEnter={event => { if (activeId === null || activeId === item.id) submenuAnchor.current = event.currentTarget; keyboardOpen.current = false; intent.requestChange(!item.disabled && item.submenu?.length ? item.id : null, event); }}
        onPointerLeave={intent.requestClose}
        ref={element => { if (activeEntry?.id === item.id && element) submenuAnchor.current = element; }}>
        <Label>{item.label}</Label>
      </Item>)}
    </MenuSurface>
    {submenu && (portalTarget ? createPortal(submenu, portalTarget) : submenu)}
  </>;
}
