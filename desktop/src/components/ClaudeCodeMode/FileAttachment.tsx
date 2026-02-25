/**
 * FileAttachment Component
 *
 * Drag & drop file attachment and @ file reference autocomplete.
 * Supports text files, images, and PDFs with configurable size limits.
 *
 * Story 011-3: File Attachment and @ File References
 */

import { useState, useCallback, useRef, useEffect, memo } from 'react';
import { clsx } from 'clsx';
import { FileIcon, Cross2Icon, UploadIcon, ImageIcon, FileTextIcon, MagnifyingGlassIcon } from '@radix-ui/react-icons';
import Fuse from 'fuse.js';

// ============================================================================
// Types
// ============================================================================

export interface FileAttachmentData {
  id: string;
  name: string;
  path: string;
  size: number;
  type: 'text' | 'image' | 'pdf' | 'unknown';
  content?: string;
  preview?: string;
}

export interface FileReference {
  id: string;
  path: string;
  name: string;
}

interface FileAttachmentProps {
  attachments: FileAttachmentData[];
  onAttach: (files: FileAttachmentData[]) => void;
  onRemove: (id: string) => void;
  maxSize?: number; // in bytes, default 10MB
  disabled?: boolean;
}

interface FileReferenceAutocompleteProps {
  isOpen: boolean;
  searchQuery: string;
  files: FileReference[];
  onSelect: (file: FileReference) => void;
  onClose: () => void;
  position: { top: number; left: number };
}

interface FileChipProps {
  file: FileAttachmentData | FileReference;
  onRemove?: () => void;
  isReference?: boolean;
}

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_MAX_SIZE = 10 * 1024 * 1024; // 10MB

const SUPPORTED_TEXT_EXTENSIONS = [
  'txt',
  'md',
  'json',
  'js',
  'ts',
  'jsx',
  'tsx',
  'py',
  'rb',
  'go',
  'rs',
  'java',
  'kt',
  'swift',
  'c',
  'cpp',
  'h',
  'hpp',
  'cs',
  'php',
  'html',
  'css',
  'scss',
  'less',
  'xml',
  'yaml',
  'yml',
  'toml',
  'ini',
  'cfg',
  'sh',
  'bash',
  'zsh',
  'ps1',
  'sql',
  'graphql',
  'vue',
  'svelte',
];

const SUPPORTED_IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg'];
const SUPPORTED_PDF_EXTENSIONS = ['pdf'];

// ============================================================================
// Helper Functions
// ============================================================================

