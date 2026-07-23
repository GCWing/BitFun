/**
 * Shared primitive and utility types.
 */
export type ID = string | number;

 
export type Status = 'pending' | 'loading' | 'success' | 'error' | 'completed';

 
export type Size = 'small' | 'medium' | 'large';

 
export type Variant = 'default' | 'primary' | 'secondary' | 'danger' | 'ghost';

 
export interface Position {
  line: number;
  column: number;
}

 
export interface Range {
  start: Position;
  end: Position;
}

 
export interface FileInfo {
  path: string;
  name: string;
  extension?: string;
  size?: number;
  lastModified?: Date;
}

 
export interface ApiResponse<T = any> {
  success: boolean;
  data?: T;
  error?: string;
  message?: string;
  code?: number;
}

 
export interface Pagination {
  page: number;
  pageSize: number;
  total: number;
  totalPages: number;
}

 
export interface PaginatedResponse<T> extends ApiResponse<T[]> {
  pagination: Pagination;
}



 
export type EventHandler<T = void> = (event: T) => void;

 
export type AsyncEventHandler<T = void> = (event: T) => Promise<void>;

 
export interface CancellablePromise<T> extends Promise<T> {
  cancel: () => void;
}



 
export interface WithChildren {
  children?: React.ReactNode;
}

 
export interface WithClassName {
  className?: string;
}

 
export interface WithOptional {
  optional?: boolean;
  required?: boolean;
}

 
export interface BaseComponentProps extends WithChildren, WithClassName {
  id?: string;
  testId?: string;
  disabled?: boolean;
}



 
export type PartialBy<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>;

 
export type RequiredBy<T, K extends keyof T> = Omit<T, K> & Required<Pick<T, K>>;

 
export type DeepPartial<T> = {
  [P in keyof T]?: T[P] extends object ? DeepPartial<T[P]> : T[P];
};

 
export type DeepReadonly<T> = {
  readonly [P in keyof T]: T[P] extends object ? DeepReadonly<T[P]> : T[P];
};

 
export type NonNullable<T> = T extends null | undefined ? never : T;
