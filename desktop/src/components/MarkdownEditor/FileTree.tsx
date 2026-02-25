/**
 * FileTree Component
 *
 * Displays discovered CLAUDE.md files in a hierarchical tree structure.
 * Supports expand/collapse for directories and file selection.
 */

import { useMemo, useState, useCallback, useEffect } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { FileTextIcon, ReloadIcon, ChevronRightIcon, ChevronDownIcon, PlusIcon } from '@radix-ui/react-icons';
import type { ClaudeMdFile } from '../../types/markdown';

interface FileTreeProps {
  files: ClaudeMdFile[];
  selectedFile: ClaudeMdFile | null;
  loading: boolean;
  onSelectFile: (file: ClaudeMdFile) => void;
  onRefresh: () => void;
  onCreateFile?: () => void;
}

/** Tree node that can be either a directory or a file */
interface TreeNode {
  name: string;
  path: string;
  isDirectory: boolean;
  file?: ClaudeMdFile;
  children: Map<string, TreeNode>;
}

/** Build a tree structure from flat file list */
function buildFileTree(files: ClaudeMdFile[]): TreeNode {
  const root: TreeNode = {
    name: 'root',
    path: '',
    isDirectory: true,
    children: new Map(),
  };

  for (const file of files) {
    // Split the relative path into parts
    const parts = file.relative_path.split(/[/\\]/).filter(Boolean);

    let current = root;

    // Navigate/create directory nodes
    for (let i = 0; i < parts.length - 1; i++) {
      const part = parts[i];
      if (!current.children.has(part)) {
        current.children.set(part, {
          name: part,
          path: parts.slice(0, i + 1).join('/'),
          isDirectory: true,
          children: new Map(),
        });
      }
      current = current.children.get(part)!;
    }

    // Add the file node
    const fileName = parts[parts.length - 1] || file.name;
    current.children.set(fileName, {
      name: fileName,
      path: file.path,
      isDirectory: false,
      file,
      children: new Map(),
    });
  }

  return root;
}

interface TreeNodeComponentProps {
  node: TreeNode;
  depth: number;
  selectedPath: string | null;
  expandedPaths: Set<string>;
  onSelect: (file: ClaudeMdFile) => void;
  onToggleExpand: (path: string) => void;
}

function TreeNodeComponent({
  node,
  depth,
  selectedPath,
  expandedPaths,
  onSelect,
  onToggleExpand,
}: TreeNodeComponentProps) {
  const isExpanded = expandedPaths.has(node.path);
  const isSelected = node.file && node.file.path === selectedPath;

  // Sort children: directories first, then files, alphabetically
  const sortedChildren = useMemo(() => {
    const children = Array.from(node.children.values());
    return children.sort((a, b) => {
      if (a.isDirectory && !b.isDirectory) return -1;
      if (!a.isDirectory && b.isDirectory) return 1;
      return a.name.localeCompare(b.name);
    });
  }, [node.children]);

  if (node.isDirectory) {
    return (
      <div>
        <button
          onClick={() => onToggleExpand(node.path)}
          className={clsx(
            'w-full flex items-center gap-1.5 px-2 py-1.5 rounded-md',
            'text-sm text-gray-700 dark:text-gray-300',
            'hover:bg-gray-100 dark:hover:bg-gray-700',
            'transition-colors',
          )}
          style={{ paddingLeft: `${depth * 12 + 8}px` }}
        >
          {isExpanded ? (
            <ChevronDownIcon className="w-3.5 h-3.5 text-gray-400 flex-shrink-0" />
          ) : (
            <ChevronRightIcon className="w-3.5 h-3.5 text-gray-400 flex-shrink-0" />
          )}
          <span className="truncate font-medium">{node.name}</span>
        </button>
        {isExpanded && (
          <div>
            {sortedChildren.map((child) => (
              <TreeNodeComponent
                key={child.path}
                node={child}
                depth={depth + 1}
                selectedPath={selectedPath}
                expandedPaths={expandedPaths}
                onSelect={onSelect}
                onToggleExpand={onToggleExpand}
              />
            ))}
          </div>
        )}
      </div>
    );
  }

  // File node
  return (
    <button
      onClick={() => node.file && onSelect(node.file)}
      className={clsx(
        'w-full flex items-center gap-1.5 px-2 py-1.5 rounded-md',
        'text-sm transition-colors',
        isSelected
          ? 'bg-primary-100 dark:bg-primary-900/40 text-primary-700 dark:text-primary-300'
          : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700',
      )}
      style={{ paddingLeft: `${depth * 12 + 8}px` }}
    >
      <FileTextIcon className="w-3.5 h-3.5 flex-shrink-0" />
      <span className="truncate">{node.name}</span>
    </button>
  );
}