function getFileType(filename: string): FileAttachmentData['type'] {
  const ext = filename.split('.').pop()?.toLowerCase() || '';

  if (SUPPORTED_TEXT_EXTENSIONS.includes(ext)) return 'text';
  if (SUPPORTED_IMAGE_EXTENSIONS.includes(ext)) return 'image';
  if (SUPPORTED_PDF_EXTENSIONS.includes(ext)) return 'pdf';
  return 'unknown';
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

// ============================================================================
// FileAttachmentDropZone Component
// ============================================================================

export const FileAttachmentDropZone = memo(function FileAttachmentDropZone({
  attachments,
  onAttach,
  onRemove,
  maxSize = DEFAULT_MAX_SIZE,
  disabled = false,
}: FileAttachmentProps) {
  const [isDragging, setIsDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dropZoneRef = useRef<HTMLDivElement>(null);

  const handleDragEnter = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (!disabled) {
        setIsDragging(true);
      }
    },
    [disabled],
  );

  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    // Only set dragging to false if we're leaving the drop zone entirely
    if (e.target === dropZoneRef.current) {
      setIsDragging(false);
    }
  }, []);

  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const processFiles = useCallback(
    async (files: FileList | File[]) => {
      setError(null);
      const newAttachments: FileAttachmentData[] = [];
      const errors: string[] = [];

      for (const file of Array.from(files)) {
        // Check file size
        if (file.size > maxSize) {
          errors.push(`${file.name}: File too large (max ${formatFileSize(maxSize)})`);
          continue;
        }

        const fileType = getFileType(file.name);

        // Read file content for text files
        let content: string | undefined;
        let preview: string | undefined;

        if (fileType === 'text') {
          try {
            content = await file.text();
            preview = content.slice(0, 200) + (content.length > 200 ? '...' : '');
          } catch {
            errors.push(`${file.name}: Failed to read file content`);
            continue;
          }
        } else if (fileType === 'image') {
          try {
            const reader = new FileReader();
            preview = await new Promise((resolve, reject) => {
              reader.onload = () => resolve(reader.result as string);
              reader.onerror = reject;
              reader.readAsDataURL(file);
            });
          } catch {
            // Image preview is optional
          }
        }

        newAttachments.push({
          id: generateId(),
          name: file.name,
          path: file.name, // In Electron/Tauri, we'd get the full path
          size: file.size,
          type: fileType,
          content,
          preview,
        });
      }

      if (errors.length > 0) {
        setError(errors.join('\n'));
      }

      if (newAttachments.length > 0) {
        onAttach(newAttachments);
      }
    },
    [maxSize, onAttach],
  );

  const handleDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);

      if (disabled) return;

      const files = e.dataTransfer?.files;
      if (files && files.length > 0) {
        processFiles(files);
      }
    },
    [disabled, processFiles],
  );

  // Set up drag and drop listeners
  useEffect(() => {
    const element = dropZoneRef.current;
    if (!element) return;

    element.addEventListener('dragenter', handleDragEnter);
    element.addEventListener('dragleave', handleDragLeave);
    element.addEventListener('dragover', handleDragOver);
    element.addEventListener('drop', handleDrop);

    return () => {
      element.removeEventListener('dragenter', handleDragEnter);
      element.removeEventListener('dragleave', handleDragLeave);
      element.removeEventListener('dragover', handleDragOver);
      element.removeEventListener('drop', handleDrop);
    };
  }, [handleDragEnter, handleDragLeave, handleDragOver, handleDrop]);

  if (attachments.length === 0 && !isDragging) {
    return (
      <div
        ref={dropZoneRef}
        className={clsx('relative transition-all duration-200', disabled && 'opacity-50 cursor-not-allowed')}
      />
    );
  }

  return (
    <div
      ref={dropZoneRef}
      className={clsx('relative transition-all duration-200', isDragging && 'ring-2 ring-primary-500 ring-offset-2')}
    >
      {/* Drag overlay */}
      {isDragging && (
        <div
          className={clsx(
            'absolute inset-0 z-10',
            'bg-primary-100/90 dark:bg-primary-900/90',
            'flex flex-col items-center justify-center',
            'border-2 border-dashed border-primary-500',
            'rounded-lg',
          )}
        >
          <UploadIcon className="w-8 h-8 text-primary-600 dark:text-primary-400 mb-2" />
          <p className="text-sm font-medium text-primary-700 dark:text-primary-300">Drop files here</p>
        </div>
      )}

      {/* Attachment chips */}
      {attachments.length > 0 && (
        <div className="flex flex-wrap gap-2 p-2 border-b border-gray-200 dark:border-gray-700">
          {attachments.map((file) => (
            <FileChip key={file.id} file={file} onRemove={() => onRemove(file.id)} />
          ))}
        </div>
      )}

      {/* Error message */}
      {error && (
        <div className="px-2 py-1 text-xs text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-900/20">{error}</div>
      )}
    </div>
  );
});

// ============================================================================
// FileChip Component
// ============================================================================

export const FileChip = memo(function FileChip({ file, onRemove, isReference = false }: FileChipProps) {
  const Icon =
    'type' in file ? (file.type === 'image' ? ImageIcon : file.type === 'text' ? FileTextIcon : FileIcon) : FileIcon;

  return (
    <div
      className={clsx(
        'inline-flex items-center gap-1.5 px-2 py-1 rounded-md',
        'text-xs font-medium',
        isReference
          ? 'bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300'
          : 'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300',
        'border border-transparent hover:border-gray-300 dark:hover:border-gray-600',
        'transition-colors',
      )}
    >
      <Icon className="w-3.5 h-3.5 flex-shrink-0" />
      <span className="truncate max-w-[150px]">{file.name}</span>
      {'size' in file && <span className="text-gray-500 dark:text-gray-400">({formatFileSize(file.size)})</span>}
      {onRemove && (
        <button
          onClick={onRemove}
          className={clsx('ml-1 p-0.5 rounded hover:bg-gray-200 dark:hover:bg-gray-700', 'transition-colors')}
        >
          <Cross2Icon className="w-3 h-3" />
        </button>
      )}
    </div>
  );
});

// ============================================================================
// FileReferenceAutocomplete Component
// ============================================================================

