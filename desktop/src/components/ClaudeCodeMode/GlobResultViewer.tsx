/**
 * GlobResultViewer Component
 *
 * Specialized result viewer for Glob tool results. Shows matched files
 * in a navigable tree structure grouped by directory with filtering and sorting.
 *
 * Story-005: Glob and Grep result viewer with file list
 */

import { useState, useMemo, useCallback } from 'react';
import { clsx } from 'clsx';
import {
  FileIcon,
  FileTextIcon,
  CodeIcon,
  ImageIcon,
  GearIcon,
  ChevronRightIcon,
  ChevronDownIcon,
  MagnifyingGlassIcon,
  CopyIcon,
  CheckIcon,
  CaretSortIcon,
  Cross2Icon,
} from '@radix-ui/react-icons';

// ============================================================================
// Types
// ============================================================================

interface GlobResultViewerProps {
  /** List of matched file paths */
  files: string[];
  /** The glob pattern used */
  pattern?: string;
  /** Callback when a file is clicked (for preview integration) */
  onFileClick?: (filePath: string) => void;
  /** Maximum height in pixels */
  maxHeight?: number;
  /** Additional CSS classes */
  className?: string;
}

type SortOption = 'name' | 'path-depth' | 'extension';

interface TreeNode {
  name: string;
  path: string;
  isDirectory: boolean;
  children: Map<string, TreeNode>;
  fileCount: number;
}

// ============================================================================
// File Icon Helper
// ============================================================================

const FILE_ICON_MAP: Record<string, { Icon: typeof FileIcon; color: string }> = {
  // Code files
  ts: { Icon: CodeIcon, color: 'text-blue-500' },
  tsx: { Icon: CodeIcon, color: 'text-blue-500' },
  js: { Icon: CodeIcon, color: 'text-yellow-500' },
  jsx: { Icon: CodeIcon, color: 'text-yellow-500' },
  py: { Icon: CodeIcon, color: 'text-green-500' },
  rs: { Icon: CodeIcon, color: 'text-orange-500' },
  go: { Icon: CodeIcon, color: 'text-cyan-500' },
  java: { Icon: CodeIcon, color: 'text-red-500' },
  cpp: { Icon: CodeIcon, color: 'text-purple-500' },
  c: { Icon: CodeIcon, color: 'text-purple-400' },
  rb: { Icon: CodeIcon, color: 'text-red-400' },
  php: { Icon: CodeIcon, color: 'text-indigo-500' },
  swift: { Icon: CodeIcon, color: 'text-orange-400' },
  kt: { Icon: CodeIcon, color: 'text-purple-400' },

  // Config files
  json: { Icon: GearIcon, color: 'text-yellow-400' },
  yaml: { Icon: GearIcon, color: 'text-pink-400' },
  yml: { Icon: GearIcon, color: 'text-pink-400' },
  toml: { Icon: GearIcon, color: 'text-gray-400' },
  xml: { Icon: GearIcon, color: 'text-orange-400' },
  env: { Icon: GearIcon, color: 'text-green-400' },

  // Text files
  md: { Icon: FileTextIcon, color: 'text-blue-400' },
  txt: { Icon: FileTextIcon, color: 'text-gray-400' },
  log: { Icon: FileTextIcon, color: 'text-gray-400' },
  csv: { Icon: FileTextIcon, color: 'text-green-400' },

  // Image files
  png: { Icon: ImageIcon, color: 'text-purple-400' },
  jpg: { Icon: ImageIcon, color: 'text-purple-400' },
  jpeg: { Icon: ImageIcon, color: 'text-purple-400' },
  gif: { Icon: ImageIcon, color: 'text-purple-400' },
  svg: { Icon: ImageIcon, color: 'text-orange-400' },
  ico: { Icon: ImageIcon, color: 'text-blue-400' },
};

function getFileIcon(filename: string): { Icon: typeof FileIcon; color: string } {
  const ext = filename.split('.').pop()?.toLowerCase() || '';
  return FILE_ICON_MAP[ext] || { Icon: FileIcon, color: 'text-gray-400' };
}

// ============================================================================
// Tree Building Helper
// ============================================================================

