import type React from 'react';

export type CanvasTone =
  | 'primary'
  | 'secondary'
  | 'tertiary'
  | 'quaternary'
  | 'muted'
  | 'success'
  | 'warning'
  | 'danger'
  | 'error'
  | 'info'
  | 'neutral';

export type CanvasColor =
  | 'gray'
  | 'purple'
  | 'green'
  | 'yellow'
  | 'cyan'
  | 'pink'
  | 'blue'
  | 'orange';

export type Color = CanvasColor;

export type CanvasSpacing =
  | number
  | string
  | {
      x?: number | string;
      y?: number | string;
      top?: number | string;
      right?: number | string;
      bottom?: number | string;
      left?: number | string;
    };

export interface CanvasCommonStyleProps {
  padding?: CanvasSpacing;
  margin?: CanvasSpacing;
  background?: React.CSSProperties['background'];
  border?: React.CSSProperties['border'];
  borderTop?: React.CSSProperties['borderTop'];
  borderRight?: React.CSSProperties['borderRight'];
  borderBottom?: React.CSSProperties['borderBottom'];
  borderLeft?: React.CSSProperties['borderLeft'];
  borderRadius?: number | string;
  width?: number | string;
  height?: number | string;
  flex?: React.CSSProperties['flex'];
  display?: React.CSSProperties['display'];
  color?: React.CSSProperties['color'];
  opacity?: React.CSSProperties['opacity'];
  minWidth?: number | string;
  maxWidth?: number | string;
  minHeight?: number | string;
  maxHeight?: number | string;
}

type CanvasDivAttributes = Omit<React.HTMLAttributes<HTMLDivElement>, keyof CanvasCommonStyleProps>;

export interface CanvasStackProps extends CanvasDivAttributes, CanvasCommonStyleProps {
  gap?: number | string;
}

export interface CanvasRowProps extends CanvasDivAttributes, CanvasCommonStyleProps {
  gap?: number | string;
  align?: React.CSSProperties['alignItems'] | 'start' | 'end';
  justify?: React.CSSProperties['justifyContent'] | 'start' | 'end';
  wrap?: boolean;
}

export interface CanvasGridProps extends CanvasDivAttributes, CanvasCommonStyleProps {
  columns?: number | string;
  gap?: number | string;
  align?: React.CSSProperties['alignItems'] | 'start' | 'end';
}

export interface CanvasBoxProps extends CanvasDivAttributes, CanvasCommonStyleProps {}

export interface CanvasDividerProps extends React.HTMLAttributes<HTMLHRElement> {}

export interface CanvasHeadingProps extends React.HTMLAttributes<HTMLHeadingElement> {}

export interface CanvasTextProps extends CanvasCommonStyleProps {
  children?: React.ReactNode;
  tone?: CanvasTone;
  size?: 'sm' | 'small' | 'body' | 'md' | 'lg' | number | string;
  weight?: 'normal' | 'medium' | 'semibold' | 'bold' | number | string;
  italic?: boolean;
  as?: keyof React.ReactHTML;
  truncate?: boolean;
  color?: React.CSSProperties['color'];
  style?: React.CSSProperties;
}

export interface CanvasCodeProps extends React.HTMLAttributes<HTMLElement> {}

export interface CanvasLinkProps extends React.AnchorHTMLAttributes<HTMLAnchorElement> {}

export interface CanvasCalloutProps extends Omit<React.HTMLAttributes<HTMLElement>, 'title'> {
  tone?: CanvasTone;
  title?: React.ReactNode;
}

export interface CanvasAlertProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'title'> {
  type?: 'success' | 'error' | 'warning' | 'info';
  tone?: CanvasTone;
  title?: React.ReactNode;
  message?: React.ReactNode;
  description?: React.ReactNode;
  showIcon?: boolean;
}

