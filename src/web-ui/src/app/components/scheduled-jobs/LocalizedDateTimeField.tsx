/**
 * Date-time entry whose field order follows the app locale.
 *
 * A native `<input type="datetime-local">` renders in the *browser's* locale,
 * which Chromium takes from the OS and does not read from the `lang`
 * attribute. Running the UI in Chinese on an English system therefore shows
 * `08/12/2026, 04:48 AM` inside an otherwise Chinese form.
 *
 * This is an ordinary text input so it looks and behaves like every other field
 * in the form, showing `2026/08/12 04:48` in Chinese and `08/12/2026 04:48` in
 * English. A calendar button opens the browser's own picker for anyone who
 * would rather click than type. Callers keep the native
 * `YYYY-MM-DDTHH:mm` value contract.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { CalendarDays } from 'lucide-react';
import { IconButton, Input } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import {
  dateTimeFormatHint,
  formatDateTimeText,
  parseDateTimeText,
  resolveDateFieldOrder,
} from './localizedDateTime';

export interface LocalizedDateTimeFieldProps {
  /** `YYYY-MM-DDTHH:mm`, matching the native datetime-local value. */
  value: string;
  onChange: (value: string) => void;
  error?: boolean;
  disabled?: boolean;
  className?: string;
  'aria-label'?: string;
}

const LocalizedDateTimeField: React.FC<LocalizedDateTimeFieldProps> = ({
  value,
  onChange,
  error = false,
  disabled = false,
  className,
  'aria-label': ariaLabel,
}) => {
  const { t, currentLanguage } = useI18n('common');
  const pickerRef = useRef<HTMLInputElement | null>(null);
  const editingRef = useRef(false);

  const order = useMemo(() => resolveDateFieldOrder(currentLanguage), [currentLanguage]);

  // Local text so partial input survives; a fully controlled field would reject
  // every keystroke until the whole date parsed, which makes it untypable.
  const [text, setText] = useState(() => formatDateTimeText(value, order));

  // Adopt external changes (loading a job, switching locale) unless the user is
  // mid-edit, where overwriting their keystrokes would fight them.
  useEffect(() => {
    if (editingRef.current) return;
    setText(formatDateTimeText(value, order));
  }, [order, value]);

  const commitText = useCallback((nextText: string) => {
    setText(nextText);

    if (!nextText.trim()) {
      onChange('');
      return;
    }

    const parsed = parseDateTimeText(nextText, order);
    if (parsed) onChange(parsed);
  }, [onChange, order]);

  const handleBlur = useCallback(() => {
    editingRef.current = false;

    // Normalize whatever parsed into the canonical rendering, so `2026/8/2 4:8`
    // settles as `2026/08/02 04:08` instead of staying half-typed.
    const parsed = text.trim() ? parseDateTimeText(text, order) : null;
    setText(parsed ? formatDateTimeText(parsed, order) : text);
  }, [order, text]);

  const openPicker = useCallback(() => {
    const picker = pickerRef.current;
    if (!picker) return;
    picker.value = value;
    picker.showPicker?.();
  }, [value]);

  const canOpenPicker = typeof HTMLInputElement !== 'undefined'
    && 'showPicker' in HTMLInputElement.prototype;

  return (
    <div
      className={['bf-datetime-field', className ?? ''].filter(Boolean).join(' ')}
      data-bf-component="localized-datetime-field"
      data-bf-part="root"
      data-bf-state={error ? 'error' : undefined}
    >
      <Input
        size="small"
        value={text}
        error={error}
        disabled={disabled}
        inputMode="numeric"
        placeholder={dateTimeFormatHint(order)}
        aria-label={ariaLabel ?? t('dateTimeField.label')}
        className="bf-datetime-field__input"
        onFocus={() => { editingRef.current = true; }}
        onChange={event => commitText(event.currentTarget.value)}
        onBlur={handleBlur}
      />

      {canOpenPicker ? (
        <>
          <IconButton
            type="button"
            size="xs"
            disabled={disabled}
            aria-label={t('dateTimeField.openPicker')}
            tooltip={t('dateTimeField.openPicker')}
            onClick={openPicker}
          >
            <CalendarDays size={14} />
          </IconButton>
          {/*
            Only ever opened programmatically: the browser picker is a good
            input surface, its inline text rendering is the part we replaced.
          */}
          <input
            ref={pickerRef}
            type="datetime-local"
            className="bf-datetime-field__picker"
            tabIndex={-1}
            aria-hidden="true"
            onChange={event => {
              editingRef.current = false;
              const picked = event.currentTarget.value;
              setText(formatDateTimeText(picked, order));
              onChange(picked);
            }}
          />
        </>
      ) : null}
    </div>
  );
};

export default LocalizedDateTimeField;
