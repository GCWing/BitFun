import React from 'react';
import {
  Button as OpenBitFunButton,
  Card as OpenBitFunCard,
  CardBody as OpenBitFunCardBody,
  CardHeader as OpenBitFunCardHeader,
  Empty as OpenBitFunEmpty,
  Field as DesignField,
  Input as DesignInput,
  StatusPill as DesignStatusPill,
  TabGroup as DesignTabGroup,
} from '@openbitfun/ui';
import type {
  CanvasButtonProps,
  CanvasCardBodyProps,
  CanvasCardHeaderProps,
  CanvasCardProps,
  CanvasEmptyProps,
  CanvasInputProps,
  CanvasPillProps,
  CanvasTabsProps,
  CanvasTone,
} from './types';
import { canvasArrayProp } from './runtimeValidation';

function cardAppearance(variant: CanvasCardProps['variant']): React.ComponentProps<typeof OpenBitFunCard>['appearance'] {
  if (variant === 'borderless') return 'subtle';
  if (variant === 'elevated' || variant === 'accent') return 'raised';
  if (variant === 'default') return 'neutral';
  return 'subtle';
}

function cardPadding(padding: CanvasCardProps['padding']): React.ComponentProps<typeof OpenBitFunCard>['padding'] {
  if (padding === 'small') return 'sm';
  if (padding === 'medium' || padding === 'large') return 'md';
  return 'none';
}

function cardRadius(radius: CanvasCardProps['radius']): React.ComponentProps<typeof OpenBitFunCard>['radius'] {
  if (radius === 'small') return 'sm';
  if (radius === 'large') return 'lg';
  return 'md';
}

export function Card({ variant, padding = 'medium', radius = 'small', style, ...props }: CanvasCardProps) {
  return (
    <OpenBitFunCard
      {...props}
      appearance={cardAppearance(variant)}
      padding={variant === 'borderless' ? 'none' : cardPadding(padding)}
      radius={cardRadius(radius)}
      style={{
        ...(variant === 'borderless' ? { background: 'transparent' } : null),
        ...style,
      }}
    />
  );
}

export function CardHeader({ children, title, subtitle, trailing, ...props }: CanvasCardHeaderProps) {
  return (
    <OpenBitFunCardHeader
      {...props}
      title={title ?? children}
      description={subtitle}
      actions={trailing}
    />
  );
}

export function CardBody(props: CanvasCardBodyProps) {
  return <OpenBitFunCardBody {...props} />;
}

function buttonSize(size: CanvasButtonProps['size']): React.ComponentProps<typeof OpenBitFunButton>['size'] {
  if (size === 'sm' || size === 'small') return 'sm';
  if (size === 'lg' || size === 'large') return 'lg';
  return 'md';
}

function buttonVariant(
  variant: CanvasButtonProps['variant'],
): React.ComponentProps<typeof OpenBitFunButton>['variant'] {
  if (variant === 'secondary' || variant === 'ghost') return 'outline';
  return 'fill';
}

export function Button({ children, variant = 'secondary', size, ...props }: CanvasButtonProps) {
  return (
    <OpenBitFunButton
      {...props}
      variant={buttonVariant(variant)}
      size={buttonSize(size)}
    >
      {children}
    </OpenBitFunButton>
  );
}

function pillTone(tone: CanvasTone | 'accent' | 'purple' | undefined, active: boolean) {
  if (tone === 'danger' || tone === 'error') return 'danger' as const;
  if (tone === 'success') return 'success' as const;
  if (tone === 'warning') return 'warning' as const;
  if (tone === 'info') return 'info' as const;
  if (tone === 'purple' || tone === 'accent' || active) return 'accent' as const;
  return 'neutral' as const;
}

export function Pill({
  children,
  active = false,
  tone,
  size: _size,
  leadingContent,
  keyboardHint,
  className,
  ...props
}: CanvasPillProps) {
  return (
    <DesignStatusPill
      {...props}
      tone={pillTone(tone, active)}
      leading={leadingContent}
      className={className}
    >
      {children}
      {keyboardHint ? <span className="openbitfun-canvas-adapter-pill__hint">{keyboardHint}</span> : null}
    </DesignStatusPill>
  );
}

export function Tabs({
  items = [],
  children,
  activeKey,
  defaultActiveKey,
  onChange,
  type = 'line',
  size = 'medium',
  stretch = false,
  className,
  style,
}: CanvasTabsProps) {
  const safeItems = canvasArrayProp<NonNullable<CanvasTabsProps['items']>[number]>('Tabs', 'items', items);
  const tabsId = React.useId();
  const [internalActiveKey, setInternalActiveKey] = React.useState(
    defaultActiveKey ?? safeItems.find(item => !item.disabled)?.key ?? '',
  );
  const candidateKey = activeKey ?? internalActiveKey;
  const selectedItem = safeItems.find(item => item.key === candidateKey && !item.disabled)
    ?? safeItems.find(item => !item.disabled);
  const selectedKey = selectedItem?.key ?? '';

  if (safeItems.length === 0) {
    return <div className={className} style={style}>{children}</div>;
  }

  const handleValueChange = (key: string) => {
    if (activeKey === undefined) setInternalActiveKey(key);
    onChange?.(key);
  };

  return (
    <div
      className={['openbitfun-canvas-adapter-tabs', className].filter(Boolean).join(' ')}
      data-size={size}
      data-stretch={stretch ? 'true' : 'false'}
      data-type={type}
      style={style}
    >
      <DesignTabGroup
        items={safeItems.map(item => ({
          disabled: item.disabled,
          id: `${tabsId}-tab-${item.key}`,
          label: item.label,
          panelId: `${tabsId}-panel`,
          value: item.key,
        }))}
        value={selectedKey}
        onValueChange={handleValueChange}
      />
      <div
        aria-labelledby={`${tabsId}-tab-${selectedKey}`}
        id={`${tabsId}-panel`}
        role="tabpanel"
      >
        {selectedItem?.children}
      </div>
    </div>
  );
}

function designInputSize(size: CanvasInputProps['size']): 'sm' | 'md' | 'lg' {
  if (size === 'small') return 'sm';
  if (size === 'large') return 'lg';
  return 'md';
}

export function Input({
  size,
  label,
  hint,
  prefix,
  suffix,
  error,
  errorMessage,
  ...props
}: CanvasInputProps) {
  const control = (
    <DesignInput
      {...props}
      invalid={error}
      leading={prefix}
      trailing={suffix}
      size={designInputSize(size)}
    />
  );
  const fieldError = error ? errorMessage : undefined;
  if (label === undefined && hint === undefined && fieldError === undefined) {
    return control;
  }
  return (
    <DesignField label={label} description={hint} error={fieldError} controlWidth="fill">
      {control}
    </DesignField>
  );
}

function emptyMediaSize(size: CanvasEmptyProps['imageSize']): React.ComponentProps<typeof OpenBitFunEmpty>['imageSize'] {
  if (size === 'small' || (typeof size === 'number' && size <= 32)) return 'sm';
  if (size === 'large' || (typeof size === 'number' && size >= 48)) return 'lg';
  return size === undefined ? undefined : 'md';
}

export function Empty({ imageSize, ...props }: CanvasEmptyProps) {
  return (
    <OpenBitFunEmpty
      {...props}
      description={props.description ?? 'No data'}
      imageSize={emptyMediaSize(imageSize)}
    />
  );
}