export interface CanvasCollapsibleSectionProps extends Omit<React.HTMLAttributes<HTMLElement>, 'title'> {
  title?: React.ReactNode;
  leading?: React.ReactNode;
  count?: React.ReactNode;
  trailing?: React.ReactNode;
  defaultOpen?: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export interface CanvasStatProps extends React.HTMLAttributes<HTMLDivElement> {
  value?: React.ReactNode;
  label?: React.ReactNode;
  tone?: CanvasTone;
}

export type CanvasTableCell = React.ReactNode;
export type CanvasTableRow = CanvasTableCell[];
export type CanvasColumnAlign = React.CSSProperties['textAlign'];

export interface CanvasTableProps extends React.HTMLAttributes<HTMLDivElement> {
  headers?: React.ReactNode[];
  rows?: CanvasTableRow[];
  columnAlign?: CanvasColumnAlign[];
  rowTone?: CanvasTone[];
  framed?: boolean;
  striped?: boolean;
  stickyHeader?: boolean;
  emptyMessage?: React.ReactNode;
}

export interface CanvasDiffStatsProps extends React.HTMLAttributes<HTMLSpanElement> {
  additions?: number;
  deletions?: number;
}

export interface CanvasDiffLine {
  type?: 'added' | 'addition' | 'removed' | 'removal' | string;
  lineNumber?: number;
  oldLineNumber?: number;
  newLineNumber?: number;
  content?: React.ReactNode;
  text?: React.ReactNode;
}

export interface CanvasDiffViewProps extends React.HTMLAttributes<HTMLDivElement> {
  lines?: string | Array<string | CanvasDiffLine>;
  showLineNumbers?: boolean;
  coloredLineNumbers?: boolean;
  showAccentStrip?: boolean;
}

export interface CanvasKeyValueItem {
  key?: React.Key;
  label?: React.ReactNode;
  value?: React.ReactNode;
  tone?: CanvasTone;
}

export interface CanvasKeyValueListProps extends React.HTMLAttributes<HTMLDListElement> {
  items?: CanvasKeyValueItem[] | Record<string, React.ReactNode>;
  columns?: number;
  compact?: boolean;
  emptyMessage?: React.ReactNode;
}

export interface CanvasTimelineItem {
  key?: React.Key;
  title?: React.ReactNode;
  description?: React.ReactNode;
  time?: React.ReactNode;
  tone?: CanvasTone;
  icon?: React.ReactNode;
}

export interface CanvasTimelineProps extends React.HTMLAttributes<HTMLOListElement> {
  items?: CanvasTimelineItem[];
  emptyMessage?: React.ReactNode;
}

export interface CanvasFileTreeItem {
  key?: React.Key;
  name?: React.ReactNode;
  path?: string;
  type?: 'file' | 'folder';
  tone?: CanvasTone;
  meta?: React.ReactNode;
  children?: CanvasFileTreeItem[];
}

export interface CanvasFileTreeProps extends React.HTMLAttributes<HTMLDivElement> {
  items?: CanvasFileTreeItem[];
  defaultExpanded?: boolean;
  emptyMessage?: React.ReactNode;
}

export interface CanvasProgressBarProps extends React.HTMLAttributes<HTMLDivElement> {
  value?: number;
  max?: number;
  label?: React.ReactNode;
  tone?: CanvasTone;
  showValue?: boolean;
}

export interface CanvasSwatchProps extends React.HTMLAttributes<HTMLSpanElement> {
  color?: CanvasColor;
}

export interface CanvasUsageBarSegment {
  id: string;
  value: number;
  color?: CanvasColor;
}

export interface CanvasUsageBarProps extends React.HTMLAttributes<HTMLDivElement> {
  segments?: readonly CanvasUsageBarSegment[];
  total?: number;
  topLeftLabel?: React.ReactNode;
  topRightLabel?: React.ReactNode;
}

export type CanvasTodoStatus = 'pending' | 'in_progress' | 'completed' | 'cancelled';

export interface CanvasTodoItem {
  id: string;
  content: string;
  status: CanvasTodoStatus;
}

export interface CanvasTodoListProps extends React.HTMLAttributes<HTMLDivElement> {
  todos?: readonly CanvasTodoItem[];
  dimmedTodoIds?: ReadonlySet<string> | string[];
  onTodoClick?: (todo: CanvasTodoItem) => void;
}

export interface CanvasTodoListCardProps extends React.HTMLAttributes<HTMLElement> {
  todos?: readonly CanvasTodoItem[];
  dimmedTodoIds?: ReadonlySet<string> | string[];
  defaultExpanded?: boolean;
  onTodoClick?: (todo: CanvasTodoItem) => void;
}

export interface CanvasDagNode {
  id: string | number;
  label?: React.ReactNode;
  title?: React.ReactNode;
  description?: React.ReactNode;
  subtitle?: React.ReactNode;
  tone?: CanvasTone;
  meta?: React.ReactNode;
  group?: string | number;
  [key: string]: unknown;
}

export interface CanvasDagEdge {
  from?: string | number;
  to?: string | number;
  source?: string | number;
  target?: string | number;
  label?: React.ReactNode;
  tone?: CanvasTone;
  [key: string]: unknown;
}

export interface CanvasDagLayoutOptions {
  nodes?: CanvasDagNode[];
  edges?: CanvasDagEdge[];
  direction?: 'vertical' | 'horizontal';
  nodeWidth?: number;
  nodeHeight?: number;
  rankGap?: number;
  nodeGap?: number;
  padding?: number;
}

export interface CanvasDagLayoutNode {
  id: string;
  label?: React.ReactNode;
  title?: React.ReactNode;
  description?: React.ReactNode;
  subtitle?: React.ReactNode;
  tone?: CanvasTone;
  meta?: CanvasDagNode;
  source?: CanvasDagNode;
  group?: string | number;
  x: number;
  y: number;
  centerX: number;
  centerY: number;
  width: number;
  height: number;
  rank: number;
  [key: string]: unknown;
}

export interface CanvasDagLayoutEdge {
  from: string;
  to: string;
  sourceX: number;
  sourceY: number;
  targetX: number;
  targetY: number;
  isBackEdge: boolean;
  path: string;
  [key: string]: unknown;
}

export interface CanvasDagLayoutRank {
  rank: number;
  x: number;
  y: number;
  width: number;
  height: number;
  nodeIds?: string[];
  nodes?: CanvasDagLayoutNode[];
}

export interface CanvasDagLayout {
  nodes: CanvasDagLayoutNode[];
  edges: CanvasDagLayoutEdge[];
  ranks: CanvasDagLayoutRank[];
  direction: 'vertical' | 'horizontal';
  width: number;
  height: number;
  [Symbol.iterator](): IterableIterator<CanvasDagLayoutNode>;
  find(
    predicate: (
      value: CanvasDagLayoutNode,
      index: number,
      obj: CanvasDagLayoutNode[],
    ) => unknown,
    thisArg?: unknown,
  ): CanvasDagLayoutNode | undefined;
  forEach(
    callbackfn: (
      value: CanvasDagLayoutNode,
      index: number,
      array: CanvasDagLayoutNode[],
    ) => void,
    thisArg?: unknown,
  ): void;
  map<T>(
    callbackfn: (
      value: CanvasDagLayoutNode,
      index: number,
      array: CanvasDagLayoutNode[],
    ) => T,
    thisArg?: unknown,
  ): T[];
  filter(
    predicate: (
      value: CanvasDagLayoutNode,
      index: number,
      array: CanvasDagLayoutNode[],
    ) => unknown,
    thisArg?: unknown,
  ): CanvasDagLayoutNode[];
}

export interface CanvasDependencyGraphProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'title'>, CanvasDagLayoutOptions {
  title?: React.ReactNode;
  height?: number;
}

export interface CanvasFlowStep {
  id?: string | number;
  label?: React.ReactNode;
  title?: React.ReactNode;
  description?: React.ReactNode;
  subtitle?: React.ReactNode;
  sub?: React.ReactNode;
  tone?: CanvasTone;
  meta?: React.ReactNode;
}

export interface CanvasFlowDiagramProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'title'>, CanvasDagLayoutOptions {
  title?: React.ReactNode;
  steps?: Array<string | CanvasFlowStep>;
  height?: number;
}

export interface CanvasChartSeries {
  name?: React.ReactNode;
  label?: React.ReactNode;
  key?: string;
  data?: Array<number | string | null | undefined>;
  value?: number | string | null;
  color?: string;
}

export interface CanvasChartDatum {
  [key: string]: unknown;
  label?: React.ReactNode;
  name?: React.ReactNode;
  title?: React.ReactNode;
  category?: React.ReactNode;
  x?: React.ReactNode;
  value?: number | string | null;
  color?: string;
}

export interface CanvasChartProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'title'> {
  title?: React.ReactNode;
  height?: number;
  data?: Array<number | string | null | undefined | CanvasChartDatum>;
  categories?: React.ReactNode[];
  series?: Array<string | CanvasChartSeries>;
  labelKey?: string;
  nameKey?: string;
  xKey?: string;
  valueKey?: string;
  yKey?: string;
  color?: string;
  name?: React.ReactNode;
}

export interface CanvasCardProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'elevated' | 'subtle' | 'accent' | 'borderless';
  padding?: 'none' | 'small' | 'medium' | 'large';
  radius?: 'small' | 'medium' | 'large';
}

