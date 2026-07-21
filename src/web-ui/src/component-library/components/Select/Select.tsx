/**
 * Select dropdown component
 */

import React, {
  useState,
  useRef,
  useEffect,
  useLayoutEffect,
  useMemo,
  useCallback,
} from 'react';
import { useI18n } from '@/infrastructure/i18n';
import './Select.scss';

export interface SelectOption {
  label: string;
  value: string | number;
  disabled?: boolean;
  description?: string;
  icon?: React.ReactNode;
  group?: string;
  testId?: string;
  testAttributes?: Record<`data-${string}`, string | number | boolean | undefined>;
}

export interface SelectProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'defaultValue' | 'onChange'> {
  options?: SelectOption[];
  value?: string | number | (string | number)[];
  defaultValue?: string | number | (string | number)[];
  placeholder?: string;
  disabled?: boolean;
  onChange?: (value: string | number | (string | number)[]) => void;
  size?: 'small' | 'medium' | 'large';
  label?: string;
  multiple?: boolean;
  searchable?: boolean;
  clearable?: boolean;
  showSelectAll?: boolean;
  loading?: boolean;
  error?: boolean;
  errorMessage?: string;
  maxTagCount?: number;
  searchPlaceholder?: string;
  emptyText?: string;
  renderOption?: (option: SelectOption) => React.ReactNode;
  renderValue?: (option?: SelectOption | SelectOption[]) => React.ReactNode;
  className?: string;
  placement?: 'bottom' | 'top';
  autoClose?: boolean;
  allowCustomValue?: boolean;
  customValueHint?: string;
  onOpenChange?: (isOpen: boolean) => void;
  triggerTestId?: string;
  dropdownTestId?: string;
  triggerAriaLabel?: string;
  triggerAriaLabelledBy?: string;
}

