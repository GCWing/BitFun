import { createContext, useContext, useEffect, useId, useLayoutEffect, useRef, useState, type HTMLAttributes, type KeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Icon } from "../Icon";
import { IconButton } from "../IconButton";
import { SearchField } from "../SearchField";
import { ScrollArea } from "../ScrollArea";
import { classNames } from "../../internal/classNames";
import { isImeOwnedKeyboardEvent } from "../../internal/ime";
import { resolveLayerPortal, useAnchoredLayer, type PortalTarget } from "../../internal/useAnchoredLayer";
import styles from "./Combobox.module.css";

export type ComboboxValue = string | number | (string | number)[];
export interface ComboboxOption {
  label: string;
  value: string | number;
  disabled?: boolean;
  description?: string;
  icon?: ReactNode;
  group?: string;
  testId?: string;
  testAttributes?: Record<`data-${string}`, string | number | boolean | undefined>;
}
export interface ComboboxLabels {
  placeholder: string;
  search: string;
  empty: string;
  loading: string;
  clear: string;
  selectAll: string;
  create: string;
}
const defaultLabels: ComboboxLabels = { placeholder: "Select…", search: "Search options", empty: "No options", loading: "Loading…", clear: "Clear selection", selectAll: "Select all", create: "Add value" };
const ComboboxContext = createContext<{ labels: ComboboxLabels; portalContainer?: PortalTarget }>({ labels: defaultLabels });
export function ComboboxProvider({ children, labels, portalContainer }: { children: ReactNode; labels?: Partial<ComboboxLabels>; portalContainer?: PortalTarget }) {
  return <ComboboxContext.Provider value={{ labels: { ...defaultLabels, ...labels }, portalContainer }}>{children}</ComboboxContext.Provider>;
}

export interface ComboboxProps extends Omit<HTMLAttributes<HTMLDivElement>, "defaultValue" | "onChange"> {
  required?: boolean;
  defaultOpen?: boolean;
  defaultSearchValue?: string;
  options?: readonly ComboboxOption[];
  value?: ComboboxValue;
  defaultValue?: ComboboxValue;
  onChange?: (value: ComboboxValue) => void;
  placeholder?: string;
  label?: string;
  disabled?: boolean;
  multiple?: boolean;
  searchable?: boolean;
  clearable?: boolean;
  showSelectAll?: boolean;
  loading?: boolean;
  error?: boolean;
  errorMessage?: string;
  size?: "sm" | "md" | "lg" | "small" | "medium" | "large";
  maxTagCount?: number;
  searchPlaceholder?: string;
  emptyText?: string;
  renderOption?: (option: ComboboxOption) => ReactNode;
  renderValue?: (option?: ComboboxOption | ComboboxOption[]) => ReactNode;
  placement?: "top" | "bottom";
  autoClose?: boolean;
  allowCustomValue?: boolean;
  customValueHint?: string;
  indicator?: ReactNode;
  onOpenChange?: (open: boolean) => void;
  triggerTestId?: string;
  dropdownTestId?: string;
  triggerAriaLabel?: string;
  triggerAriaLabelledBy?: string;
  triggerAriaDescribedBy?: string;
  dropdownClassName?: string;
  dropdownMode?: "overlay" | "inline";
  dropdownMatchTriggerWidth?: boolean;
  portalContainer?: PortalTarget;
}

