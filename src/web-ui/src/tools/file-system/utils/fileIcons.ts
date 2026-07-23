import React from 'react';
import { 
  Folder, 
  FolderOpen, 
  File, 
  FileText, 
  Image, 
  Code
} from 'lucide-react';
import { FileSystemNode, FileIconType } from '../types';
import { getFileIconType as getIconTypeFromDetector } from '@/infrastructure/language-detection';

const iconTypeMap: Record<string, FileIconType> = {
  'folder': 'folder',
  'image': 'image',
  'javascript': 'javascript',
  'typescript': 'typescript',
  'react': 'react',
  'vue': 'vue',
  'python': 'python',
  'rust': 'rust',
  'go': 'go',
  'java': 'java',
  'c': 'c-cpp',
  'cpp': 'c-cpp',
  'c-cpp': 'c-cpp',
  'html': 'html',
  'css': 'css',
  'scss': 'sass',
  'sass': 'sass',
  'less': 'sass',
  'json': 'json',
  'markdown': 'markdown',
  'config': 'config',
  'yaml': 'config',
  'toml': 'config',
  'xml': 'config',
  'database': 'database',
  'sql': 'database',
  'font': 'font',
  'audio': 'audio',
  'video': 'video',
  'archive': 'archive',
  'binary': 'binary',
  'code': 'code',
  'text': 'text',
  'file': 'file',
};

export function getFileIconType(node: FileSystemNode): FileIconType {
  if (node.isDirectory) {
    return 'folder';
  }

  const detectedIconType = getIconTypeFromDetector(node.name);
  return iconTypeMap[detectedIconType] || 'file';
}

export function getFileIcon(node: FileSystemNode, isExpanded?: boolean): React.ReactElement {
  const iconType = getFileIconType(node);
  
  if (node.isDirectory) {
    return isExpanded ? React.createElement(FolderOpen, { size: 16 }) : React.createElement(Folder, { size: 16 });
  }
  
  switch (iconType) {
    case 'image':
      return React.createElement(Image, { size: 16 });
    case 'code':
    case 'javascript':
    case 'typescript':
    case 'react':
    case 'vue':
    case 'python':
    case 'rust':
    case 'go':
    case 'java':
    case 'c-cpp':
    case 'html':
    case 'css':
    case 'sass':
      return React.createElement(Code, { size: 16 });
    case 'markdown':
    case 'text':
      return React.createElement(FileText, { size: 16 });
    default:
      return React.createElement(File, { size: 16 });
  }
}

export function getFileIconClass(node: FileSystemNode, isExpanded?: boolean): string {
  if (node.isDirectory) {
    return `bitfun-file-explorer__icon bitfun-file-explorer__icon--folder${isExpanded ? ' bitfun-file-explorer__icon--folder-open' : ''}`;
  }
  
  const iconType = getFileIconType(node);
  return `bitfun-file-explorer__icon bitfun-file-explorer__icon--${iconType}`;
}

export function isImageFile(node: FileSystemNode): boolean {
  return getFileIconType(node) === 'image';
}

export function isCodeFile(node: FileSystemNode): boolean {
  const iconType = getFileIconType(node);
  return [
    'code', 'javascript', 'typescript', 'react', 'vue', 'python',
    'rust', 'go', 'java', 'c-cpp', 'html', 'css', 'sass'
  ].includes(iconType);
}

export function isConfigFile(node: FileSystemNode): boolean {
  const iconType = getFileIconType(node);
  return ['config', 'json'].includes(iconType);
}