/** File path breadcrumb with a dropdown for quick navigation. */

import React, { useMemo, useCallback, useState, useRef, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { ChevronRight, File, Folder, Code, Loader2, ArrowLeft } from 'lucide-react';
import { getFileIconType } from '@/tools/file-system/utils/fileIcons';
import { workspaceAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import { Tooltip } from '@/component-library';
import './EditorBreadcrumb.scss';

const log = createLogger('EditorBreadcrumb');

export interface EditorBreadcrumbProps {
  /** Full file path */
  filePath: string;
  /** Workspace path (for calculating relative path) */
  workspacePath?: string;
  /** Custom class name */
  className?: string;
}

interface PathSegment {
  name: string;
  fullPath: string;
  isFile: boolean;
}

interface FileItem {
  name: string;
  path: string;
  isDirectory: boolean;
}

/** Get icon component based on file name */
const getFileIconComponent = (fileName: string, size: number = 12): React.ReactElement => {
  const iconType = getFileIconType({ name: fileName, isDirectory: false } as any);
  
  switch (iconType) {
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
    case 'code':
      return <Code size={size} />;
    default:
      return <File size={size} />;
  }
};

/** Get directory name from path */
const getDirectoryName = (path: string): string => {
  const normalized = path.replace(/\\/g, '/');
  const parts = normalized.split('/').filter(Boolean);
  return parts[parts.length - 1] || path;
};

/** Get parent directory path */
const getParentPath = (path: string): string | null => {
  const normalized = path.replace(/\\/g, '/');
  const lastSlash = normalized.lastIndexOf('/');
  if (lastSlash <= 0) return null;
  return normalized.substring(0, lastSlash);
};

/** Dropdown menu component (rendered to body via Portal) */
interface DropdownMenuProps {
  isOpen: boolean;
  items: FileItem[];
  loading: boolean;
  currentDirPath: string;
  initialDirPath: string;
  onSelect: (item: FileItem) => void;
  onGoBack: () => void;
  onClose: () => void;
  anchorEl: HTMLElement | null;
  currentFilePath: string;
  workspacePath?: string;
}

const DropdownMenu: React.FC<DropdownMenuProps> = ({
  isOpen,
  items,
  loading,
  currentDirPath,
  initialDirPath,
  onSelect,
  onGoBack,
  onClose,
  anchorEl,
  currentFilePath,
}) => {
  const menuRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ top: 0, left: 0 });

  useEffect(() => {
    if (!isOpen || !anchorEl) return;

    const rect = anchorEl.getBoundingClientRect();
    setPosition({
      top: rect.bottom + 2,
      left: rect.left,
    });
  }, [isOpen, anchorEl]);

  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (event: MouseEvent) => {
      if (
        menuRef.current &&
        !menuRef.current.contains(event.target as Node) &&
        anchorEl &&
        !anchorEl.contains(event.target as Node)
      ) {
        onClose();
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };

    const timer = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
      document.addEventListener('keydown', handleKeyDown);
    }, 0);

    return () => {
      clearTimeout(timer);
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen, onClose, anchorEl]);

  if (!isOpen) return null;

  // Sort: directories first, then by name
  const sortedItems = [...items].sort((a, b) => {
    if (a.isDirectory && !b.isDirectory) return -1;
    if (!a.isDirectory && b.isDirectory) return 1;
    return a.name.localeCompare(b.name);
  });

  // Check if we can go back to parent
  const canGoBack = currentDirPath !== initialDirPath;
  const currentDirName = getDirectoryName(currentDirPath);

  const menuContent = (
    <div 
      ref={menuRef} 
      className="editor-breadcrumb-dropdown"
      style={{
        position: 'fixed',
        top: position.top,
        left: position.left,
      }}
    >
      {canGoBack && (
        <div className="editor-breadcrumb-dropdown__header">
          <Tooltip content="Go to parent directory" placement="top">
            <button 
              className="editor-breadcrumb-dropdown__back"
              onClick={(e) => {
                e.stopPropagation();
                onGoBack();
              }}
            >
              <ArrowLeft size={12} />
            </button>
          </Tooltip>
          <Tooltip content={currentDirPath} placement="top">
            <span className="editor-breadcrumb-dropdown__title">
              {currentDirName}
            </span>
          </Tooltip>
        </div>
      )}
      
      {loading ? (
        <div className="editor-breadcrumb-dropdown__loading">
          <Loader2 size={14} className="editor-breadcrumb-dropdown__spinner" />
          <span>Loading...</span>
        </div>
      ) : sortedItems.length === 0 ? (
        <div className="editor-breadcrumb-dropdown__empty">
          Empty directory
        </div>
      ) : (
        <ul className="editor-breadcrumb-dropdown__list">
          {sortedItems.map((item) => {
            const isCurrentFile = item.path.replace(/\\/g, '/') === currentFilePath.replace(/\\/g, '/');
            return (
              <li
                key={item.path}
                className={`editor-breadcrumb-dropdown__item ${isCurrentFile ? 'editor-breadcrumb-dropdown__item--current' : ''}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onSelect(item);
                }}
              >
                <span className="editor-breadcrumb-dropdown__item-icon">
                  {item.isDirectory ? (
                    <Folder size={14} />
                  ) : (
                    getFileIconComponent(item.name, 14)
                  )}
                </span>
                <span className="editor-breadcrumb-dropdown__item-name">
                  {item.name}
                </span>
                {item.isDirectory && (
                  <ChevronRight size={12} className="editor-breadcrumb-dropdown__item-arrow" />
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );

  return createPortal(menuContent, document.body);
};
export const EditorBreadcrumb: React.FC<EditorBreadcrumbProps> = ({
  filePath,
  workspacePath,
  className = '',
}) => {
  // Dropdown menu state
  const [openDropdown, setOpenDropdown] = useState<string | null>(null);
  const [dropdownItems, setDropdownItems] = useState<FileItem[]>([]);
  const [dropdownLoading, setDropdownLoading] = useState(false);
  const [currentDirPath, setCurrentDirPath] = useState<string>('');
  const [initialDirPath, setInitialDirPath] = useState<string>('');
  const [anchorEl, setAnchorEl] = useState<HTMLElement | null>(null);
  const itemRefs = useRef<Map<string, HTMLSpanElement>>(new Map());

  // Parse path into segments
  const segments = useMemo<PathSegment[]>(() => {
    if (!filePath) return [];

    const normalizedPath = filePath.replace(/\\/g, '/');
    let relativePath = normalizedPath;
    const normalizedWorkspace = workspacePath ? workspacePath.replace(/\\/g, '/') : '';

    if (normalizedWorkspace) {
      if (normalizedPath.toLowerCase().startsWith(normalizedWorkspace.toLowerCase())) {
        relativePath = normalizedPath.slice(normalizedWorkspace.length).replace(/^\//, '');
      }
    }

    const parts = relativePath.split('/').filter(Boolean);
    if (parts.length === 0) return [];

    const result: PathSegment[] = [];
    
    // Add root directory as first level
    if (normalizedWorkspace) {
      const rootName = normalizedWorkspace.split('/').filter(Boolean).pop() || 'root';
      result.push({
        name: rootName,
        fullPath: normalizedWorkspace,
        isFile: false,
      });
    }

    let currentPath = normalizedWorkspace;

    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      currentPath = currentPath ? `${currentPath}/${part}` : part;
      result.push({
        name: part,
        fullPath: currentPath,
        isFile: i === parts.length - 1,
      });
    }

    return result;
  }, [filePath, workspacePath]);

  // Load directory contents
  const loadDirectoryContents = useCallback(async (dirPath: string) => {
    setDropdownLoading(true);
    setCurrentDirPath(dirPath);
    try {
      const fileTree = await workspaceAPI.getFileTree(dirPath, 1);
      const rootNode = fileTree?.[0];
      const children = rootNode?.children || [];
      
      const items: FileItem[] = children
        .filter((entry: any) => {
          const name = entry.name || '';
          return !name.startsWith('.') && 
                 !['node_modules', 'target', 'dist', 'build', '__pycache__', '.git'].includes(name);
        })
        .map((entry: any) => ({
          name: entry.name,
          path: entry.path,
          isDirectory: entry.isDirectory || false,
        }));

      setDropdownItems(items);
    } catch (error) {
      log.error('Failed to load directory', error);
      setDropdownItems([]);
    } finally {
      setDropdownLoading(false);
    }
  }, []);

  // Handle segment click
  const handleSegmentClick = useCallback((segment: PathSegment, event: React.MouseEvent) => {
    event.stopPropagation();
    event.preventDefault();
    
    const target = event.currentTarget as HTMLElement;
    
    if (openDropdown === segment.fullPath) {
      setOpenDropdown(null);
      setAnchorEl(null);
    } else {
      setOpenDropdown(segment.fullPath);
      setAnchorEl(target);
      
      const dirPath = segment.isFile 
        ? segment.fullPath.substring(0, segment.fullPath.lastIndexOf('/'))
        : segment.fullPath;
      
      setInitialDirPath(dirPath);
      loadDirectoryContents(dirPath);
    }
  }, [openDropdown, loadDirectoryContents]);

  // Handle dropdown item selection
  const handleDropdownSelect = useCallback(async (item: FileItem) => {
    if (item.isDirectory) {
      loadDirectoryContents(item.path);
    } else {
      setOpenDropdown(null);
      setAnchorEl(null);
      
      const { fileTabManager } = await import('@/shared/services/FileTabManager');
      fileTabManager.openFile({
        filePath: item.path,
        fileName: item.name,
        workspacePath
      });
    }
  }, [loadDirectoryContents, workspacePath]);

  const handleGoBack = useCallback(() => {
    const parentPath = getParentPath(currentDirPath);
    if (parentPath) {
      loadDirectoryContents(parentPath);
    }
  }, [currentDirPath, loadDirectoryContents]);

  const handleCloseDropdown = useCallback(() => {
    setOpenDropdown(null);
    setAnchorEl(null);
  }, []);

  const setItemRef = useCallback((path: string, el: HTMLSpanElement | null) => {
    if (el) {
      itemRefs.current.set(path, el);
    } else {
      itemRefs.current.delete(path);
    }
  }, []);

  if (segments.length === 0) {
    return null;
  }

  const maxVisibleSegments = 6;
  let displaySegments: (PathSegment | { name: string; isEllipsis: true })[] = segments;
  
  if (segments.length > maxVisibleSegments) {
    displaySegments = [
      segments[0],
      { name: '…', isEllipsis: true },
      ...segments.slice(-4)
    ];
  }

  return (
    <nav className={`editor-breadcrumb ${className}`}>
      {displaySegments.map((segment, index) => {
        const isEllipsis = 'isEllipsis' in segment && segment.isEllipsis;
        const pathSegment = segment as PathSegment;
        const isDropdownOpen = openDropdown === pathSegment.fullPath;

        return (
          <React.Fragment key={isEllipsis ? 'ellipsis' : pathSegment.fullPath}>
            {index > 0 && (
              <ChevronRight 
                size={10} 
                className="editor-breadcrumb__separator" 
              />
            )}
            
            {isEllipsis ? (
              <span className="editor-breadcrumb__item editor-breadcrumb__item--ellipsis">
                {segment.name}
              </span>
            ) : (
              <Tooltip content={pathSegment.fullPath} placement="bottom">
                <span
                  ref={(el) => setItemRef(pathSegment.fullPath, el)}
                  className={`editor-breadcrumb__item ${
                    pathSegment.isFile 
                      ? 'editor-breadcrumb__item--file' 
                      : 'editor-breadcrumb__item--folder'
                  } editor-breadcrumb__item--clickable ${isDropdownOpen ? 'editor-breadcrumb__item--active' : ''}`}
                  onClick={(e) => handleSegmentClick(pathSegment, e)}
                >
                  <span className="editor-breadcrumb__item-icon">
                    {pathSegment.isFile ? (
                      getFileIconComponent(pathSegment.name)
                    ) : (
                      <Folder size={12} />
                    )}
                  </span>
                  <span className="editor-breadcrumb__item-text">
                    {pathSegment.name}
                  </span>
                </span>
              </Tooltip>
            )}
          </React.Fragment>
        );
      })}
      
      <DropdownMenu
        isOpen={openDropdown !== null}
        items={dropdownItems}
        loading={dropdownLoading}
        currentDirPath={currentDirPath}
        initialDirPath={initialDirPath}
        onSelect={handleDropdownSelect}
        onGoBack={handleGoBack}
        onClose={handleCloseDropdown}
        anchorEl={anchorEl}
        currentFilePath={filePath}
        workspacePath={workspacePath}
      />
    </nav>
  );
};

EditorBreadcrumb.displayName = 'EditorBreadcrumb';

export default EditorBreadcrumb;
