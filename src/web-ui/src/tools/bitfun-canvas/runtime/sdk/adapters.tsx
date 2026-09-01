import React from 'react';
import {
  Button as BitFunButton,
  Card as BitFunCard,
  CardBody as BitFunCardBody,
  CardHeader as BitFunCardHeader,
  Empty as BitFunEmpty,
  Field as DesignField,
  Input as DesignInput,
  StatusPill as DesignStatusPill,
  TabGroup as DesignTabGroup,
} from '@bitfun/ui';
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

function cardAppearance(variant: CanvasCardProps['variant']): React.ComponentProps<typeof BitFunCard>['appearance'] {
  if (variant === 'borderless') return 'subtle';
  if (variant === 'elevated' || variant === 'accent') return 'raised';
  if (variant === 'default') return 'neutral';
  return 'subtle';
}

function cardPadding(padding: CanvasCardProps['padding']): React.ComponentProps<typeof BitFunCard>['padding'] {
  if (padding === 'small') return 'sm';
  if (padding === 'medium' || padding === 'large') return 'md';
  return 'none';
}

function cardRadius(radius: CanvasCardProps['radius']): React.ComponentProps<typeof BitFunCard>['radius'] {
  if (radius === 'small') return 'sm';
  if (radius === 'large') return 'lg';
  return 'md';
}

export function Card({ variant, padding = 'medium', radius = 'small', style, ...props }: CanvasCardProps) {
  return (
    <BitFunCard
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
    <BitFunCardHeader
      {...props}
      title={title ?? children}
      description={subtitle}
      actions={trailing}
    />
  );
}

export function CardBody(props: CanvasCardBodyProps) {
  return <BitFunCardBody {...props} />;
}

function buttonSize(size: CanvasButtonProps['size']): React.ComponentProps<typeof BitFunButton>['size'] {
  if (size === 'sm' || size === 'small') return 'sm';
  if (size === 'lg' || size === 'large') return 'lg';
  return 'md';
}

function buttonVariant(
  variant: CanvasButtonProps['variant'],
): React.ComponentProps<typeof BitFunButton>['variant'] {
  if (variant === 'secondary' || variant === 'ghost') return 'outline';
  return 'fill';
}

export function Button({ children, variant = 'secondary', size, ...props }: CanvasButtonProps) {
  return (
    <BitFunButton
      {...props}
      variant={buttonVariant(variant)}
      size={buttonSize(size)}
    >
      {children}
    </BitFunButton>
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
      {keyboardHint ? <span className="bitfun-canvas-adapter-pill__hint">{keyboardHint}</span> : null}
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
  const tabsId = React.useId();
  const [internalActiveKey, setInternalActiveKey] = React.useState(
    defaultActiveKey ?? items.find(item => !item.disabled)?.key ?? '',
  );
  const candidateKey = activeKey ?? internalActiveKey;
  const selectedItem = items.find(item => item.key === candidateKey && !item.disabled)
    ?? items.find(item => !item.disabled);
  const selectedKey = selectedItem?.key ?? '';

  if (items.length === 0) {
    return <div className={className} style={style}>{children}</div>;
  }

  const handleValueChange = (key: string) => {
    if (activeKey === undefined) setInternalActiveKey(key);
    onChange?.(key);
  };

  return (
    <div
      className={['bitfun-canvas-adapter-tabs', className].filter(Boolean).join(' ')}
      data-size={size}
      data-stretch={stretch ? 'true' : 'false'}
      data-type={type}
      style={style}
    >
      <DesignTabGroup
        items={items.map(item => ({
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

function emptyMediaSize(size: CanvasEmptyProps['imageSize']): React.ComponentProps<typeof BitFunEmpty>['imageSize'] {
  if (size === 'small' || (typeof size === 'number' && size <= 32)) return 'sm';
  if (size === 'large' || (typeof size === 'number' && size >= 48)) return 'lg';
  return size === undefined ? undefined : 'md';
}

export function Empty({ imageSize, ...props }: CanvasEmptyProps) {
  return (
    <BitFunEmpty
      {...props}
      description={props.description ?? 'No data'}
      imageSize={emptyMediaSize(imageSize)}
    />
  );
}