/** Loading skeleton for file tree */
function FileTreeSkeleton() {
  return (
    <div className="space-y-2 p-2">
      {[1, 2, 3, 4, 5].map((i) => (
        <div key={i} className="flex items-center gap-2 px-2 py-1.5" style={{ paddingLeft: `${(i % 3) * 12 + 8}px` }}>
          <div className="w-3.5 h-3.5 bg-gray-200 dark:bg-gray-700 rounded animate-pulse" />
          <div
            className="h-4 bg-gray-200 dark:bg-gray-700 rounded animate-pulse"
            style={{ width: `${60 + ((i * 20) % 60)}px` }}
          />
        </div>
      ))}
    </div>
  );
}

export function FileTree({ files, selectedFile, loading, onSelectFile, onRefresh, onCreateFile }: FileTreeProps) {
  const { t } = useTranslation();
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());

  // Build the tree structure
  const tree = useMemo(() => buildFileTree(files), [files]);

  // Auto-expand path to selected file
  useEffect(() => {
    if (selectedFile) {
      const parts = selectedFile.relative_path.split(/[/\\]/).filter(Boolean);
      const newExpanded = new Set(expandedPaths);
      let path = '';
      for (let i = 0; i < parts.length - 1; i++) {
        path = path ? `${path}/${parts[i]}` : parts[i];
        newExpanded.add(path);
      }
      setExpandedPaths(newExpanded);
    }
  }, [selectedFile]);

  // Toggle expand/collapse
  const handleToggleExpand = useCallback((path: string) => {
    setExpandedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  // Get sorted root children
  const rootChildren = useMemo(() => {
    const children = Array.from(tree.children.values());
    return children.sort((a, b) => {
      if (a.isDirectory && !b.isDirectory) return -1;
      if (!a.isDirectory && b.isDirectory) return 1;
      return a.name.localeCompare(b.name);
    });
  }, [tree.children]);

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-3 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between mb-2">
          <h3 className="text-sm font-semibold text-gray-900 dark:text-white">{t('markdownEditor.fileTree.title')}</h3>
          <div className="flex items-center gap-1">
            {onCreateFile && (
              <button
                onClick={onCreateFile}
                className={clsx(
                  'p-1.5 rounded-md',
                  'text-gray-500 dark:text-gray-400',
                  'hover:bg-gray-100 dark:hover:bg-gray-700',
                  'hover:text-gray-700 dark:hover:text-gray-300',
                  'transition-colors',
                )}
                title={t('markdownEditor.fileTree.createNew')}
              >
                <PlusIcon className="w-4 h-4" />
              </button>
            )}
            <button
              onClick={onRefresh}
              disabled={loading}
              className={clsx(
                'p-1.5 rounded-md',
                'text-gray-500 dark:text-gray-400',
                'hover:bg-gray-100 dark:hover:bg-gray-700',
                'hover:text-gray-700 dark:hover:text-gray-300',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors',
                loading && 'animate-spin',
              )}
              title={t('markdownEditor.fileTree.refresh')}
            >
              <ReloadIcon className="w-4 h-4" />
            </button>
          </div>
        </div>
        <p className="text-xs text-gray-500 dark:text-gray-400">
          {t('markdownEditor.fileTree.fileCount', { count: files.length })}
        </p>
      </div>

      {/* File List */}
      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <FileTreeSkeleton />
        ) : files.length === 0 ? (
          <div className="p-4 text-center">
            <FileTextIcon className="w-8 h-8 mx-auto mb-2 text-gray-400" />
            <p className="text-sm text-gray-500 dark:text-gray-400">{t('markdownEditor.fileTree.noFiles')}</p>
            <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">{t('markdownEditor.fileTree.noFilesHint')}</p>
          </div>
        ) : (
          <div className="py-1">
            {rootChildren.map((child) => (
              <TreeNodeComponent
                key={child.path}
                node={child}
                depth={0}
                selectedPath={selectedFile?.path || null}
                expandedPaths={expandedPaths}
                onSelect={onSelectFile}
                onToggleExpand={handleToggleExpand}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export default FileTree;
