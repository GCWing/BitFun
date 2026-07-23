/**
 * Project explorer/view types.
 */
export interface FileTreeNode {
  id: string;
  name: string;
  path: string;
  isDirectory: boolean;
  children?: FileTreeNode[];
  size?: number;
  lastModified?: string;
  extension?: string;
}


export interface FilePreviewData {
  path: string;
  content: string;
  language?: string;
  size: number;
  lastModified: string;
}
