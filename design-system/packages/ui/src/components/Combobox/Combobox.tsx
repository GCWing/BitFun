import {
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type HTMLAttributes,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { LoaderCircle } from "lucide-react";
import { Icon } from "../Icon";
import { classNames } from "../../internal/classNames";
import { IconButton } from "../IconButton";
import {
  Listbox,
  ListboxEmpty,
  ListboxGroup,
  ListboxOption,
  type ListboxValue,
} from "../Listbox";
import { SearchField } from "../SearchField";
import {
  resolveLayerPortal,
  useAnchoredLayer,
  type PortalTarget,
} from "../../internal/useAnchoredLayer";
import styles from "./Combobox.module.css";

export type ComboboxValue = ListboxValue;
export type ComboboxSize = "sm" | "md" | "lg";
export type ComboboxPlacement = "top" | "bottom";
export type ComboboxPopoverMode = "overlay" | "inline";
export type ComboboxPortalTarget = PortalTarget;

export interface ComboboxOption {
  description?: ReactNode;
  disabled?: boolean;
  group?: string;
  /** Compatibility alias for `leading`. */
  icon?: ReactNode;
  label: string;
  leading?: ReactNode;
  metadata?: ReactNode;
  testAttributes?: Record<`data-${string}`, string | number | boolean | undefined>;
  testId?: string;
  value: ComboboxValue;
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

const defaultLabels: ComboboxLabels = {
  placeholder: "Select an option",
  search: "Search options",
  empty: "No options",
  loading: "Loading",
  clear: "Clear selection",
  selectAll: "Select all",
  create: "Use custom value",
};

const ComboboxContext = createContext<{
  labels: ComboboxLabels;
  portalContainer?: ComboboxPortalTarget;
}>({ labels: defaultLabels });

export function ComboboxProvider({
  children,
  labels,
  portalContainer,
}: {
  children: ReactNode;
  labels?: Partial<ComboboxLabels>;
  portalContainer?: ComboboxPortalTarget;
}) {
  const value = useMemo(() => ({
    labels: { ...defaultLabels, ...labels },
    portalContainer,
  }), [labels, portalContainer]);
  return <ComboboxContext.Provider value={value}>{children}</ComboboxContext.Provider>;
}

export interface ComboboxProps
  extends Omit<HTMLAttributes<HTMLDivElement>, "defaultValue" | "onChange"> {
  allowCustomValue?: boolean;
  autoClose?: boolean;
  clearLabel?: string;
  clearable?: boolean;
  customValueHint?: ReactNode | ((value: string) => ReactNode);
  defaultOpen?: boolean;
  defaultSearchValue?: string;
  defaultValue?: ComboboxValue | ComboboxValue[];
  disabled?: boolean;
  /** Compatibility alias for `popoverMode`. */
  dropdownMode?: ComboboxPopoverMode;
  /** Compatibility alias for `matchTriggerWidth`. */
  dropdownMatchTriggerWidth?: boolean;
  dropdownClassName?: string;
  dropdownTestId?: string;
  emptyText?: ReactNode;
  /** Compatibility alias for `invalid`. */
  error?: boolean;
  errorMessage?: ReactNode;
  filterOption?: (option: ComboboxOption, query: string) => boolean;
  invalid?: boolean;
  label?: ReactNode;
  loading?: boolean;
  loadingText?: ReactNode;
  matchTriggerWidth?: boolean;
  maxTagCount?: number;
  multiple?: boolean;
  /** Compatibility callback retained while consumers migrate to `onValueChange`. */
  onChange?: (value: ComboboxValue | ComboboxValue[]) => void;
  onOpenChange?: (open: boolean) => void;
  onValueChange?: (value: ComboboxValue | ComboboxValue[]) => void;
  open?: boolean;
  options?: readonly ComboboxOption[];
  placement?: ComboboxPlacement;
  placeholder?: ReactNode;
  popoverMode?: ComboboxPopoverMode;
  portalContainer?: ComboboxPortalTarget;
  renderOption?: (option: ComboboxOption) => ReactNode;
  renderValue?: (option?: ComboboxOption | ComboboxOption[]) => ReactNode;
  required?: boolean;
  searchPlaceholder?: string;
  searchable?: boolean;
  selectAllLabel?: ReactNode;
  showSelectAll?: boolean;
  size?: ComboboxSize | "small" | "medium" | "large";
  triggerAriaDescribedBy?: string;
  triggerAriaLabel?: string;
  triggerAriaLabelledBy?: string;
  triggerClassName?: string;
  triggerTestId?: string;
  value?: ComboboxValue | ComboboxValue[];
  /** Optional custom suffix retained for existing public consumers. */
  indicator?: ReactNode;
}

type NavigationItem =
  | { kind: "all" }
  | { kind: "custom"; value: string }
  | { kind: "option"; option: ComboboxOption };

function isImeOwnedKeyboardEvent(event: ReactKeyboardEvent) {
  const nativeEvent = event.nativeEvent as KeyboardEvent;
  return nativeEvent.isComposing || nativeEvent.keyCode === 229;
}

function optionMatches(option: ComboboxOption, query: string) {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  if (!normalizedQuery) return true;
  return option.label.toLocaleLowerCase().includes(normalizedQuery)
    || String(option.value).toLocaleLowerCase().includes(normalizedQuery)
    || (typeof option.description === "string"
      && option.description.toLocaleLowerCase().includes(normalizedQuery));
}

function valuesFrom(value: ComboboxProps["value"], multiple: boolean): ComboboxValue[] {
  if (Array.isArray(value)) return value;
  if (value === undefined || value === "") return [];
  return multiple ? [value] : [value];
}

function firstEnabledIndex(items: readonly NavigationItem[], direction: 1 | -1) {
  if (items.length === 0) return -1;
  let index = direction === 1 ? 0 : items.length - 1;
  while (index >= 0 && index < items.length) {
    const item = items[index];
    if (item?.kind !== "option" || !item.option.disabled) return index;
    index += direction;
  }
  return -1;
}

export const Combobox = forwardRef<HTMLDivElement, ComboboxProps>(function Combobox({
  allowCustomValue = false,
  autoClose,
  className,
  clearLabel: clearLabelProp,
  clearable = false,
  customValueHint: customValueHintProp,
  defaultOpen = false,
  defaultSearchValue = "",
  defaultValue,
  id: providedId,
  "aria-describedby": rootAriaDescribedBy,
  "aria-invalid": rootAriaInvalid,
  "aria-label": rootAriaLabel,
  "aria-labelledby": rootAriaLabelledBy,
  disabled = false,
  dropdownMode,
  dropdownMatchTriggerWidth,
  dropdownClassName,
  dropdownTestId,
  emptyText: emptyTextProp,
  error = false,
  errorMessage,
  filterOption = optionMatches,
  invalid: invalidProp = false,
  indicator,
  label,
  loading = false,
  loadingText: loadingTextProp,
  matchTriggerWidth: matchTriggerWidthProp,
  maxTagCount = 3,
  multiple = false,
  onChange,
  onOpenChange,
  onValueChange,
  open,
  options: optionsProp = [],
  placement = "bottom",
  placeholder: placeholderProp,
  popoverMode: popoverModeProp,
  portalContainer: portalContainerProp,
  renderOption,
  renderValue,
  required = false,
  searchPlaceholder: searchPlaceholderProp,
  searchable = true,
  selectAllLabel: selectAllLabelProp,
  showSelectAll = false,
  size: sizeProp = "md",
  triggerAriaDescribedBy,
  triggerAriaLabel,
  triggerAriaLabelledBy,
  triggerClassName,
  triggerTestId,
  value,
  ...rootProps
}, forwardedRef) {
  const context = useContext(ComboboxContext);
  const clearLabel = clearLabelProp ?? context.labels.clear;
  const customValueHint = customValueHintProp ?? context.labels.create;
  const emptyText = emptyTextProp ?? context.labels.empty;
  const invalid = invalidProp || error
    || rootAriaInvalid === true
    || rootAriaInvalid === "true";
  const loadingText = loadingTextProp ?? context.labels.loading;
  const matchTriggerWidth = matchTriggerWidthProp ?? dropdownMatchTriggerWidth ?? true;
  const options = optionsProp;
  const placeholder = placeholderProp ?? context.labels.placeholder;
  const popoverMode = popoverModeProp ?? dropdownMode ?? "overlay";
  const portalContainer = portalContainerProp ?? context.portalContainer;
  const searchPlaceholder = searchPlaceholderProp ?? context.labels.search;
  const selectAllLabel = selectAllLabelProp ?? context.labels.selectAll;
  const size: ComboboxSize = sizeProp === "small"
    ? "sm"
    : sizeProp === "medium"
      ? "md"
      : sizeProp === "large"
        ? "lg"
        : sizeProp;
  const generatedId = useId();
  const id = providedId ?? `bf-combobox-${generatedId}`;
  const labelId = `${id}-label`;
  const listboxId = `${id}-listbox`;
  const errorId = `${id}-error`;
  const [uncontrolledOpen, setUncontrolledOpen] = useState(
    defaultOpen && !disabled,
  );
  const [uncontrolledValue, setUncontrolledValue] = useState<ComboboxValue | ComboboxValue[]>(
    defaultValue ?? (multiple ? [] : ""),
  );
  const [query, setQuery] = useState(defaultSearchValue);
  const [activeIndex, setActiveIndex] = useState(-1);
  const [keyboardOpen, setKeyboardOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const searchRef = useRef<HTMLInputElement | null>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const resolvedOpen = open ?? uncontrolledOpen;
  const resolvedValue = value ?? uncontrolledValue;
  const selectedValues = useMemo(
    () => valuesFrom(resolvedValue, multiple),
    [multiple, resolvedValue],
  );

  const setRootRef = useCallback((node: HTMLDivElement | null) => {
    rootRef.current = node;
    if (typeof forwardedRef === "function") forwardedRef(node);
    else if (forwardedRef) forwardedRef.current = node;
  }, [forwardedRef]);

  const setSearchInputRef = useCallback((node: HTMLInputElement | null) => {
    searchRef.current = node;
    if (node && resolvedOpen && searchable) node.focus();
  }, [resolvedOpen, searchable]);

  const updateOpen = useCallback((nextOpen: boolean) => {
    if (nextOpen === resolvedOpen) return;
    if (open === undefined) setUncontrolledOpen(nextOpen);
    onOpenChange?.(nextOpen);
  }, [onOpenChange, open, resolvedOpen]);

  const commitValue = useCallback((nextValue: ComboboxValue | ComboboxValue[]) => {
    if (value === undefined) setUncontrolledValue(nextValue);
    onValueChange?.(nextValue);
    onChange?.(nextValue);
  }, [onChange, onValueChange, value]);

  const filteredOptions = useMemo(
    () => searchable && query.trim()
      ? options.filter(option => filterOption(option, query))
      : [...options],
    [filterOption, options, query, searchable],
  );

  const customCandidate = useMemo(() => {
    const candidate = query.trim();
    if (!allowCustomValue || !candidate) return null;
    const normalized = candidate.toLocaleLowerCase();
    const exactMatch = options.some(option => (
      String(option.value).toLocaleLowerCase() === normalized
      || option.label.toLocaleLowerCase() === normalized
    ));
    return exactMatch ? null : candidate;
  }, [allowCustomValue, options, query]);

  const navigationItems = useMemo<NavigationItem[]>(() => {
    const items: NavigationItem[] = [];
    if (multiple && showSelectAll && filteredOptions.some(option => !option.disabled)) {
      items.push({ kind: "all" });
    }
    filteredOptions.forEach(option => items.push({ kind: "option", option }));
    if (customCandidate) items.push({ kind: "custom", value: customCandidate });
    return items;
  }, [customCandidate, filteredOptions, multiple, showSelectAll]);

  const optionId = useCallback((index: number) => `${listboxId}-option-${index}`, [listboxId]);

  const moveActive = useCallback((current: number, direction: 1 | -1) => {
    if (navigationItems.length === 0) return -1;
    let index = current;
    for (let count = 0; count < navigationItems.length; count += 1) {
      index += direction;
      if (index < 0) index = navigationItems.length - 1;
      if (index >= navigationItems.length) index = 0;
      const item = navigationItems[index];
      if (item?.kind !== "option" || !item.option.disabled) return index;
    }
    return -1;
  }, [navigationItems]);

  const selectOption = useCallback((option: ComboboxOption) => {
    if (disabled || loading || option.disabled) return;
    if (multiple) {
      const nextValues = selectedValues.includes(option.value)
        ? selectedValues.filter(candidate => candidate !== option.value)
        : [...selectedValues, option.value];
      commitValue(nextValues);
      if (autoClose === true) updateOpen(false);
      return;
    }
    commitValue(option.value);
    setQuery("");
    updateOpen(false);
    triggerRef.current?.focus();
  }, [autoClose, commitValue, disabled, loading, multiple, selectedValues, updateOpen]);

  const submitCustomValue = useCallback((candidate: string) => {
    if (!allowCustomValue || !candidate || disabled || loading) return;
    if (multiple) {
      const nextValues = selectedValues.includes(candidate)
        ? selectedValues
        : [...selectedValues, candidate];
      commitValue(nextValues);
      if (autoClose === true) updateOpen(false);
    } else {
      commitValue(candidate);
      updateOpen(false);
      triggerRef.current?.focus();
    }
    setQuery("");
  }, [allowCustomValue, autoClose, commitValue, disabled, loading, multiple, selectedValues, updateOpen]);

  const toggleSelectAll = useCallback(() => {
    if (!multiple || disabled || loading) return;
    const availableValues = filteredOptions
      .filter(option => !option.disabled)
      .map(option => option.value);
    const allSelected = availableValues.length > 0
      && availableValues.every(candidate => selectedValues.includes(candidate));
    commitValue(allSelected
      ? selectedValues.filter(candidate => !availableValues.includes(candidate))
      : [...new Set([...selectedValues, ...availableValues])]);
  }, [commitValue, disabled, filteredOptions, loading, multiple, selectedValues]);

  const activateItem = useCallback((index: number) => {
    const item = navigationItems[index];
    if (!item) return;
    if (item.kind === "all") toggleSelectAll();
    else if (item.kind === "custom") submitCustomValue(item.value);
    else selectOption(item.option);
  }, [navigationItems, selectOption, submitCustomValue, toggleSelectAll]);

  const openListbox = useCallback((direction: 1 | -1 = 1, keyboard = false) => {
    if (disabled) return;
    setKeyboardOpen(keyboard);
    updateOpen(true);
    if (!keyboard) {
      setActiveIndex(-1);
      return;
    }
    const selectedIndex = navigationItems.findIndex(item => (
      item.kind === "option" && selectedValues.includes(item.option.value) && !item.option.disabled
    ));
    setActiveIndex(selectedIndex >= 0
      ? selectedIndex
      : firstEnabledIndex(navigationItems, direction));
  }, [disabled, navigationItems, selectedValues, updateOpen]);

  const closeListbox = useCallback((restoreFocus: boolean) => {
    updateOpen(false);
    setQuery("");
    setActiveIndex(-1);
    if (restoreFocus) triggerRef.current?.focus();
  }, [updateOpen]);

  const handleTriggerKeyDown = useCallback((event: ReactKeyboardEvent<HTMLButtonElement>) => {
    if (disabled) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!resolvedOpen) openListbox(event.key === "ArrowDown" ? 1 : -1, true);
      else setActiveIndex(current => moveActive(current, event.key === "ArrowDown" ? 1 : -1));
      return;
    }
    if (event.key === "Home" && resolvedOpen) {
      event.preventDefault();
      setActiveIndex(firstEnabledIndex(navigationItems, 1));
      return;
    }
    if (event.key === "End" && resolvedOpen) {
      event.preventDefault();
      setActiveIndex(firstEnabledIndex(navigationItems, -1));
      return;
    }
    if (event.key === "Escape" && resolvedOpen) {
      event.preventDefault();
      closeListbox(true);
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (!resolvedOpen) openListbox(1, true);
      else if (activeIndex >= 0) activateItem(activeIndex);
    }
  }, [
    activateItem,
    activeIndex,
    closeListbox,
    disabled,
    moveActive,
    navigationItems,
    openListbox,
    resolvedOpen,
  ]);

  const handleSearchKeyDown = useCallback((event: ReactKeyboardEvent<HTMLInputElement>) => {
    if ((event.key === "Enter" || event.key === "Escape") && isImeOwnedKeyboardEvent(event)) {
      event.stopPropagation();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      setActiveIndex(current => moveActive(current, event.key === "ArrowDown" ? 1 : -1));
      return;
    }
    if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      event.stopPropagation();
      setActiveIndex(firstEnabledIndex(navigationItems, event.key === "Home" ? 1 : -1));
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      if (activeIndex >= 0) activateItem(activeIndex);
      else if (customCandidate) submitCustomValue(customCandidate);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      closeListbox(true);
      return;
    }
    if (event.key === "Tab") {
      const candidate = query.trim();
      const exactOption = candidate
        ? options.find(option => (
            String(option.value).toLocaleLowerCase() === candidate.toLocaleLowerCase()
            || option.label.toLocaleLowerCase() === candidate.toLocaleLowerCase()
          ))
        : undefined;
      if (exactOption && !exactOption.disabled) selectOption(exactOption);
      else if (customCandidate) submitCustomValue(customCandidate);
      closeListbox(true);
    }
  }, [
    activateItem,
    activeIndex,
    closeListbox,
    customCandidate,
    moveActive,
    navigationItems,
    options,
    query,
    selectOption,
    submitCustomValue,
  ]);

  useEffect(() => {
    if (!resolvedOpen) return;
    if (activeIndex < 0) return;
    const current = navigationItems[activeIndex];
    if (current?.kind !== "option" || !current.option.disabled) {
      if (current) return;
    }
    setActiveIndex(firstEnabledIndex(navigationItems, 1));
  }, [activeIndex, navigationItems, resolvedOpen]);

  useEffect(() => {
    if (!resolvedOpen) return;
    const handleOutsidePointer = (event: MouseEvent) => {
      const target = event.target;
      if (!target || typeof (target as Node).nodeType !== "number") return;
      if (!rootRef.current?.contains(target as Node) && !popoverRef.current?.contains(target as Node)) {
        closeListbox(false);
      }
    };
    document.addEventListener("mousedown", handleOutsidePointer, true);
    return () => document.removeEventListener("mousedown", handleOutsidePointer, true);
  }, [closeListbox, resolvedOpen]);

  const layout = useAnchoredLayer({
    open: resolvedOpen && popoverMode === "overlay",
    anchorRef: triggerRef,
    layerRef: popoverRef,
    placement,
    matchWidth: matchTriggerWidth,
    revision: `${query}:${filteredOptions.length}:${loading}`,
  });

  useEffect(() => {
    if (!resolvedOpen || activeIndex < 0) return;
    document.getElementById(optionId(activeIndex))?.scrollIntoView?.({ block: "nearest" });
  }, [activeIndex, optionId, resolvedOpen]);

  const selectedOptions = useMemo<ComboboxOption[]>(() => selectedValues.map(selectedValue => (
    options.find(option => option.value === selectedValue)
      ?? { label: String(selectedValue), value: selectedValue }
  )), [options, selectedValues]);

  const renderedValue = useMemo(() => {
    const customValue = renderValue?.(multiple ? selectedOptions : selectedOptions[0]);
    if (customValue !== undefined && customValue !== null) return customValue;
    if (selectedOptions.length === 0) {
      return <span className={styles.placeholder}>{placeholder}</span>;
    }
    if (!multiple) {
      const selected = selectedOptions[0];
      const selectedLeading = selected?.leading ?? selected?.icon;
      return (
        <span className={styles.singleValue}>
          {selectedLeading && <span aria-hidden="true" className={styles.valueLeading}>{selectedLeading}</span>}
          <span className={styles.valueLabel}>{selected?.label}</span>
        </span>
      );
    }
    return (
      <span className={styles.valueLabel}>
        {selectedOptions.map(option => option.label).join(", ")}
      </span>
    );
  }, [multiple, placeholder, renderValue, selectedOptions]);

  const hasValue = selectedValues.length > 0;
  const allFilteredSelected = filteredOptions.some(option => !option.disabled)
    && filteredOptions
      .filter(option => !option.disabled)
      .every(option => selectedValues.includes(option.value));
  const groupedOptions = useMemo(() => {
    const ungrouped: ComboboxOption[] = [];
    const groups = new Map<string, ComboboxOption[]>();
    filteredOptions.forEach(option => {
      if (!option.group) {
        ungrouped.push(option);
        return;
      }
      const group = groups.get(option.group) ?? [];
      group.push(option);
      groups.set(option.group, group);
    });
    return { groups, ungrouped };
  }, [filteredOptions]);

  const renderListboxOption = (option: ComboboxOption) => {
    const navigationIndex = navigationItems.findIndex(item => (
      item.kind === "option" && item.option === option
    ));
    const selected = selectedValues.includes(option.value);
    return (
      <ListboxOption
        active={activeIndex === navigationIndex}
        aria-label={typeof option.description === "string"
          ? `${option.label} — ${option.description}`
          : option.label}
        description={renderOption ? undefined : option.description}
        disabled={option.disabled || loading}
        id={optionId(navigationIndex)}
        indicator={selected ? <Icon name="check-line" /> : null}
        key={`${typeof option.value}:${option.value}`}
        leading={renderOption ? undefined : option.leading ?? option.icon}
        metadata={renderOption ? undefined : option.metadata}
        onClick={() => selectOption(option)}
        onMouseDown={event => event.preventDefault()}
        selected={selected}
        value={option.value}
        data-testid={option.testId}
        {...option.testAttributes}
      >
        {renderOption ? renderOption(option) : option.label}
      </ListboxOption>
    );
  };

  const activeDescendant = resolvedOpen && activeIndex >= 0
    ? optionId(activeIndex)
    : undefined;
  const resolvedTriggerAriaLabel = triggerAriaLabel
    ?? rootAriaLabel
    ?? (label || providedId
      ? undefined
      : typeof placeholder === "string" ? placeholder : context.labels.placeholder);
  const resolvedTriggerLabelledBy = resolvedTriggerAriaLabel
    ? triggerAriaLabelledBy ?? rootAriaLabelledBy
    : triggerAriaLabelledBy ?? rootAriaLabelledBy ?? (label ? labelId : undefined);
  const resolvedTriggerDescribedBy = [
    triggerAriaDescribedBy ?? rootAriaDescribedBy,
    errorMessage ? errorId : undefined,
  ].filter(Boolean).join(" ") || undefined;
  const portalTarget = resolvedOpen && popoverMode === "overlay"
    ? resolveLayerPortal(portalContainer, triggerRef.current)
    : null;
  const popover = resolvedOpen ? (
    <div
      className={classNames(styles.popover, dropdownClassName)}
      data-bf-component="combobox-popup"
      data-bf-part="popover"
      data-keyboard-open={keyboardOpen ? "true" : "false"}
      data-placement={layout?.placement ?? placement}
      data-popover-mode={popoverMode}
      data-testid={dropdownTestId}
      ref={popoverRef}
      style={popoverMode === "overlay"
        ? layout?.style ?? { position: "fixed", visibility: "hidden" }
        : undefined}
    >
      {searchable && (
        <div className={styles.search} data-bf-part="search">
          <SearchField
            aria-activedescendant={activeDescendant}
            aria-autocomplete="list"
            aria-controls={listboxId}
            aria-expanded={resolvedOpen}
            aria-label={searchPlaceholder}
            autoComplete="off"
            clearLabel={clearLabel}
            onClear={() => {
              setQuery("");
              setActiveIndex(firstEnabledIndex(navigationItems, 1));
            }}
            onKeyDown={handleSearchKeyDown}
            onValueChange={(nextQuery) => {
              setQuery(nextQuery);
              setActiveIndex(-1);
            }}
            placeholder={searchPlaceholder}
            ref={setSearchInputRef}
            role="combobox"
            size={size}
            value={query}
          />
        </div>
      )}
      <Listbox
        aria-label={triggerAriaLabel ?? (typeof label === "string" ? label : "Options")}
        className={styles.listbox}
        focusMode="virtual"
        id={listboxId}
        multiple={multiple}
      >
        {loading && filteredOptions.length === 0 ? (
          <ListboxEmpty className={styles.message}>
            <LoaderCircle aria-hidden="true" className={styles.spinner} />
            <span>{loadingText}</span>
          </ListboxEmpty>
        ) : navigationItems.length === 0 ? (
          <ListboxEmpty>{emptyText}</ListboxEmpty>
        ) : (
          <>
            {navigationItems[0]?.kind === "all" && (
              <ListboxOption
                active={activeIndex === 0}
                id={optionId(0)}
                indicator={allFilteredSelected ? <Icon name="check-line" /> : null}
                onClick={toggleSelectAll}
                onMouseDown={event => event.preventDefault()}
                selected={allFilteredSelected}
              >
                {selectAllLabel}
              </ListboxOption>
            )}
            {groupedOptions.ungrouped.map(renderListboxOption)}
            {[...groupedOptions.groups].map(([groupLabel, groupOptions]) => (
              <ListboxGroup key={groupLabel} label={groupLabel}>
                {groupOptions.map(renderListboxOption)}
              </ListboxGroup>
            ))}
            {customCandidate && (() => {
              const customIndex = navigationItems.findIndex(item => item.kind === "custom");
              return (
                <ListboxOption
                  active={activeIndex === customIndex}
                  id={optionId(customIndex)}
                  leading={<Icon name="plus" />}
                  onClick={() => submitCustomValue(customCandidate)}
                  onMouseDown={event => event.preventDefault()}
                  selected={selectedValues.includes(customCandidate)}
                  value={customCandidate}
                >
                  {typeof customValueHint === "function"
                    ? customValueHint(customCandidate)
                    : <>{customValueHint}: {customCandidate}</>}
                </ListboxOption>
              );
            })()}
          </>
        )}
      </Listbox>
    </div>
  ) : null;

  return (
    <div
      {...rootProps}
      className={classNames(styles.root, className)}
      data-bf-component="combobox"
      data-disabled={disabled ? "true" : "false"}
      data-invalid={invalid ? "true" : "false"}
      data-multiple={multiple ? "true" : "false"}
      data-open={resolvedOpen ? "true" : "false"}
      data-size={size}
      ref={setRootRef}
    >
      {label !== undefined && label !== null && (
        <label className={styles.visibleLabel} data-bf-part="label" htmlFor={id} id={labelId}>
          {label}
        </label>
      )}
      <div
        className={styles.control}
        data-bf-part="control"
        data-tags={multiple && !renderValue && hasValue ? "true" : "false"}
      >
        {multiple && !renderValue && hasValue && (
          <span className={styles.tags} data-bf-part="tags">
            {selectedOptions.slice(0, Math.max(1, maxTagCount)).map(option => (
              <span className={styles.tag} data-bf-part="tag" key={`${typeof option.value}:${option.value}`}>
                <span>{option.label}</span>
                <IconButton
                  aria-label={`${clearLabel}: ${option.label}`}
                  disabled={disabled}
                  icon={<Icon name="xmark" />}
                  onClick={event => {
                    event.stopPropagation();
                    commitValue(selectedValues.filter(candidate => candidate !== option.value));
                  }}
                  onMouseDown={event => event.preventDefault()}
                  size="xs"
                  variant="quiet"
                />
              </span>
            ))}
            {selectedOptions.length > Math.max(1, maxTagCount) && (
              <span className={styles.tag}>+{selectedOptions.length - Math.max(1, maxTagCount)}</span>
            )}
          </span>
        )}
        <button
          aria-activedescendant={!searchable ? activeDescendant : undefined}
          aria-controls={resolvedOpen ? listboxId : undefined}
          aria-describedby={resolvedTriggerDescribedBy}
          aria-expanded={resolvedOpen}
          aria-haspopup="listbox"
          aria-invalid={invalid || undefined}
          aria-label={resolvedTriggerAriaLabel}
          aria-labelledby={resolvedTriggerLabelledBy}
          aria-required={required || undefined}
          aria-busy={loading || undefined}
          className={classNames(styles.trigger, triggerClassName)}
          data-bf-part="trigger"
          data-testid={triggerTestId}
          disabled={disabled}
          id={id}
          onClick={() => {
            if (resolvedOpen) closeListbox(false);
            else openListbox(1, false);
          }}
          onKeyDown={handleTriggerKeyDown}
          ref={triggerRef}
          role="combobox"
          type="button"
        >
          <span className={styles.value} data-bf-part="value">{renderedValue}</span>
        </button>
        {clearable && hasValue && !disabled && (
          <IconButton
            aria-label={clearLabel}
            className={styles.clear}
            icon={<Icon name="xmark" />}
            onClick={event => {
              event.stopPropagation();
              commitValue(multiple ? [] : "");
              setQuery("");
            }}
            size="sm"
            variant="quiet"
          />
        )}
        <span aria-hidden="true" className={styles.indicator} data-bf-part="indicator">
          {indicator ?? <Icon name="chevron-down" />}
        </span>
      </div>
      {errorMessage ? (
        <span className={styles.error} data-bf-part="message" id={errorId}>
          {errorMessage}
        </span>
      ) : null}
      {popover && (portalTarget ? createPortal(popover, portalTarget) : popover)}
    </div>
  );
});