export interface CanvasCardHeaderProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'title'> {
  children?: React.ReactNode;
  title?: React.ReactNode;
  subtitle?: React.ReactNode;
  trailing?: React.ReactNode;
}

export type CanvasCardBodyProps = React.HTMLAttributes<HTMLDivElement>;

export interface CanvasButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger' | 'success';
  size?: 'sm' | 'small' | 'md' | 'medium' | 'lg' | 'large';
}

export interface CanvasPillProps extends React.HTMLAttributes<HTMLSpanElement> {
  active?: boolean;
  tone?: CanvasTone | 'accent' | 'purple';
  size?: 'sm' | 'md' | 'lg' | 'small' | 'medium' | 'large';
  leadingContent?: React.ReactNode;
  keyboardHint?: React.ReactNode;
}

export interface CanvasTabsItem {
  key: string;
  label: React.ReactNode;
  children: React.ReactNode;
  disabled?: boolean;
}

export interface CanvasTabsProps {
  items?: CanvasTabsItem[];
  activeKey?: string;
  defaultActiveKey?: string;
  onChange?: (key: string) => void;
  children?: React.ReactNode;
  type?: 'line' | 'card' | 'pill';
  size?: 'small' | 'medium' | 'large';
  stretch?: boolean;
  className?: string;
  style?: React.CSSProperties;
}