/** Searchable single/multiple selection. Data, async discovery and copy remain host-owned. */
export function Combobox({
  options = [], value, defaultValue, onChange, placeholder, label, disabled = false, required = false, id: providedId, defaultOpen = false, defaultSearchValue = "",
  multiple = false, searchable = true, clearable = false, showSelectAll = false,
  loading = false, error = false, errorMessage, size = "md", maxTagCount = 3,
  searchPlaceholder, emptyText, renderOption, renderValue, placement = "bottom",
  autoClose = false, allowCustomValue = false, customValueHint, indicator, onOpenChange,
  triggerTestId, dropdownTestId, triggerAriaLabel, triggerAriaLabelledBy, triggerAriaDescribedBy,
  dropdownClassName, dropdownMode = "overlay", dropdownMatchTriggerWidth = true,
  portalContainer, className, ...props
}: ComboboxProps) {
  const context = useContext(ComboboxContext);
  const labels = context.labels;
  const generatedId = useId();
  const id = providedId ?? `bf-combobox-${generatedId}`;
  const invalid = error || props['aria-invalid'] === true || props['aria-invalid'] === 'true';
  const listId = `${id}-list`;
  const rootRef = useRef<HTMLDivElement>(null);
  const controlRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const popupRef = useRef<HTMLDivElement>(null);
  const composing = useRef(false);
  const [mounted, setMounted] = useState(false);
  useLayoutEffect(() => setMounted(true), []);
  const [open, setOpen] = useState(defaultOpen && !disabled);
  const [query, setQuery] = useState(defaultSearchValue);
  const [active, setActive] = useState(-1);
  const [internalValue, setInternalValue] = useState<ComboboxValue>(defaultValue ?? (multiple ? [] : ""));
  const selectedValue = value === undefined ? internalValue : value;
  const values = Array.isArray(selectedValue) ? selectedValue : selectedValue === "" ? [] : [selectedValue];
  const selectedOptions = values.map(v => options.find(option => option.value === v) ?? { label: String(v), value: v });
  const filtered = options.filter(option => !query || !searchable || `${option.label} ${option.value} ${option.description ?? ""}`.toLowerCase().includes(query.toLowerCase()));
  // Flatten groups in exactly the same order used by keyboard navigation and the DOM.
  const groups = new Map<string, ComboboxOption[]>();
  for (const option of filtered) { const group = option.group ?? ""; groups.set(group, [...(groups.get(group) ?? []), option]); }
  const displayed = [...groups.values()].flat();
  const custom: ComboboxOption | null = allowCustomValue && query.trim() && !options.some(option => String(option.value).toLowerCase() === query.trim().toLowerCase() || option.label.toLowerCase() === query.trim().toLowerCase())
    ? { label: query.trim(), value: query.trim() } : null;
  const choices = custom ? [...displayed, custom] : displayed;
  const activeIndex = choices[active] && !choices[active].disabled ? active : -1;
  const setValue = (next: ComboboxValue) => { if (value === undefined) setInternalValue(next); onChange?.(next); };
  const changeOpen = (next: boolean, restore = false) => {
    if (next === open || (next && disabled)) return;
    setOpen(next); onOpenChange?.(next); setActive(-1);
    if (!next) { setQuery(""); if (restore) triggerRef.current?.focus(); }
  };
  const select = (option: ComboboxOption) => {
    if (option.disabled || disabled) return;
    setValue(multiple ? (values.includes(option.value) ? values.filter(v => v !== option.value) : [...values, option.value]) : option.value);
    setQuery(""); setActive(-1);
    if (!multiple || autoClose) changeOpen(false, true);
    else inputRef.current?.focus();
  };
  const commitQuery = () => {
    if (!allowCustomValue || !query.trim()) return;
    const exact = options.find(option => String(option.value).toLowerCase() === query.trim().toLowerCase() || option.label.toLowerCase() === query.trim().toLowerCase());
    const option = exact ?? custom;
    if (option && !option.disabled && !values.includes(option.value)) {
      setValue(multiple ? [...values, option.value] : option.value);
    }
  };
  const layout = useAnchoredLayer({ open: open && mounted && dropdownMode === "overlay", anchorRef: controlRef, layerRef: popupRef, placement, matchWidth: dropdownMatchTriggerWidth, revision: `${query}:${options.length}:${loading}` });

  useEffect(() => {
    if (!open) return;
    if (disabled) { changeOpen(false); return; }
    const doc = rootRef.current?.ownerDocument;
    const outside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !popupRef.current?.contains(target)) {
        if (!multiple) commitQuery();
        changeOpen(false);
      }
    };
    doc?.addEventListener("mousedown", outside);
    return () => doc?.removeEventListener("mousedown", outside);
  });
  useEffect(() => { if (open && mounted && searchable) inputRef.current?.focus(); }, [open, mounted, searchable]);
  useEffect(() => {
    if (open && activeIndex >= 0) popupRef.current?.querySelector(`[data-option-index="${activeIndex}"]`)?.scrollIntoView?.({ block: "nearest" });
  }, [open, activeIndex]);

  const keyboard = (event: KeyboardEvent) => {
    if (disabled) return;
    if (isImeOwnedKeyboardEvent(event, composing.current)) { event.stopPropagation(); return; }
    if (event.key === "Escape" && open) { event.preventDefault(); event.stopPropagation(); changeOpen(false, true); return; }
    if (event.key === "Tab" && open) {
      // Portalled search is outside the trigger's tab order: return to the trigger before native Tab.
      commitQuery(); changeOpen(false, true); return;
    }
    if ((event.target as HTMLElement).closest("button") && event.target !== triggerRef.current) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp" || (open && (event.key === "Home" || event.key === "End"))) {
      event.preventDefault(); event.stopPropagation();
      if (!open) changeOpen(true);
      const indices = choices.flatMap((option, i) => option.disabled ? [] : [i]);
      if (!indices.length) return;
      const current = indices.indexOf(activeIndex);
      const next = event.key === "Home" ? 0 : event.key === "End" ? indices.length - 1 : event.key === "ArrowDown" ? (current + 1) % indices.length : (current <= 0 ? indices.length - 1 : current - 1);
      setActive(indices[next] ?? -1); return;
    }
    if (event.key === "Enter") {
      event.preventDefault(); event.stopPropagation();
      if (!open) changeOpen(true);
      else if (activeIndex >= 0 && choices[activeIndex]) select(choices[activeIndex]);
      else if (custom) select(custom);
      else if (query.trim()) { const exact = displayed.find(option => !option.disabled && (option.label.toLowerCase() === query.trim().toLowerCase() || String(option.value).toLowerCase() === query.trim().toLowerCase())); if (exact) select(exact); }
    }
  };
  const optionNode = (option: ComboboxOption, index: number, isCustom = false) => (
    <div key={`${typeof option.value}:${option.value}`} id={`${listId}-${index}`} role="option" aria-selected={values.includes(option.value)} aria-disabled={option.disabled || undefined}
      className={styles.option} data-bf-part="option" data-option-index={index} data-active={activeIndex === index} data-selected={values.includes(option.value)}
      data-testid={option.testId} {...option.testAttributes} onMouseDown={event => event.preventDefault()} onMouseMove={() => !option.disabled && setActive(index)} onClick={() => select(option)}>
      {isCustom ? <><Icon name="plus" size="sm" /><span>{customValueHint ?? labels.create}: {option.label}</span></> : <>
        {renderOption ? renderOption(option) : <>{option.icon}<span className={styles.copy}><span>{option.label}</span>{option.description && <small>{option.description}</small>}</span></>}
        {values.includes(option.value) && <Icon name="check-line" size="sm" />}
      </>}
    </div>
  );
  let optionIndex = 0;
  const popup = <div ref={popupRef} className={classNames(styles.popup, dropdownClassName)} data-bf-component="combobox-popup" data-placement={layout?.placement ?? placement}
    data-testid={dropdownTestId} onKeyDown={keyboard} style={dropdownMode === "inline" ? undefined : layout?.style ?? { position: "fixed", visibility: "hidden" }}>
    {searchable && <SearchField ref={inputRef} className={styles.search} aria-label={searchPlaceholder ?? labels.search} placeholder={searchPlaceholder ?? labels.search}
      role="combobox" aria-autocomplete="list" aria-expanded={open} aria-controls={listId} aria-activedescendant={activeIndex >= 0 ? `${listId}-${activeIndex}` : undefined}
      value={query} onValueChange={next => { setQuery(next); setActive(-1); }} onCompositionStart={() => { composing.current = true; }} onCompositionEnd={() => { composing.current = false; }} />}
    {multiple && showSelectAll && filtered.some(option => !option.disabled) && <button type="button" className={styles.selectAll} onClick={() => {
      const enabled = filtered.filter(option => !option.disabled).map(option => option.value);
      setValue(enabled.every(v => values.includes(v)) ? values.filter(v => !enabled.includes(v)) : [...new Set([...values, ...enabled])]);
    }}>{labels.selectAll}</button>}
    <ScrollArea className={styles.options} id={listId} role="listbox" aria-label={label ?? triggerAriaLabel ?? placeholder ?? labels.placeholder} aria-multiselectable={multiple || undefined} aria-busy={loading || undefined}>
      {[...groups].map(([group, groupOptions]) => group ? <div key={group} role="group" aria-label={group}><div className={styles.group}>{group}</div>{groupOptions.map(option => optionNode(option, optionIndex++))}</div> : groupOptions.map(option => optionNode(option, optionIndex++)))}
      {custom && optionNode(custom, displayed.length, true)}
      {!choices.length && <div className={styles.empty} role="status">{loading ? labels.loading : emptyText ?? labels.empty}</div>}
    </ScrollArea>
  </div>;
  const target = open && mounted ? resolveLayerPortal(portalContainer ?? context.portalContainer, triggerRef.current) : null;
  const selectedCopy = renderValue?.(multiple ? selectedOptions : selectedOptions[0]) ?? (selectedOptions.length ? selectedOptions.slice(0, Math.max(1, maxTagCount)).map(option => option.label).join(", ") + (selectedOptions.length > maxTagCount ? ` +${selectedOptions.length - maxTagCount}` : "") : placeholder ?? labels.placeholder);
  return <div {...props} ref={rootRef} className={classNames(styles.root, className)} data-bf-component="combobox" data-multiple={multiple} data-size={({ small: "sm", medium: "md", large: "lg" } as Record<string, string>)[size] ?? size} data-invalid={invalid} data-disabled={disabled}>
    {label && <label id={`${id}-label`} htmlFor={id} className={styles.label}>{label}</label>}
    <div ref={controlRef} className={styles.control} data-bf-part="control" data-tags={multiple && !renderValue && values.length > 0}>
      {multiple && !renderValue && values.length > 0 && <div className={styles.tags} data-bf-part="tags">
        {selectedOptions.slice(0, Math.max(1, maxTagCount)).map(option => <span key={`${typeof option.value}:${option.value}`} className={styles.tag} data-bf-part="tag">
          <span>{option.label}</span><IconButton aria-label={`${labels.clear}: ${option.label}`} icon={<Icon name="xmark" size="xs" />} size="xs" variant="quiet" disabled={disabled} onClick={() => setValue(values.filter(v => v !== option.value))} />
        </span>)}
        {values.length > Math.max(1, maxTagCount) && <span>+{values.length - Math.max(1, maxTagCount)}</span>}
      </div>}
      <button id={id} ref={triggerRef} type="button" disabled={disabled} className={styles.trigger} data-bf-part="trigger" data-testid={triggerTestId}
        role="combobox" aria-haspopup="listbox" aria-expanded={open} aria-controls={open ? listId : undefined}
        aria-activedescendant={open && !searchable && activeIndex >= 0 ? `${listId}-${activeIndex}` : undefined}
        aria-label={triggerAriaLabel ?? props["aria-label"] ?? (label || providedId ? undefined : placeholder ?? labels.placeholder)} aria-labelledby={triggerAriaLabelledBy ?? (label ? `${id}-label` : props["aria-labelledby"])}
        aria-describedby={[triggerAriaDescribedBy ?? props['aria-describedby'], error && errorMessage ? `${id}-error` : undefined].filter(Boolean).join(' ') || undefined} aria-invalid={invalid || undefined} aria-required={required || undefined} aria-busy={loading || undefined}
        onClick={() => changeOpen(!open)} onKeyDown={keyboard}>
        <span className={styles.value} data-bf-part="value" data-placeholder={!selectedOptions.length}>{selectedCopy}</span>
        <span data-bf-part="indicator">{loading ? <Icon name="refresh" size="sm" /> : indicator ?? <Icon name="chevron-down" size="sm" />}</span>
      </button>
      {clearable && values.length > 0 && <IconButton aria-label={labels.clear} icon={<Icon name="xmark" />} disabled={disabled} size="xs" variant="quiet" onClick={() => { setValue(multiple ? [] : ""); triggerRef.current?.focus(); }} />}
    </div>
    {open && !disabled && (dropdownMode === "inline" ? popup : target ? createPortal(popup, target) : null)}
    {error && errorMessage && <div id={`${id}-error`} className={styles.error}>{errorMessage}</div>}
  </div>;
}