function buildFileTree(files: string[]): TreeNode {
  const root: TreeNode = {
    name: '',
    path: '',
    isDirectory: true,
    children: new Map(),
    fileCount: 0,
  };

  files.forEach(filePath => {
    // Normalize path separators
    const normalizedPath = filePath.replace(/\\/g, '/');
    const parts = normalizedPath.split('/').filter(Boolean);

    let currentNode = root;
    let currentPath = '';

    parts.forEach((part, index) => {
      currentPath = currentPath ? `${currentPath}/${part}` : part;
      const isFile = index === parts.length - 1;

      if (!currentNode.children.has(part)) {
        currentNode.children.set(part, {
          name: part,
          path: currentPath,
          isDirectory: !isFile,
          children: new Map(),
          fileCount: 0,
        });
      }

      if (!isFile) {
        currentNode = currentNode.children.get(part)!;
        currentNode.fileCount++;
      }
    });

    root.fileCount++;
  });

  return root;
}

// ============================================================================
// Sorting Helper
// ============================================================================

function sortFiles(files: string[], sortBy: SortOption): string[] {
  return [...files].sort((a, b) => {
    switch (sortBy) {
      case 'name': {
        const nameA = a.split(/[/\\]/).pop() || a;
        const nameB = b.split(/[/\\]/).pop() || b;
        return nameA.localeCompare(nameB);
      }
      case 'path-depth': {
        const depthA = a.split(/[/\\]/).length;
        const depthB = b.split(/[/\\]/).length;
        return depthA - depthB || a.localeCompare(b);
      }
      case 'extension': {
        const extA = a.split('.').pop() || '';
        const extB = b.split('.').pop() || '';
        return extA.localeCompare(extB) || a.localeCompare(b);
      }
      default:
        return 0;
    }
  });
}

// ============================================================================
// TreeNodeItem Component
// ============================================================================

interface TreeNodeItemProps {
  node: TreeNode;
  depth: number;
  expandedPaths: Set<string>;
  toggleExpanded: (path: string) => void;
  onFileClick?: (filePath: string) => void;
  filter: string;
}