export const FileReferenceAutocomplete = memo(function FileReferenceAutocomplete({
  isOpen,
  searchQuery,
  files,
  onSelect,
  onClose,
  position,
}: FileReferenceAutocompleteProps) {
  const [selectedIndex, setSelectedIndex] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);

  // Fuzzy search using Fuse.js
  const fuse = new Fuse(files, {
    keys: ['name', 'path'],
    threshold: 0.4,
    includeScore: true,
  });

  const filteredFiles = searchQuery ? fuse.search(searchQuery).map((result) => result.item) : files.slice(0, 10);

  // Reset selection when query changes
  useEffect(() => {
    setSelectedIndex(0);
  }, [searchQuery]);

  // Handle keyboard navigation
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          setSelectedIndex((prev) => (prev < filteredFiles.length - 1 ? prev + 1 : prev));
          break;
        case 'ArrowUp':
          e.preventDefault();
          setSelectedIndex((prev) => (prev > 0 ? prev - 1 : prev));
          break;
        case 'Enter':
          e.preventDefault();
          if (filteredFiles[selectedIndex]) {
            onSelect(filteredFiles[selectedIndex]);
          }
          break;
        case 'Escape':
          e.preventDefault();
          onClose();
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, filteredFiles, selectedIndex, onSelect, onClose]);

  // Scroll selected item into view
  useEffect(() => {
    if (listRef.current) {
      const selectedElement = listRef.current.children[selectedIndex] as HTMLElement;
      if (selectedElement) {
        selectedElement.scrollIntoView({ block: 'nearest' });
      }
    }
  }, [selectedIndex]);

  if (!isOpen || filteredFiles.length === 0) return null;

  return (
    <div
      className={clsx(
        'absolute z-50 w-72 max-h-64 overflow-auto',
        'bg-white dark:bg-gray-800',
        'border border-gray-200 dark:border-gray-700',
        'rounded-lg shadow-lg',
        'animate-in fade-in-0 zoom-in-95',
      )}
      style={{ top: position.top, left: position.left }}
    >
      <div className="sticky top-0 px-3 py-2 bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
          <MagnifyingGlassIcon className="w-3.5 h-3.5" />
          <span>{searchQuery ? `Searching for "${searchQuery}"` : 'Recent files'}</span>
        </div>
      </div>

      <div ref={listRef} className="py-1">
        {filteredFiles.map((file, index) => (
          <button
            key={file.id}
            onClick={() => onSelect(file)}
            className={clsx(
              'w-full flex items-center gap-2 px-3 py-2 text-left',
              'transition-colors',
              index === selectedIndex
                ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-900 dark:text-primary-100'
                : 'hover:bg-gray-100 dark:hover:bg-gray-700',
            )}
          >
            <FileIcon className="w-4 h-4 flex-shrink-0 text-gray-400" />
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium truncate">{file.name}</div>
              <div className="text-xs text-gray-500 dark:text-gray-400 truncate">{file.path}</div>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
});

// ============================================================================
// useFileReferences Hook
// ============================================================================

export interface UseFileReferencesResult {
  references: FileReference[];
  isAutocompleteOpen: boolean;
  autocompleteQuery: string;
  autocompletePosition: { top: number; left: number };
  handleInputChange: (value: string, cursorPosition: number) => void;
  handleSelectFile: (file: FileReference) => string;
  addReference: (file: FileReference) => void;
  removeReference: (id: string) => void;
  closeAutocomplete: () => void;
}

export function useFileReferences(_workspaceFiles: FileReference[] = []): UseFileReferencesResult {
  const [references, setReferences] = useState<FileReference[]>([]);
  const [isAutocompleteOpen, setIsAutocompleteOpen] = useState(false);
  const [autocompleteQuery, setAutocompleteQuery] = useState('');
  const [autocompletePosition, setAutocompletePosition] = useState({ top: 0, left: 0 });

  const handleInputChange = useCallback((value: string, cursorPosition: number) => {
    // Check if there's an @ symbol before cursor
    const beforeCursor = value.slice(0, cursorPosition);
    const atIndex = beforeCursor.lastIndexOf('@');

    if (atIndex >= 0) {
      // Check if there's no space between @ and cursor
      const afterAt = beforeCursor.slice(atIndex + 1);
      if (!afterAt.includes(' ')) {
        setAutocompleteQuery(afterAt);
        setIsAutocompleteOpen(true);
        // Position would be calculated based on cursor position
        // This is a simplified version
        setAutocompletePosition({ top: -200, left: 0 });
        return;
      }
    }

    setIsAutocompleteOpen(false);
  }, []);

  const handleSelectFile = useCallback((file: FileReference): string => {
    setReferences((prev) => [...prev, file]);
    setIsAutocompleteOpen(false);
    setAutocompleteQuery('');
    // Return the text to replace the @ mention with
    return `@${file.name} `;
  }, []);

  const addReference = useCallback((file: FileReference) => {
    setReferences((prev) => [...prev, file]);
  }, []);

  const removeReference = useCallback((id: string) => {
    setReferences((prev) => prev.filter((r) => r.id !== id));
  }, []);

  const closeAutocomplete = useCallback(() => {
    setIsAutocompleteOpen(false);
    setAutocompleteQuery('');
  }, []);

  return {
    references,
    isAutocompleteOpen,
    autocompleteQuery,
    autocompletePosition,
    handleInputChange,
    handleSelectFile,
    addReference,
    removeReference,
    closeAutocomplete,
  };
}

export default FileAttachmentDropZone;
