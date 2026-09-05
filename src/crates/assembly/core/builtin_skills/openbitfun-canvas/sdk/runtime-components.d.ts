import type React from "react";
import type { CSSProperties, HTMLAttributes, ReactNode } from "react";

export type BoxProps = HTMLAttributes<HTMLDivElement> & {
    padding?: number | string;
    margin?: number | string;
    background?: string;
    border?: string;
    borderRadius?: number | string;
    width?: number | string;
    height?: number | string;
};
export declare function Box(props: BoxProps): JSX.Element;

export type AlertProps = HTMLAttributes<HTMLDivElement> & {
    type?: "success" | "error" | "warning" | "info";
    tone?: "success" | "error" | "warning" | "info" | "danger";
    title?: ReactNode;
    message?: ReactNode;
    description?: ReactNode;
    showIcon?: boolean;
};
export declare function Alert(props: AlertProps): JSX.Element;

export type EmptyProps = {
    description?: ReactNode;
    image?: ReactNode;
    imageSize?: "small" | "medium" | "large" | number;
    children?: ReactNode;
    className?: string;
    style?: CSSProperties;
};
export declare function Empty(props: EmptyProps): JSX.Element;

export interface TabsItem {
    key: string;
    label: ReactNode;
    children: ReactNode;
    disabled?: boolean;
}
export type TabsProps = {
    items?: TabsItem[];
    activeKey?: string;
    defaultActiveKey?: string;
    onChange?: (key: string) => void;
    children?: ReactNode;
    type?: "line" | "card" | "pill";
    size?: "small" | "medium" | "large";
    stretch?: boolean;
    className?: string;
    style?: CSSProperties;
};
export declare function Tabs(props: TabsProps): JSX.Element;

export type InputProps = Omit<React.InputHTMLAttributes<HTMLInputElement>, "size" | "prefix"> & {
    size?: "small" | "medium" | "large";
    label?: string;
    hint?: ReactNode;
    prefix?: ReactNode;
    suffix?: ReactNode;
    error?: boolean;
    errorMessage?: string;
};
export declare function Input(props: InputProps): JSX.Element;

export interface KeyValueItem {
    key?: React.Key;
    label?: ReactNode;
    value?: ReactNode;
    tone?: string;
}
export type KeyValueListProps = HTMLAttributes<HTMLDListElement> & {
    items?: KeyValueItem[] | Record<string, ReactNode>;
    columns?: number;
    compact?: boolean;
    emptyMessage?: ReactNode;
};
export declare function KeyValueList(props: KeyValueListProps): JSX.Element;

export interface TimelineItem {
    key?: React.Key;
    title?: ReactNode;
    description?: ReactNode;
    time?: ReactNode;
    tone?: string;
    icon?: ReactNode;
}
export type TimelineProps = HTMLAttributes<HTMLOListElement> & {
    items?: TimelineItem[];
    emptyMessage?: ReactNode;
};
export declare function Timeline(props: TimelineProps): JSX.Element;

export interface FileTreeItem {
    key?: React.Key;
    name?: ReactNode;
    path?: string;
    type?: "file" | "folder";
    tone?: string;
    meta?: ReactNode;
    children?: FileTreeItem[];
}
export type FileTreeProps = HTMLAttributes<HTMLDivElement> & {
    items?: FileTreeItem[];
    defaultExpanded?: boolean;
    emptyMessage?: ReactNode;
};
export declare function FileTree(props: FileTreeProps): JSX.Element;

export type ProgressBarProps = HTMLAttributes<HTMLDivElement> & {
    value?: number;
    max?: number;
    label?: ReactNode;
    tone?: string;
    showValue?: boolean;
};
export declare function ProgressBar(props: ProgressBarProps): JSX.Element;

export interface GraphNode {
    id: string | number;
    label?: ReactNode;
    title?: ReactNode;
    description?: ReactNode;
    tone?: string;
}
export interface GraphEdge {
    from?: string | number;
    to?: string | number;
    source?: string | number;
    target?: string | number;
    label?: ReactNode;
}
export type DependencyGraphProps = HTMLAttributes<HTMLDivElement> & {
    nodes?: GraphNode[];
    edges?: GraphEdge[];
    direction?: "vertical" | "horizontal";
    nodeWidth?: number;
    nodeHeight?: number;
    rankGap?: number;
    nodeGap?: number;
    padding?: number;
    title?: ReactNode;
    height?: number;
};
export declare function DependencyGraph(props: DependencyGraphProps): JSX.Element;

export interface FlowStep {
    id?: string | number;
    label?: ReactNode;
    title?: ReactNode;
    description?: ReactNode;
    tone?: string;
}
export type FlowDiagramProps = DependencyGraphProps & { steps?: Array<string | FlowStep> };
export declare function FlowDiagram(props: FlowDiagramProps): JSX.Element;

export declare function normalizeDiffLines(lines?: string | unknown[]): unknown[];