function TreeNodeItem({
  node,
  depth,
  expandedPaths,
  toggleExpanded,
  onFileClick,
  filter,
}: TreeNodeItemProps) {
  const isExpanded = expandedPaths.has(node.path);
  const hasChildren = node.children.size > 0;

  // Filter matching
  const matchesFilter = !filter || node.name.toLowerCase().includes(filter.toLowerCase());

  // Get visible children
  const visibleChildren = useMemo(() => {
    const children = Array.from(node.children.values());

    if (!filter) return children;

    // Show children that match or have matching descendants
    return children.filter(child => {
      if (child.name.toLowerCase().includes(filter.toLowerCase())) return true;
      if (child.isDirectory) {
        // Check if any descendant matches
        const checkDescendants = (n: TreeNode): boolean => {
          if (n.name.toLowerCase().includes(filter.toLowerCase())) return true;
          for (const child of n.children.values()) {
            if (checkDescendants(child)) return true;
          }
          return false;
        };
        return checkDescendants(child);
      }
      return false;
    });
  }, [node.children, filter]);

  // Sort children: directories first, then files
  const sortedChildren = useMemo(() => {
    return visibleChildren.sort((a, b) => {
      if (a.isDirectory && !b.isDirectory) return -1;
      if (!a.isDirectory && b.isDirectory) return 1;
      return a.name.localeCompare(b.name);
    });
  }, [visibleChildren]);

  // Skip non-matching nodes in filter mode
  if (filter && !matchesFilter && visibleChildren.length === 0) {
    return null;
  }

  // Render file
  if (!node.isDirectory) {
    const { Icon, color } = getFileIcon(node.name);

    return (
      <button
        onClick={() => onFileClick?.(node.path)}
        className={clsx(
          'w-full flex items-center gap-2 px-2 py-1.5 rounded',
          'hover:bg-gray-100 dark:hover:bg-gray-700',
          'transition-colors text-left'
        )}
        style={{ paddingLeft: `${depth * 16 + 8}px` }}
      >
        <Icon className={clsx('w-4 h-4 flex-shrink-0', color)} />
        <span className={clsx(
          'text-sm truncate',
          filter && matchesFilter && 'font-medium text-primary-600 dark:text-primary-400'
        )}>
          {node.name}
        </span>
      </button>
    );
  }

  // Render directory
  return (
    <div>
      <button
        onClick={() => toggleExpanded(node.path)}
        className={clsx(
          'w-full flex items-center gap-2 px-2 py-1.5 rounded',
          'hover:bg-gray-100 dark:hover:bg-gray-700',
          'transition-colors text-left font-medium'
        )}
        style={{ paddingLeft: `${depth * 16 + 8}px` }}
      >
        {hasChildren ? (
          isExpanded ? (
            <ChevronDownIcon className="w-4 h-4 text-gray-400" />
          ) : (
            <ChevronRightIcon className="w-4 h-4 text-gray-400" />
          )
        ) : (
          <span className="w-4" />
        )}
        <span className={clsx(
          'text-sm text-gray-700 dark:text-gray-300',
          filter && matchesFilter && 'text-primary-600 dark:text-primary-400'
        )}>
          {node.name}
        </span>
        <span className="text-xs text-gray-400 ml-1">
          ({node.fileCount})
        </span>
      </button>

      {isExpanded && sortedChildren.length > 0 && (
        <div>
          {sortedChildren.map(child => (
            <TreeNodeItem
              key={child.path}
              node={child}
              depth={depth + 1}
              expandedPaths={expandedPaths}
              toggleExpanded={toggleExpanded}
              onFileClick={onFileClick}
              filter={filter}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// GlobResultViewer Component
// ============================================================================

export function GlobResultViewer({
  files,
  pattern,
  onFileClick,
  maxHeight = 300,
  className,
}: GlobResultViewerProps) {
  // State
  const [filter, setFilter] = useState('');
  const [sortBy, setSortBy] = useState<SortOption>('name');
  const [viewMode, setViewMode] = useState<'tree' | 'list'>('tree');
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [copied, setCopied] = useState(false);

  // Build tree from files
  const fileTree = useMemo(() => buildFileTree(files), [files]);

  // Sorted and filtered files for list view
  const sortedFiles = useMemo(() => {
    const sorted = sortFiles(files, sortBy);
    if (!filter) return sorted;
    return sorted.filter(f => f.toLowerCase().includes(filter.toLowerCase()));
  }, [files, sortBy, filter]);

  // Toggle expanded state
  const toggleExpanded = useCallback((path: string) => {
    setExpandedPaths(prev => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  // Expand all directories
  const expandAll = useCallback(() => {
    const allPaths = new Set<string>();
    const collectPaths = (node: TreeNode) => {
      if (node.isDirectory && node.path) {
        allPaths.add(node.path);
      }
      node.children.forEach(child => collectPaths(child));
    };
    collectPaths(fileTree);
    setExpandedPaths(allPaths);
  }, [fileTree]);

  // Collapse all directories
  const collapseAll = useCallback(() => {
    setExpandedPaths(new Set());
  }, []);

  // Copy file list
  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(sortedFiles.join('\n'));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  }, [sortedFiles]);

  // Auto-expand when filtering
  useMemo(() => {
    if (filter) {
      expandAll();
    }
  }, [filter, expandAll]);

  return (
    <div className={clsx('flex flex-col', className)}>
      {/* Header with count */}
      <div className={clsx(
        'flex items-center justify-between px-3 py-2',
        'bg-gray-50 dark:bg-gray-800/50',
        'border-b border-gray-200 dark:border-gray-700',
        'rounded-t-lg'
      )}>
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
            {files.length} file{files.length !== 1 ? 's' : ''} matched
          </span>
          {pattern && (
            <code className="text-xs px-1.5 py-0.5 rounded bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-400">
              {pattern}
            </code>
          )}
        </div>

        <div className="flex items-center gap-1">
          {/* View mode toggle */}
          <button
            onClick={() => setViewMode(viewMode === 'tree' ? 'list' : 'tree')}
            className={clsx(
              'p-1.5 rounded text-gray-500',
              'hover:bg-gray-200 dark:hover:bg-gray-700',
              'transition-colors'
            )}
            title={viewMode === 'tree' ? 'Switch to list view' : 'Switch to tree view'}
          >
            {viewMode === 'tree' ? (
              <FileTextIcon className="w-4 h-4" />
            ) : (
              <ChevronRightIcon className="w-4 h-4" />
            )}
          </button>

          {/* Copy button */}
          <button
            onClick={handleCopy}
            className={clsx(
              'p-1.5 rounded transition-colors',
              copied
                ? 'text-green-500'
                : 'text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700'
            )}
            title={copied ? 'Copied!' : 'Copy file list'}
          >
            {copied ? <CheckIcon className="w-4 h-4" /> : <CopyIcon className="w-4 h-4" />}
          </button>
        </div>
      </div>

      {/* Filter and sort controls */}
      <div className={clsx(
        'flex items-center gap-2 px-3 py-2',
        'border-b border-gray-200 dark:border-gray-700'
      )}>
        {/* Filter input */}
        <div className="relative flex-1">
          <MagnifyingGlassIcon className="absolute left-2 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
          <input
            type="text"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Filter files..."
            className={clsx(
              'w-full pl-8 pr-8 py-1.5 rounded text-sm',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'focus:outline-none focus:ring-2 focus:ring-primary-500'
            )}
          />
          {filter && (
            <button
              onClick={() => setFilter('')}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
            >
              <Cross2Icon className="w-4 h-4" />
            </button>
          )}
        </div>

        {/* Sort dropdown */}
        <div className="relative">
          <select
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as SortOption)}
            className={clsx(
              'appearance-none pl-2 pr-7 py-1.5 rounded text-sm',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'focus:outline-none focus:ring-2 focus:ring-primary-500'
            )}
          >
            <option value="name">By name</option>
            <option value="path-depth">By depth</option>
            <option value="extension">By type</option>
          </select>
          <CaretSortIcon className="absolute right-1.5 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 pointer-events-none" />
        </div>

        {/* Expand/Collapse buttons (tree view only) */}
        {viewMode === 'tree' && (
          <div className="flex gap-1">
            <button
              onClick={expandAll}
              className="p-1.5 rounded text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700"
              title="Expand all"
            >
              <ChevronDownIcon className="w-4 h-4" />
            </button>
            <button
              onClick={collapseAll}
              className="p-1.5 rounded text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700"
              title="Collapse all"
            >
              <ChevronRightIcon className="w-4 h-4" />
            </button>
          </div>
        )}
      </div>

      {/* File list/tree */}
      <div
        className="overflow-auto"
        style={{ maxHeight }}
      >
        {viewMode === 'tree' ? (
          <div className="py-1">
            {Array.from(fileTree.children.values())
              .sort((a, b) => {
                if (a.isDirectory && !b.isDirectory) return -1;
                if (!a.isDirectory && b.isDirectory) return 1;
                return a.name.localeCompare(b.name);
              })
              .map(node => (
                <TreeNodeItem
                  key={node.path}
                  node={node}
                  depth={0}
                  expandedPaths={expandedPaths}
                  toggleExpanded={toggleExpanded}
                  onFileClick={onFileClick}
                  filter={filter}
                />
              ))}
          </div>
        ) : (
          <div className="divide-y divide-gray-100 dark:divide-gray-800">
            {sortedFiles.map((file, index) => {
              const filename = file.split(/[/\\]/).pop() || file;
              const { Icon, color } = getFileIcon(filename);

              return (
                <button
                  key={index}
                  onClick={() => onFileClick?.(file)}
                  className={clsx(
                    'w-full flex items-center gap-2 px-3 py-2',
                    'hover:bg-gray-50 dark:hover:bg-gray-800',
                    'transition-colors text-left'
                  )}
                >
                  <Icon className={clsx('w-4 h-4 flex-shrink-0', color)} />
                  <span className="text-sm font-mono truncate">
                    {file}
                  </span>
                </button>
              );
            })}
          </div>
        )}

        {sortedFiles.length === 0 && (
          <div className="flex flex-col items-center justify-center py-8 text-gray-500">
            <MagnifyingGlassIcon className="w-8 h-8 mb-2 opacity-50" />
            <p className="text-sm">No files match the filter</p>
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Exports
// ============================================================================

export { getFileIcon, buildFileTree };
export default GlobResultViewer;
