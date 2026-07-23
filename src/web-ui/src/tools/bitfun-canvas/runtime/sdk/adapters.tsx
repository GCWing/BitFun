import React from 'react';
import { Badge as BitFunBadge } from '@/component-library/components/Badge';
import { Button as BitFunButton } from '@/component-library/components/Button';
import {
  Card as BitFunCard,
  CardBody as BitFunCardBody,
  CardHeader as BitFunCardHeader,
} from '@/component-library/components/Card';
import { Empty as BitFunEmpty } from '@/component-library/components/Empty';
import { Input as BitFunInput } from '@/component-library/components/Input';
import { TabPane as BitFunTabPane, Tabs as BitFunTabs } from '@/component-library/components/Tabs';
import { Tag as BitFunTag } from '@/component-library/components/Tag';
import type {
  CanvasButtonProps,
  CanvasCardBodyProps,
  CanvasCardHeaderProps,
  CanvasCardProps,
  CanvasEmptyProps,
  CanvasInputProps,
  CanvasPillProps,
  CanvasTabsItem,
  CanvasTabsProps,
  CanvasTone,
} from './types';

function cardVariant(variant: CanvasCardProps['variant']): React.ComponentProps<typeof BitFunCard>['variant'] {
  if (variant === 'borderless') return 'subtle';
  return variant ?? 'default';
}

export function Card({ variant, padding = 'medium', radius = 'small', style, ...props }: CanvasCardProps) {
  return (
    <BitFunCard
      {...props}
      variant={cardVariant(variant)}
      padding={variant === 'borderless' ? 'none' : padding}
      radius={radius}
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
      subtitle={subtitle}
      extra={trailing}
    />
  );
}

export function CardBody(props: CanvasCardBodyProps) {
  return <BitFunCardBody {...props} />;
}

function buttonSize(size: CanvasButtonProps['size']): React.ComponentProps<typeof BitFunButton>['size'] {
  if (size === 'sm') return 'small';
  if (size === 'md') return 'medium';
  if (size === 'lg') return 'large';
  return size ?? 'medium';
}

export function Button({ variant = 'secondary', size, ...props }: CanvasButtonProps) {
  return (
    <BitFunButton
      {...props}
      variant={variant}
      size={buttonSize(size)}
    />
  );
}

function pillTone(tone: CanvasTone | 'accent' | 'purple' | undefined, active: boolean) {
  if (tone === 'danger' || tone === 'error') return { badge: 'error' as const, tag: 'red' as const };
  if (tone === 'success') return { badge: 'success' as const, tag: 'green' as const };
  if (tone === 'warning') return { badge: 'warning' as const, tag: 'yellow' as const };
  if (tone === 'info') return { badge: 'info' as const, tag: 'blue' as const };
  if (tone === 'purple') return { badge: 'purple' as const, tag: 'purple' as const };
  if (tone === 'accent' || active) return { badge: 'accent' as const, tag: 'blue' as const };
  return { badge: 'neutral' as const, tag: 'gray' as const };
}

function tagSize(size: CanvasPillProps['size']): React.ComponentProps<typeof BitFunTag>['size'] {
  if (size === 'sm') return 'small';
  if (size === 'md') return 'medium';
  if (size === 'lg') return 'large';
  return size ?? 'medium';
}

export function Pill({
  children,
  active = false,
  tone,
  size,
  leadingContent,
  keyboardHint,
  className,
  ...props
}: CanvasPillProps) {
  const resolvedTone = pillTone(tone, active);
  const content = (
    <>
      {leadingContent}
      {children}
      {keyboardHint ? <span className="bitfun-canvas-adapter-pill__hint">{keyboardHint}</span> : null}
    </>
  );

  if (active) {
    return (
      <BitFunBadge variant={resolvedTone.badge} className={className}>
        {content}
      </BitFunBadge>
    );
  }

  return (
    <BitFunTag
      {...props}
      color={resolvedTone.tag}
      size={tagSize(size)}
      rounded
      className={className}
    >
      {content}
    </BitFunTag>
  );
}

function renderTabItems(items: CanvasTabsItem[]) {
  return items.map(item => (
    <BitFunTabPane
      key={item.key}
      tabKey={item.key}
      label={item.label}
      disabled={item.disabled}
    >
      {item.children}
    </BitFunTabPane>
  ));
}

export function Tabs({ items, children, ...props }: CanvasTabsProps) {
  return (
    <BitFunTabs {...props}>
      {items ? renderTabItems(items) : children}
    </BitFunTabs>
  );
}

export function Input({ size, ...props }: CanvasInputProps) {
  return <BitFunInput {...props} size={size} />;
}

export function Empty(props: CanvasEmptyProps) {
  return <BitFunEmpty {...props} />;
}
