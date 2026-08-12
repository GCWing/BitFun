/**
 * Date-time entry whose field order follows the app locale.
 *
 * A native `<input type="datetime-local">` renders in the *browser's* locale,
 * which Chromium takes from the OS and does not read from the `lang`
 * attribute. Running the UI in Chinese on an English system therefore shows
 * `08/12/2026, 04:48 AM` inside an otherwise Chinese form. This control keeps
 * the same `YYYY-MM-DDTHH:mm` value contract as the native input so callers are
 * unchanged, but lays the segments out in the order the active locale reads.
 */

import React, { useCallback, useMemo, useRef } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import {
  SEGMENTS,
  clampSegmentToRange,
  composeDateTimeValue,
  parseDateTimeValue,
  resolveDateSegmentOrder,
  type ParsedValue,
  type SegmentId,
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
  const containerRef = useRef<HTMLDivElement | null>(null);

  const parsed = useMemo(() => parseDateTimeValue(value), [value]);
  const dateOrder = useMemo(() => resolveDateSegmentOrder(currentLanguage), [currentLanguage]);

  const emit = useCallback((next: ParsedValue) => {
    const composed = composeDateTimeValue(next);
    // Hold the edit until every segment is filled; a partial date has no
    // meaningful timestamp and clearing one field should not wipe the value.
    if (composed !== null) onChange(composed);
  }, [onChange]);

  const handleSegmentChange = useCallback((id: SegmentId, raw: string) => {
    const spec = SEGMENTS[id];
    const digits = raw.replace(/\D/g, '').slice(0, spec.length);
    emit({ ...parsed, [id]: digits });

    // Advance once a segment is unambiguously complete, so typing flows across
    // the row the way the native control does.
    if (digits.length === spec.length) {
      const inputs = containerRef.current?.querySelectorAll<HTMLInputElement>('input');
      if (!inputs) return;
      const current = Array.from(inputs).findIndex(input => input.dataset.segment === id);
      inputs[current + 1]?.focus();
    }
  }, [emit, parsed]);

  const handleSegmentBlur = useCallback((id: SegmentId, raw: string) => {
    const normalized = clampSegmentToRange(raw, SEGMENTS[id]);
    if (!normalized) return;
    emit({ ...parsed, [id]: normalized });
  }, [emit, parsed]);

  const renderSegment = (id: SegmentId) => {
    const spec = SEGMENTS[id];
    return (
      <input
        key={id}
        type="text"
        inputMode="numeric"
        data-segment={id}
        className={`bf-datetime-field__segment bf-datetime-field__segment--${id}`}
        value={parsed[id]}
        disabled={disabled}
        maxLength={spec.length}
        size={spec.length}
        placeholder={t(`dateTimeField.segments.${id}`)}
        aria-label={t(`dateTimeField.segments.${id}`)}
        onChange={event => handleSegmentChange(id, event.currentTarget.value)}
        onBlur={event => handleSegmentBlur(id, event.currentTarget.value)}
        onFocus={event => event.currentTarget.select()}
      />
    );
  };

  return (
    <div
      ref={containerRef}
      className={[
        'bf-datetime-field',
        error ? 'bf-datetime-field--error' : '',
        disabled ? 'bf-datetime-field--disabled' : '',
        className ?? '',
      ].filter(Boolean).join(' ')}
      data-bf-component="localized-datetime-field"
      data-bf-part="root"
      data-bf-state={error ? 'error' : undefined}
      role="group"
      aria-label={ariaLabel ?? t('dateTimeField.label')}
    >
      <span className="bf-datetime-field__group" data-bf-component="localized-datetime-field" data-bf-part="date">
        {dateOrder.map((id, index) => (
          <React.Fragment key={id}>
            {index > 0 ? <span className="bf-datetime-field__sep" aria-hidden="true">/</span> : null}
            {renderSegment(id)}
          </React.Fragment>
        ))}
      </span>
      <span className="bf-datetime-field__group" data-bf-component="localized-datetime-field" data-bf-part="time">
        {renderSegment('hour')}
        <span className="bf-datetime-field__sep" aria-hidden="true">:</span>
        {renderSegment('minute')}
      </span>
    </div>
  );
};

export default LocalizedDateTimeField;