export const Select: React.FC<SelectProps> = ({
  options = [],
  value,
  defaultValue,
  placeholder,
  disabled = false,
  onChange,
  size = 'medium',
  label,
  multiple = false,
  searchable = false,
  clearable = false,
  showSelectAll = false,
  loading = false,
  error = false,
  errorMessage,
  maxTagCount = 3,
  searchPlaceholder,
  emptyText,
  renderOption,
  renderValue,
  className = '',
  placement = 'bottom',
  autoClose = false,
  allowCustomValue = false,
  customValueHint,
  onOpenChange,
  triggerTestId,
  dropdownTestId,
  triggerAriaLabel,
  triggerAriaLabelledBy,
  ...rootProps
}) => {
  const { t } = useI18n('components');
  const baseId = React.useId();
  const labelId = `${baseId}-label`;
  const listboxId = `${baseId}-listbox`;
  
  // Resolve i18n default values
  const resolvedPlaceholder = placeholder ?? t('select.placeholder');
  const resolvedSearchPlaceholder = searchPlaceholder ?? t('select.search');
  const resolvedEmptyText = emptyText ?? t('select.emptyText');
  const resolvedCustomValueHint = customValueHint ?? t('select.customValueHint');
  const [isOpen, setIsOpen] = useState(false);
  const [resolvedPlacement, setResolvedPlacement] = useState<'bottom' | 'top'>(placement);
  const [selectedValue, setSelectedValue] = useState<string | number | (string | number)[]>(
    value !== undefined ? value : defaultValue !== undefined ? defaultValue : multiple ? [] : ''
  );
  const [searchQuery, setSearchQuery] = useState('');
  const [highlightedIndex, setHighlightedIndex] = useState(-1);
  const hasMountedRef = useRef(false);
  
  const selectRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const isKeyboardNavigation = useRef(false);

  useEffect(() => {
    if (value !== undefined) {
      setSelectedValue(value);
    }
  }, [value]);

  const filteredOptions = useMemo(() => {
    if (!searchQuery || !searchable) return options;
    const query = searchQuery.toLowerCase();
    return options.filter(opt => 
      opt.label.toLowerCase().includes(query) ||
      String(opt.value).toLowerCase().includes(query) ||
      opt.description?.toLowerCase().includes(query)
    );
  }, [options, searchQuery, searchable]);

  useLayoutEffect(() => {
    if (!isOpen) {
      setResolvedPlacement(placement);
      return;
    }

    const selectElement = selectRef.current;
    const dropdownElement = dropdownRef.current;
    if (!selectElement || !dropdownElement || typeof window === 'undefined') {
      setResolvedPlacement(placement);
      return;
    }

    const triggerRect = selectElement.getBoundingClientRect();
    const dropdownHeight = dropdownElement.offsetHeight || dropdownElement.scrollHeight || 240;
    const spaceBelow = window.innerHeight - triggerRect.bottom;
    const spaceAbove = triggerRect.top;

    let nextPlacement: 'bottom' | 'top' = placement;
    if (placement === 'bottom' && spaceBelow < dropdownHeight && spaceAbove > spaceBelow) {
      nextPlacement = 'top';
    } else if (placement === 'top' && spaceAbove < dropdownHeight && spaceBelow > spaceAbove) {
      nextPlacement = 'bottom';
    }

    setResolvedPlacement(nextPlacement);
  }, [
    isOpen,
    placement,
    options.length,
    searchable,
    multiple,
    showSelectAll,
    allowCustomValue,
    searchQuery,
    filteredOptions.length,
  ]);

  const groupedOptions = useMemo(() => {
    const groups: { [key: string]: SelectOption[] } = {};
    const ungrouped: SelectOption[] = [];
    
    filteredOptions.forEach(opt => {
      if (opt.group) {
        if (!groups[opt.group]) {
          groups[opt.group] = [];
        }
        groups[opt.group].push(opt);
      } else {
        ungrouped.push(opt);
      }
    });
    
    return { groups, ungrouped, hasGroups: Object.keys(groups).length > 0 };
  }, [filteredOptions]);

  const displayOptions = useMemo(() => [
    ...groupedOptions.ungrouped,
    ...Object.values(groupedOptions.groups).flat(),
  ], [groupedOptions]);

  const isSelected = useCallback((optionValue: string | number) => {
    if (multiple) {
      return (selectedValue as (string | number)[]).includes(optionValue);
    }
    return selectedValue === optionValue;
  }, [selectedValue, multiple]);

  const selectedOptions = useMemo(() => {
    if (multiple) {
      return options.filter(opt => 
        (selectedValue as (string | number)[]).includes(opt.value)
      );
    }
    return options.find(opt => opt.value === selectedValue);
  }, [selectedValue, options, multiple]);

  const handleSelect = useCallback((option: SelectOption) => {
    if (option.disabled) return;

    let newValue: string | number | (string | number)[];
    
    if (multiple) {
      const currentValues = selectedValue as (string | number)[];
      if (currentValues.includes(option.value)) {
        newValue = currentValues.filter(v => v !== option.value);
      } else {
        newValue = [...currentValues, option.value];
      }
      setSelectedValue(newValue);
      onChange?.(newValue);
      
      if (autoClose && newValue.length > 0) {
        setIsOpen(false);
        setSearchQuery('');
      }
    } else {
      newValue = option.value;
      setSelectedValue(newValue);
      onChange?.(newValue);
      setIsOpen(false);
      setSearchQuery('');
    }
    
    setHighlightedIndex(-1);
  }, [selectedValue, multiple, onChange, autoClose]);

  const handleSelectAll = useCallback(() => {
    if (!multiple) return;
    
    const currentValues = selectedValue as (string | number)[];
    const availableOptions = filteredOptions.filter(opt => !opt.disabled);
    const availableValues = availableOptions.map(opt => opt.value);
    
    const allSelected = availableValues.every(v => currentValues.includes(v));
    
    let newValue: (string | number)[];
    if (allSelected) {
      newValue = currentValues.filter(v => !availableValues.includes(v));
    } else {
      newValue = [...new Set([...currentValues, ...availableValues])];
    }
    
    setSelectedValue(newValue);
    onChange?.(newValue);
  }, [multiple, selectedValue, filteredOptions, onChange]);

  const handleClear = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    const newValue = multiple ? [] : '';
    setSelectedValue(newValue);
    onChange?.(newValue);
    setSearchQuery('');
  }, [multiple, onChange]);

  const handleCustomValueSubmit = useCallback(() => {
    if (!allowCustomValue || !searchQuery.trim()) return false;
    
    const trimmedValue = searchQuery.trim();
    const existingOption = options.find(opt => 
      opt.value === trimmedValue || opt.label.toLowerCase() === trimmedValue.toLowerCase()
    );
    
    if (existingOption) {
      handleSelect(existingOption);
    } else if (multiple) {
      const currentValues = selectedValue as (string | number)[];
      if (!currentValues.includes(trimmedValue)) {
        const newValue = [...currentValues, trimmedValue];
        setSelectedValue(newValue);
        onChange?.(newValue);
      }
      setIsOpen(false);
      setSearchQuery('');
    } else {
      setSelectedValue(trimmedValue);
      onChange?.(trimmedValue);
      setIsOpen(false);
      setSearchQuery('');
    }
    return true;
  }, [allowCustomValue, multiple, searchQuery, options, handleSelect, selectedValue, onChange]);

  const moveHighlight = useCallback((current: number, direction: 1 | -1) => {
    if (displayOptions.length === 0) return -1;
    let index = current;
    for (let count = 0; count < displayOptions.length; count += 1) {
      index += direction;
      if (index < 0) index = displayOptions.length - 1;
      if (index >= displayOptions.length) index = 0;
      if (!displayOptions[index]?.disabled) return index;
    }
    return -1;
  }, [displayOptions]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (disabled) return;

    switch (e.key) {
      case 'Enter':
        e.preventDefault();
        if (!isOpen) {
          setIsOpen(true);
        } else if (highlightedIndex >= 0 && highlightedIndex < displayOptions.length) {
          handleSelect(displayOptions[highlightedIndex]);
        } else if (allowCustomValue && searchQuery.trim()) {
          handleCustomValueSubmit();
        }
        break;
        
      case 'Escape':
        e.preventDefault();
        setIsOpen(false);
        setSearchQuery('');
        break;
        
      case 'ArrowDown':
        e.preventDefault();
        isKeyboardNavigation.current = true;
        if (!isOpen) {
          setIsOpen(true);
          setHighlightedIndex(moveHighlight(-1, 1));
        } else {
          setHighlightedIndex((previous) => moveHighlight(previous, 1));
        }
        break;
        
      case 'ArrowUp':
        e.preventDefault();
        isKeyboardNavigation.current = true;
        if (isOpen) {
          setHighlightedIndex((previous) => moveHighlight(
            previous < 0 ? displayOptions.length : previous,
            -1,
          ));
        }
        break;
        
      case 'Tab':
        if (isOpen) {
          if (allowCustomValue && searchQuery.trim()) {
            handleCustomValueSubmit();
          }
          setIsOpen(false);
        }
        break;
    }
  }, [disabled, isOpen, highlightedIndex, displayOptions, handleSelect, allowCustomValue,
    searchQuery, handleCustomValueSubmit, moveHighlight]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (selectRef.current && !selectRef.current.contains(event.target as Node)) {
        if (allowCustomValue && !multiple && searchQuery.trim()) {
          const trimmedValue = searchQuery.trim();
          const existingOption = options.find(opt => 
            opt.value === trimmedValue || opt.label.toLowerCase() === trimmedValue.toLowerCase()
          );
          if (existingOption) {
            setSelectedValue(existingOption.value);
            onChange?.(existingOption.value);
          } else {
            setSelectedValue(trimmedValue);
            onChange?.(trimmedValue);
          }
        }
        setIsOpen(false);
        setSearchQuery('');
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [allowCustomValue, multiple, searchQuery, options, onChange]);

  useEffect(() => {
    if (!hasMountedRef.current) {
      hasMountedRef.current = true;
      return;
    }
    onOpenChange?.(isOpen);
  }, [isOpen, onOpenChange]);

  useEffect(() => {
    if (isOpen && searchable && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [isOpen, searchable]);

  useEffect(() => {
    if (highlightedIndex >= 0 && dropdownRef.current && isKeyboardNavigation.current) {
      const highlightedElement = document.getElementById(`${listboxId}-option-${highlightedIndex}`);
      highlightedElement?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      isKeyboardNavigation.current = false;
    }
  }, [highlightedIndex, listboxId]);

  const classNames = [
    'select',
    `select--${size}`,
    `select--placement-${resolvedPlacement}`,
    isOpen && 'select--open',
    disabled && 'select--disabled',
    error && 'select--error',
    multiple && 'select--multiple',
    className
  ]
    .filter(Boolean)
    .join(' ');

  const renderSelectedValue = () => {
    if (renderValue) {
      const customRenderedValue = renderValue(selectedOptions);
      if (customRenderedValue) {
        return customRenderedValue;
      }
    }

    if (multiple) {
      const selected = selectedOptions as SelectOption[];
      if (selected.length === 0) {
        return <span className="select__placeholder">{resolvedPlaceholder}</span>;
      }
      
      const displayTags = selected.slice(0, maxTagCount);
      const remaining = selected.length - maxTagCount;
      
      return (
        <div className="select__tags">
          {displayTags.map(opt => (
            <span key={opt.value} className="select__tag">
              {opt.icon && <span className="select__tag-icon">{opt.icon}</span>}
              <span className="select__tag-label">{opt.label}</span>
              <span 
                className="select__tag-remove"
                onClick={(e) => {
                  e.stopPropagation();
                  handleSelect(opt);
                }}
              >
                ×
              </span>
            </span>
          ))}
          {remaining > 0 && (
            <span className="select__tag select__tag--more">+{remaining}</span>
          )}
        </div>
      );
    } else {
      const selected = selectedOptions as SelectOption | undefined;
      if (!selected) {
        if (allowCustomValue && selectedValue && selectedValue !== '') {
          return (
            <span className="select__value">
              <span className="select__value-label select__value-label--custom">{String(selectedValue)}</span>
            </span>
          );
        }
        return <span className="select__placeholder">{resolvedPlaceholder}</span>;
      }
      return (
        <span className="select__value">
          {selected.icon && <span className="select__value-icon">{selected.icon}</span>}
          <span className="select__value-label">{selected.label}</span>
        </span>
      );
    }
  };

  const renderOptionItem = (option: SelectOption, index: number) => {
    const selected = isSelected(option.value);
    const highlighted = index === highlightedIndex;
    
    return (
      <div
        id={`${listboxId}-option-${index}`}
        key={option.value}
        className={`select__option ${selected ? 'select__option--selected' : ''} ${
          option.disabled ? 'select__option--disabled' : ''
        } ${highlighted ? 'select__option--highlighted' : ''}`}
        onClick={() => handleSelect(option)}
        onMouseEnter={() => {
          if (!option.disabled) setHighlightedIndex(index);
        }}
        role="option"
        aria-selected={selected}
        aria-disabled={option.disabled}
        data-selected={selected ? 'true' : 'false'}
        data-testid={option.testId}
        {...option.testAttributes}
      >
        {multiple && (
          <span className={`select__checkbox ${selected ? 'select__checkbox--checked' : ''}`}>
            {selected && '✓'}
          </span>
        )}
        
        {renderOption ? renderOption(option) : (
          <div className="select__option-content">
            {option.icon && <span className="select__option-icon">{option.icon}</span>}
            <div className="select__option-text">
              <div className="select__option-label">{option.label}</div>
              {option.description && (
                <div className="select__option-description">{option.description}</div>
              )}
            </div>
          </div>
        )}
      </div>
    );
  };

  return (
    <div {...rootProps} className={classNames} ref={selectRef}>
      {label && <div id={labelId} className="select__label">{label}</div>}
      
      <div
        className="select__trigger"
        onClick={() => !disabled && setIsOpen(!isOpen)}
        onKeyDown={handleKeyDown}
        tabIndex={disabled ? -1 : 0}
        role="combobox"
        aria-expanded={isOpen}
        aria-haspopup="listbox"
        aria-controls={listboxId}
        aria-activedescendant={isOpen && highlightedIndex >= 0
          ? `${listboxId}-option-${highlightedIndex}`
          : undefined}
        aria-disabled={disabled}
        aria-label={triggerAriaLabel}
        aria-labelledby={triggerAriaLabelledBy ?? (label ? labelId : undefined)}
        data-testid={triggerTestId}
      >
        {renderSelectedValue()}
        
        <div className="select__suffix">
          {loading && (
            <span className="select__loading">
              <span className="select__loading-spinner" />
            </span>
          )}
          {clearable && !loading && (multiple ? (selectedValue as any[]).length > 0 : selectedValue) && (
            <span className="select__clear" onClick={handleClear}>×</span>
          )}
          <span className={`select__arrow ${isOpen ? 'select__arrow--open' : ''}`}>
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
              <path d="M2 4L6 8L10 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
          </span>
        </div>
      </div>

      {isOpen && (
        <div
          id={listboxId}
          className={`select__dropdown select__dropdown--${resolvedPlacement}`}
          ref={dropdownRef}
          role="listbox"
          data-testid={dropdownTestId}
        >
          {searchable && (
            <div className="select__search">
              <input
                ref={searchInputRef}
                type="text"
                className="select__search-input"
                role="combobox"
                aria-label={resolvedSearchPlaceholder}
                aria-autocomplete="list"
                aria-expanded={isOpen}
                aria-controls={listboxId}
                aria-activedescendant={highlightedIndex >= 0
                  ? `${listboxId}-option-${highlightedIndex}`
                  : undefined}
                placeholder={resolvedSearchPlaceholder}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                onClick={(e) => e.stopPropagation()}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault();
                    if (highlightedIndex >= 0 && highlightedIndex < displayOptions.length) {
                      handleSelect(displayOptions[highlightedIndex]);
                    } else if (allowCustomValue && searchQuery.trim()) {
                      handleCustomValueSubmit();
                    }
                  } else if (e.key === 'Escape') {
                    e.preventDefault();
                    setIsOpen(false);
                    setSearchQuery('');
                  } else if (e.key === 'ArrowDown') {
                    e.preventDefault();
                    isKeyboardNavigation.current = true;
                    setHighlightedIndex((previous) => moveHighlight(previous, 1));
                  } else if (e.key === 'ArrowUp') {
                    e.preventDefault();
                    isKeyboardNavigation.current = true;
                    setHighlightedIndex((previous) => moveHighlight(
                      previous < 0 ? displayOptions.length : previous,
                      -1,
                    ));
                  }
                }}
              />
              {searchQuery && (
                <span
                  className="select__search-clear"
                  onClick={(e) => {
                    e.stopPropagation();
                    setSearchQuery('');
                    setHighlightedIndex(-1);
                    searchInputRef.current?.focus();
                  }}
                >
                  ×
                </span>
              )}
            </div>
          )}
          
          {multiple && showSelectAll && filteredOptions.length > 0 && (
            <div className="select__select-all" onClick={handleSelectAll}>
              <span className={`select__checkbox ${
                filteredOptions.filter(opt => !opt.disabled).every(opt => isSelected(opt.value))
                  ? 'select__checkbox--checked' : ''
              }`}>
                {filteredOptions.filter(opt => !opt.disabled).every(opt => isSelected(opt.value)) && '✓'}
              </span>
              <span>{t('select.selectAll')}</span>
            </div>
          )}
          
          <div className="select__options">
            {filteredOptions.length === 0 ? (
              loading ? (
                <div className="select__empty select__empty--loading">
                  <span className="select__loading-spinner" aria-hidden="true" />
                  <span>{t('select.loading')}</span>
                </div>
              ) : allowCustomValue && searchQuery.trim() ? (
                <div 
                  className="select__custom-value-hint"
                  onClick={() => handleCustomValueSubmit()}
                >
                  <span className="select__custom-value-text">"{searchQuery.trim()}"</span>
                  <span className="select__custom-value-action">{resolvedCustomValueHint}</span>
                </div>
              ) : (
                <div className="select__empty">{resolvedEmptyText}</div>
              )
            ) : groupedOptions.hasGroups ? (
              (() => {
                let globalIndex = 0;
                return (
                  <>
                    {groupedOptions.ungrouped.map((option) => 
                      renderOptionItem(option, globalIndex++)
                    )}
                    {Object.entries(groupedOptions.groups).map(([groupName, groupOptions]) => (
                      <div
                        key={groupName}
                        className="select__group"
                        role="group"
                        aria-label={groupName}
                      >
                        <div className="select__group-label">{groupName}</div>
                        {groupOptions.map((option) => 
                          renderOptionItem(option, globalIndex++)
                        )}
                      </div>
                    ))}
                  </>
                );
              })()
            ) : (
              <>
                {filteredOptions.map((option, index) => renderOptionItem(option, index))}
                {allowCustomValue && searchQuery.trim() && 
                 !filteredOptions.some(opt => (
                   opt.label.toLowerCase() === searchQuery.trim().toLowerCase() ||
                   String(opt.value).toLowerCase() === searchQuery.trim().toLowerCase()
                 )) && (
                  <div 
                    className="select__custom-value-hint"
                    onClick={() => handleCustomValueSubmit()}
                  >
                    <span className="select__custom-value-text">"{searchQuery.trim()}"</span>
                    <span className="select__custom-value-action">{resolvedCustomValueHint}</span>
                  </div>
                )}
              </>
            )}
          </div>
        </div>
      )}
      
      {error && errorMessage && (
        <div className="select__error-message">{errorMessage}</div>
      )}
    </div>
  );
};
