/**
 * Shared types barrel.
 */
export * from './base';
export * from './chat';
export * from './project-view';
export * from './code-node';
export * from './global-state';
export * from './session-history';
export * from './tab';
export * from './tool-display';


export interface BaseEntity {
  id: string;
  createdAt: string;
  updatedAt: string;
}

export interface PaginationParams {
  page: number;
  pageSize: number;
  total?: number;
}

export interface SortParams {
  field: string;
  order: 'asc' | 'desc';
}

export interface ApiResponse<T> {
  data: T;
  success: boolean;
  message?: string;
  error?: string;
}

export interface LoadingState {
  loading: boolean;
  error: string | null;
}


export interface Position {
  x: number;
  y: number;
}

export interface Size {
  width: number;
  height: number;
}

export interface Rect extends Position, Size {}

export interface SelectOption<T = string> {
  label: string;
  value: T;
  disabled?: boolean;
}
