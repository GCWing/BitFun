/**
 * NumberInput component
 * Supports step buttons and direct input
 */

import React, { forwardRef, useState, useCallback, useRef, useEffect } from 'react';
import { ChevronUp, ChevronDown, Minus, Plus } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import './NumberInput.scss';

export interface NumberInputProps {
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  unit?: string;
  disabled?: boolean;
  size?: 'small' | 'medium' | 'large';
  variant?: 'default' | 'compact' | 'stepper';
  showButtons?: boolean;
  precision?: number;
  className?: string;
  label?: string;
  draggable?: boolean;
  disableWheel?: boolean;
}

export const NumberInput = forwardRef<HTMLInputElement, NumberInputProps>(
  (
    {
      value,
      onChange,
      min = -Infinity,
      max = Infinity,
      step = 1,
      unit,
      disabled = false,
      size = 'medium',
      variant = 'default',
      showButtons = true,
      precision = 0,
      className = '',
      label,
      draggable = false,
      disableWheel = false,
    },
    ref
  ) => {
    const { t } = useI18n('components');
    const [isEditing, setIsEditing] = useState(false);
    const [inputValue, setInputValue] = useState(String(value));
    const [isDragging, setIsDragging] = useState(false);
    const [isHovered, setIsHovered] = useState(false);
    const dragStartRef = useRef<{ y: number; value: number } | null>(null);
    const inputRef = useRef<HTMLInputElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);

    const formatValue = useCallback(
      (val: number) => {
        return precision > 0 ? val.toFixed(precision) : String(Math.round(val));
      },
      [precision]
    );

    useEffect(() => {
      if (!isEditing) {
        setInputValue(formatValue(value));
      }
    }, [value, isEditing, formatValue]);

    const clampValue = useCallback(
      (val: number) => Math.min(max, Math.max(min, val)),
      [min, max]
    );

    const increment = useCallback(() => {
      if (disabled) return;
      const newValue = clampValue(value + step);
      onChange(newValue);
    }, [value, step, clampValue, onChange, disabled]);

    const decrement = useCallback(() => {
      if (disabled) return;
      const newValue = clampValue(value - step);
      onChange(newValue);
    }, [value, step, clampValue, onChange, disabled]);

    const handleInputChange = useCallback(
      (e: React.ChangeEvent<HTMLInputElement>) => {
        setInputValue(e.target.value);
      },
      []
    );

    const handleInputBlur = useCallback(() => {
      setIsEditing(false);
      const parsed = parseFloat(inputValue);
      if (!isNaN(parsed)) {
        const clamped = clampValue(parsed);
        onChange(clamped);
        setInputValue(formatValue(clamped));
      } else {
        setInputValue(formatValue(value));
      }
    }, [inputValue, value, clampValue, onChange, formatValue]);

    const handleKeyDown = useCallback(
      (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Enter') {
          handleInputBlur();
          inputRef.current?.blur();
        } else if (e.key === 'Escape') {
          setIsEditing(false);
          setInputValue(formatValue(value));
          inputRef.current?.blur();
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          increment();
        } else if (e.key === 'ArrowDown') {
          e.preventDefault();
          decrement();
        }
      },
      [handleInputBlur, formatValue, value, increment, decrement]
    );

    const handleDragStart = useCallback(
      (e: React.MouseEvent) => {
        if (!draggable || disabled) return;
        e.preventDefault();
        setIsDragging(true);
        dragStartRef.current = { y: e.clientY, value };
        document.body.style.cursor = 'ns-resize';
      },
      [draggable, disabled, value]
    );

    useEffect(() => {
      if (!isDragging) return;

      const handleMouseMove = (e: MouseEvent) => {
        if (!dragStartRef.current) return;
        const delta = dragStartRef.current.y - e.clientY;
        const steps = Math.round(delta / 5);
        const newValue = clampValue(dragStartRef.current.value + steps * step);
        onChange(newValue);
      };

      const handleMouseUp = () => {
        setIsDragging(false);
        dragStartRef.current = null;
        document.body.style.cursor = '';
      };

      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);

      return () => {
        document.removeEventListener('mousemove', handleMouseMove);
        document.removeEventListener('mouseup', handleMouseUp);
      };
    }, [isDragging, step, clampValue, onChange]);

    const handleWheel = useCallback(
      (e: React.WheelEvent) => {
        if (disabled || disableWheel || !isHovered) return;
        e.preventDefault();
        if (e.deltaY < 0) {
          increment();
        } else {
          decrement();
        }
      },
      [disabled, disableWheel, isHovered, increment, decrement]
    );

    const containerClassName = [
      'bitfun-number-input',
      `bitfun-number-input--${size}`,
      `bitfun-number-input--${variant}`,
      disabled && 'bitfun-number-input--disabled',
      isDragging && 'bitfun-number-input--dragging',
      isEditing && 'bitfun-number-input--editing',
      className,
    ]
      .filter(Boolean)
      .join(' ');

    return (
      <div className={containerClassName}>
        {label && <label className="bitfun-number-input__label">{label}</label>}
        <div
          ref={containerRef}
          className="bitfun-number-input__container"
          onMouseEnter={() => setIsHovered(true)}
          onMouseLeave={() => setIsHovered(false)}
          onWheel={handleWheel}
        >
          <div className="bitfun-number-input__glow" />

          <div
            className="bitfun-number-input__value-area"
            onMouseDown={draggable ? handleDragStart : undefined}
            style={{ cursor: draggable && !disabled ? 'ns-resize' : 'text' }}
          >
            <input
              ref={(node) => {
                if (typeof ref === 'function') {
                  ref(node);
                } else if (ref) {
                  ref.current = node;
                }
                (inputRef as React.MutableRefObject<HTMLInputElement | null>).current = node;
              }}
              type="text"
              inputMode="decimal"
              className="bitfun-number-input__input"
              value={inputValue}
              onChange={handleInputChange}
              onFocus={() => setIsEditing(true)}
              onBlur={handleInputBlur}
              onKeyDown={handleKeyDown}
              disabled={disabled}
            />
            {unit && <span className="bitfun-number-input__unit">{unit}</span>}
          </div>

          {showButtons && variant !== 'compact' && (
            <div className="bitfun-number-input__buttons">
              {variant === 'stepper' ? (
                <>
                  <button
                    type="button"
                    className="bitfun-number-input__btn bitfun-number-input__btn--minus"
                    onClick={decrement}
                    disabled={disabled || value <= min}
                    tabIndex={-1}
                    aria-label={t('numberInput.decrease')}
                  >
                    <Minus size={12} strokeWidth={2.5} />
                  </button>
                  <button
                    type="button"
                    className="bitfun-number-input__btn bitfun-number-input__btn--plus"
                    onClick={increment}
                    disabled={disabled || value >= max}
                    tabIndex={-1}
                    aria-label={t('numberInput.increase')}
                  >
                    <Plus size={12} strokeWidth={2.5} />
                  </button>
                </>
              ) : (
                <>
                  <button
                    type="button"
                    className="bitfun-number-input__btn bitfun-number-input__btn--up"
                    onClick={increment}
                    disabled={disabled || value >= max}
                    tabIndex={-1}
                    aria-label={t('numberInput.increase')}
                  >
                    <ChevronUp size={14} strokeWidth={2.5} />
                  </button>
                  <button
                    type="button"
                    className="bitfun-number-input__btn bitfun-number-input__btn--down"
                    onClick={decrement}
                    disabled={disabled || value <= min}
                    tabIndex={-1}
                    aria-label={t('numberInput.decrease')}
                  >
                    <ChevronDown size={14} strokeWidth={2.5} />
                  </button>
                </>
              )}
            </div>
          )}

          {min !== -Infinity && max !== Infinity && (
            <div className="bitfun-number-input__progress">
              <div
                className="bitfun-number-input__progress-bar"
                style={{ width: `${((value - min) / (max - min)) * 100}%` }}
              />
            </div>
          )}
        </div>
      </div>
    );
  }
);

NumberInput.displayName = 'NumberInput';