export interface CanvasInputProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size' | 'prefix'> {
  size?: 'small' | 'medium' | 'large';
  label?: string;
  hint?: React.ReactNode;
  prefix?: React.ReactNode;
  suffix?: React.ReactNode;
  error?: boolean;
  errorMessage?: string;
}

export interface CanvasToggleProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'onChange' | 'size'> {
  checked?: boolean;
  onChange?: (checked: boolean) => void;
  label?: string;
  description?: string;
  size?: 'sm' | 'small' | 'md' | 'medium' | 'lg' | 'large';
  loading?: boolean;
  checkedText?: string;
  uncheckedText?: string;
}

export interface CanvasCheckboxProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'onChange' | 'size'> {
  checked?: boolean;
  onChange?: (checked: boolean) => void;
  label?: React.ReactNode;
  description?: string;
  size?: 'sm' | 'small' | 'md' | 'medium' | 'lg' | 'large';
  indeterminate?: boolean;
  error?: boolean;
}

export interface CanvasSelectOption {
  label?: React.ReactNode;
  value: string | number;
  disabled?: boolean;
}

export interface CanvasSelectProps extends Omit<React.SelectHTMLAttributes<HTMLSelectElement>, 'onChange' | 'size'> {
  value?: string | number;
  options?: Array<string | number | CanvasSelectOption>;
  placeholder?: React.ReactNode;
  onChange?: (value: string) => void;
  size?: 'sm' | 'small' | 'md' | 'medium' | 'lg' | 'large';
}

export interface CanvasTextInputProps extends Omit<CanvasInputProps, 'onChange'> {
  onChange?: (value: string) => void;
}

export interface CanvasTextAreaProps extends Omit<React.TextareaHTMLAttributes<HTMLTextAreaElement>, 'onChange'> {
  label?: string;
  error?: boolean;
  errorMessage?: string;
  hint?: string;
  autoResize?: boolean;
  showCount?: boolean;
  onChange?: (value: string) => void;
}

export interface CanvasIconButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  size?: 'sm' | 'small' | 'md' | 'medium' | 'lg' | 'large';
  variant?: 'default' | 'primary' | 'ghost' | 'danger' | 'success' | 'warning' | 'ai';
  shape?: 'square' | 'circle';
  isLoading?: boolean;
  tooltip?: React.ReactNode;
}

export interface CanvasEmptyProps {
  description?: React.ReactNode;
  image?: React.ReactNode;
  imageSize?: 'small' | 'medium' | 'large' | number;
  children?: React.ReactNode;
  className?: string;
  style?: React.CSSProperties;
}
