/**
 * InputBox Component
 *
 * Text input with submit button for task descriptions.
 * Supports multiline input, keyboard shortcuts, loading state,
 * drag-and-drop file attachment, file picker, and @ file autocomplete.
 */

import { clsx } from 'clsx';
import {
  KeyboardEvent,
  DragEvent,
  useRef,
  useState,
  useCallback,
  useEffect,
} from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import {
  PaperPlaneIcon,
  UpdateIcon,
  FilePlusIcon,
  Cross2Icon,
  FileTextIcon,
  ImageIcon,
  FileIcon,
  UploadIcon,
} from '@radix-ui/react-icons';
import type { FileAttachmentData } from '../../types/attachment';

// ============================================================================
// Types
// ============================================================================

interface InputBoxProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  disabled?: boolean;
  placeholder?: string;
  isLoading?: boolean;
  attachments?: FileAttachmentData[];
  onAttach?: (file: FileAttachmentData) => void;
  onRemoveAttachment?: (id: string) => void;
  workspacePath?: string | null;
}

interface WorkspaceFileResult {
  name: string;
  path: string;
  size: number;
  is_dir: boolean;
}

interface FileContentResult {
  content: string;
  size: number;
  is_binary: boolean;
  mime_type: string;
}

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

// ============================================================================
// Constants
// ============================================================================

const LARGE_FILE_WARNING_SIZE = 1024 * 1024; // 1MB

// ============================================================================
// Helpers
// ============================================================================

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).substring(2, 11)}`;
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function getFileTypeFromMime(mimeType: string): FileAttachmentData['type'] {
  if (mimeType.startsWith('image/')) return 'image';
  if (mimeType === 'application/pdf') return 'pdf';
  if (
    mimeType.startsWith('text/') ||
    mimeType === 'application/json' ||
    mimeType === 'application/xml'
  ) {
    return 'text';
  }
  return 'unknown';
}

function getFileNameFromPath(filePath: string): string {
  const parts = filePath.replace(/\\/g, '/').split('/');
  return parts[parts.length - 1] || filePath;
}

// ============================================================================
// InputBox Component
// ============================================================================

export function InputBox({
  value,
  onChange,
  onSubmit,
  disabled = false,
  placeholder,
  isLoading = false,
  attachments = [],
  onAttach,
  onRemoveAttachment,
  workspacePath,
}: InputBoxProps) {
  const { t } = useTranslation('simpleMode');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const dropZoneRef = useRef<HTMLDivElement>(null);
  const autocompleteRef = useRef<HTMLDivElement>(null);
  const defaultPlaceholder = placeholder || t('input.placeholder');

  // Drag & drop state
  const [isDragging, setIsDragging] = useState(false);

  // File loading state
  const [isReadingFile, setIsReadingFile] = useState(false);
  const [fileError, setFileError] = useState<string | null>(null);

  // @ autocomplete state
  const [isAutocompleteOpen, setIsAutocompleteOpen] = useState(false);
  const [autocompleteQuery, setAutocompleteQuery] = useState('');
  const [autocompleteFiles, setAutocompleteFiles] = useState<WorkspaceFileResult[]>([]);
  const [autocompleteIndex, setAutocompleteIndex] = useState(0);
  const [atTriggerPos, setAtTriggerPos] = useState(-1);

  // Clear file error after timeout
  useEffect(() => {
    if (fileError) {
      const timer = setTimeout(() => setFileError(null), 5000);
      return () => clearTimeout(timer);
    }
  }, [fileError]);

  // Close autocomplete on outside click
  useEffect(() => {
    if (!isAutocompleteOpen) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (
        autocompleteRef.current &&
        !autocompleteRef.current.contains(e.target as Node)
      ) {
        closeAutocomplete();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isAutocompleteOpen]);

  // ============================================================================
  // File reading via Tauri backend
  // ============================================================================

  const readFileAndAttach = useCallback(
    async (filePath: string) => {
      if (!onAttach) return;
      setIsReadingFile(true);
      setFileError(null);

      try {
        const result = await invoke<CommandResponse<FileContentResult>>(
          'read_file_for_attachment',
          { path: filePath }
        );

        if (!result.success || !result.data) {
          const errMsg = result.error || t('simpleMode.attachment.unsupportedType', { defaultValue: 'Unsupported file type' });
          setFileError(errMsg);
          return;
        }

        const data = result.data;
        const fileName = getFileNameFromPath(filePath);
        const fileType = getFileTypeFromMime(data.mime_type);

        // Warn for large text files
        if (!data.is_binary && data.size > LARGE_FILE_WARNING_SIZE) {
          console.warn(`Large file attached: ${fileName} (${formatFileSize(data.size)})`);
        }

        const attachment: FileAttachmentData = {
          id: generateId(),
          name: fileName,
          path: filePath,
          size: data.size,
          type: fileType,
          content: data.is_binary ? undefined : data.content,
          preview: data.is_binary ? data.content : undefined, // base64 data URL for images
        };

        onAttach(attachment);
      } catch (err) {
        const errMsg =
          err instanceof Error ? err.message : 'Failed to read file';
        setFileError(errMsg);
      } finally {
        setIsReadingFile(false);
      }
    },
    [onAttach, t]
  );

  // ============================================================================
  // Drag & Drop handlers
  // ============================================================================

  const handleDragEnter = useCallback(
    (e: DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();
      if (!disabled && onAttach) {
        setIsDragging(true);
      }
    },
    [disabled, onAttach]
  );

  const handleDragLeave = useCallback(
    (e: DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();
      // Only unflag if leaving the drop zone entirely
      if (
        dropZoneRef.current &&
        !dropZoneRef.current.contains(e.relatedTarget as Node)
      ) {
        setIsDragging(false);
      }
    },
    []
  );

  const handleDragOver = useCallback(
    (e: DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();
    },
    []
  );

  const handleDrop = useCallback(
    async (e: DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);

      if (disabled || !onAttach) return;

      const files = e.dataTransfer?.files;
      if (!files || files.length === 0) return;

      for (const file of Array.from(files)) {
        // In Tauri, dropped files may have path available
        // For browser-based drag, use FileReader as fallback
        const filePath = (file as unknown as { path?: string }).path;
        if (filePath) {
          await readFileAndAttach(filePath);
        } else {
          // Browser fallback: read file content directly
          try {
            const content = await file.text();
            const attachment: FileAttachmentData = {
              id: generateId(),
              name: file.name,
              path: file.name,
              size: file.size,
              type: 'text',
              content,
            };
            onAttach(attachment);
          } catch {
            setFileError(`Failed to read ${file.name}`);
          }
        }
      }
    },
    [disabled, onAttach, readFileAndAttach]
  );

  // ============================================================================
  // File Picker
  // ============================================================================

  const handleFilePick = useCallback(async () => {
    if (disabled || !onAttach) return;

    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: true,
        title: t('attachment.pickFile', { defaultValue: 'Pick a file' }),
      });

      if (!selected) return;

      const paths = Array.isArray(selected) ? selected : [selected];
      for (const filePath of paths) {
        if (typeof filePath === 'string') {
          await readFileAndAttach(filePath);
        }
      }
    } catch (err) {
      console.error('Failed to open file picker:', err);
      // Fallback: use hidden file input
      const input = document.createElement('input');
      input.type = 'file';
      input.multiple = true;
      input.onchange = async () => {
        if (!input.files) return;
        for (const file of Array.from(input.files)) {
          try {
            const content = await file.text();
            const attachment: FileAttachmentData = {
              id: generateId(),
              name: file.name,
              path: file.name,
              size: file.size,
              type: 'text',
              content,
            };
            onAttach(attachment);
          } catch {
            setFileError(`Failed to read ${file.name}`);
          }
        }
      };
      input.click();
    }
  }, [disabled, onAttach, readFileAndAttach, t]);

  // ============================================================================
  // @ Autocomplete
  // ============================================================================

  const fetchWorkspaceFiles = useCallback(
    async (query: string) => {
      if (!workspacePath) {
        setAutocompleteFiles([]);
        return;
      }

      try {
        const result = await invoke<CommandResponse<WorkspaceFileResult[]>>(
          'list_workspace_files',
          {
            path: workspacePath,
            query: query || null,
            maxResults: 20,
          }
        );

        if (result.success && result.data) {
          setAutocompleteFiles(result.data);
        } else {
          setAutocompleteFiles([]);
        }
      } catch {
        setAutocompleteFiles([]);
      }
    },
    [workspacePath]
  );

  const closeAutocomplete = useCallback(() => {
    setIsAutocompleteOpen(false);
    setAutocompleteQuery('');
    setAutocompleteFiles([]);
    setAutocompleteIndex(0);
    setAtTriggerPos(-1);
  }, []);

  const handleAutocompleteSelect = useCallback(
    async (file: WorkspaceFileResult) => {
      if (!onAttach || !workspacePath) return;

      // Replace @query text in the textarea with the file reference
      const beforeAt = value.substring(0, atTriggerPos);
      const afterQuery = value.substring(
        atTriggerPos + 1 + autocompleteQuery.length
      );
      const newValue = `${beforeAt}@${file.name} ${afterQuery}`;
      onChange(newValue);

      closeAutocomplete();

      // Read the file and add as attachment
      const fullPath = file.path.startsWith('/')
        ? file.path
        : `${workspacePath}/${file.path}`;

      if (!file.is_dir) {
        await readFileAndAttach(fullPath);
      }
    },
    [
      onAttach,
      workspacePath,
      value,
      atTriggerPos,
      autocompleteQuery,
      onChange,
      closeAutocomplete,
      readFileAndAttach,
    ]
  );

  // Detect @ triggers when text changes
  const handleTextChange = useCallback(
    (newValue: string) => {
      onChange(newValue);

      if (!workspacePath || !onAttach) return;

      const textarea = textareaRef.current;
      if (!textarea) return;
      const cursorPos = textarea.selectionStart;

      // Find the last @ before cursor
      const beforeCursor = newValue.substring(0, cursorPos);
      const lastAtIndex = beforeCursor.lastIndexOf('@');

      if (lastAtIndex >= 0) {
        const afterAt = beforeCursor.substring(lastAtIndex + 1);
        // Only trigger if no spaces in the query (typing a filename)
        if (!afterAt.includes(' ') && !afterAt.includes('\n')) {
          setAtTriggerPos(lastAtIndex);
          setAutocompleteQuery(afterAt);
          setIsAutocompleteOpen(true);
          setAutocompleteIndex(0);
          fetchWorkspaceFiles(afterAt);
          return;
        }
      }

      if (isAutocompleteOpen) {
        closeAutocomplete();
      }
    },
    [
      onChange,
      workspacePath,
      onAttach,
      isAutocompleteOpen,
      fetchWorkspaceFiles,
      closeAutocomplete,
    ]
  );

  // Auto-resize textarea
  const handleInput = () => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(textarea.scrollHeight, 200)}px`;
    }
  };

  // ============================================================================
  // Keyboard handling
  // ============================================================================

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Handle autocomplete keyboard navigation
    if (isAutocompleteOpen && autocompleteFiles.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setAutocompleteIndex((prev) =>
          prev < autocompleteFiles.length - 1 ? prev + 1 : prev
        );
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setAutocompleteIndex((prev) => (prev > 0 ? prev - 1 : prev));
        return;
      }
      if (e.key === 'Enter' && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const selectedFile = autocompleteFiles[autocompleteIndex];
        if (selectedFile) {
          handleAutocompleteSelect(selectedFile);
        }
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        closeAutocomplete();
        return;
      }
    }

    // Submit on Cmd/Ctrl + Enter
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      if (!disabled && !isLoading && value.trim()) {
        onSubmit();
      }
    }
  };

  const canSubmit = !disabled && !isLoading && (value.trim() || attachments.length > 0);

  return (
    <div
      ref={dropZoneRef}
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      className={clsx(
        'relative rounded-xl transition-all',
        'bg-white dark:bg-gray-800',
        'border-2',
        isDragging
          ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
          : 'border-gray-200 dark:border-gray-700',
        'focus-within:border-primary-500 dark:focus-within:border-primary-500',
        'shadow-sm',
        disabled && 'opacity-60 cursor-not-allowed'
      )}
    >
      {/* Drag overlay */}
      {isDragging && (
        <div className="absolute inset-0 z-10 flex flex-col items-center justify-center rounded-xl bg-primary-100/90 dark:bg-primary-900/90 border-2 border-dashed border-primary-500">
          <UploadIcon className="w-8 h-8 text-primary-600 dark:text-primary-400 mb-2" />
          <p className="text-sm font-medium text-primary-700 dark:text-primary-300">
            {t('attachment.dropHere', { defaultValue: 'Drop files here' })}
          </p>
        </div>
      )}

      {/* File chips */}
      {attachments.length > 0 && (
        <div className="flex flex-wrap gap-1.5 px-4 pt-3 pb-1">
          {attachments.map((file) => (
            <FileChip
              key={file.id}
              file={file}
              onRemove={onRemoveAttachment ? () => onRemoveAttachment(file.id) : undefined}
            />
          ))}
        </div>
      )}

      {/* Error message */}
      {fileError && (
        <div className="px-4 py-1.5 text-xs text-red-600 dark:text-red-400">
          {fileError}
        </div>
      )}

      {/* Loading indicator for file reading */}
      {isReadingFile && (
        <div className="px-4 py-1.5 flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
          <UpdateIcon className="w-3 h-3 animate-spin" />
          <span>Reading file...</span>
        </div>
      )}

      {/* Input area */}
      <div className="relative flex items-end gap-2 p-4">
        <textarea
          ref={textareaRef}
          value={value}
          onChange={(e) => {
            handleTextChange(e.target.value);
            handleInput();
          }}
          onKeyDown={handleKeyDown}
          disabled={disabled}
          placeholder={defaultPlaceholder}
          rows={1}
          className={clsx(
            'flex-1 resize-none bg-transparent',
            'text-gray-900 dark:text-white',
            'placeholder-gray-400 dark:placeholder-gray-500',
            'focus:outline-none',
            'text-base leading-relaxed',
            disabled && 'cursor-not-allowed'
          )}
        />

        {/* @ Autocomplete dropdown */}
        {isAutocompleteOpen && autocompleteFiles.length > 0 && (
          <div
            ref={autocompleteRef}
            className={clsx(
              'absolute z-50 left-4 right-16 max-h-56 overflow-auto',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'rounded-lg shadow-lg',
              'bottom-full mb-2'
            )}
          >
            <div className="sticky top-0 px-3 py-1.5 bg-gray-50 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
              <span className="text-xs text-gray-500 dark:text-gray-400">
                {autocompleteQuery
                  ? `Files matching "${autocompleteQuery}"`
                  : 'Workspace files'}
              </span>
            </div>
            <div className="py-1">
              {autocompleteFiles.map((file, index) => (
                <button
                  key={file.path}
                  onClick={() => handleAutocompleteSelect(file)}
                  className={clsx(
                    'w-full flex items-center gap-2 px-3 py-1.5 text-left transition-colors',
                    index === autocompleteIndex
                      ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-900 dark:text-primary-100'
                      : 'hover:bg-gray-100 dark:hover:bg-gray-700'
                  )}
                >
                  {file.is_dir ? (
                    <FileIcon className="w-3.5 h-3.5 flex-shrink-0 text-amber-500" />
                  ) : (
                    <FileTextIcon className="w-3.5 h-3.5 flex-shrink-0 text-gray-400" />
                  )}
                  <div className="flex-1 min-w-0">
                    <div className="text-sm font-medium truncate">
                      {file.name}
                    </div>
                    <div className="text-xs text-gray-500 dark:text-gray-400 truncate">
                      {file.path}
                      {!file.is_dir && file.size > 0 && (
                        <span className="ml-1">
                          ({formatFileSize(file.size)})
                        </span>
                      )}
                    </div>
                  </div>
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Attach button */}
        {onAttach && (
          <button
            onClick={handleFilePick}
            disabled={disabled || isReadingFile}
            className={clsx(
              'flex items-center justify-center',
              'w-10 h-10 rounded-lg',
              'text-gray-500 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-700',
              'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
              'dark:focus:ring-offset-gray-800',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors'
            )}
            title={t('attachment.pickFile', { defaultValue: 'Pick a file' })}
          >
            <FilePlusIcon className="w-5 h-5" />
          </button>
        )}

        {/* Submit button */}
        <button
          onClick={onSubmit}
          disabled={!canSubmit}
          className={clsx(
            'flex items-center justify-center',
            'w-10 h-10 rounded-lg',
            'bg-primary-600 text-white',
            'hover:bg-primary-700',
            'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
            'dark:focus:ring-offset-gray-900',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'transition-colors'
          )}
          title={t('input.submitTitle')}
        >
          {isLoading ? (
            <UpdateIcon className="w-5 h-5 animate-spin" />
          ) : (
            <PaperPlaneIcon className="w-5 h-5" />
          )}
        </button>
      </div>
    </div>
  );
}

// ============================================================================
// FileChip Component
// ============================================================================

function FileChip({
  file,
  onRemove,
}: {
  file: FileAttachmentData;
  onRemove?: () => void;
}) {
  const chipStyles = {
    text: 'bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 border-blue-200 dark:border-blue-800',
    image:
      'bg-green-100 dark:bg-green-900/40 text-green-700 dark:text-green-300 border-green-200 dark:border-green-800',
    pdf: 'bg-orange-100 dark:bg-orange-900/40 text-orange-700 dark:text-orange-300 border-orange-200 dark:border-orange-800',
    unknown:
      'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 border-gray-200 dark:border-gray-700',
  };

  const TypeIcon =
    file.type === 'image'
      ? ImageIcon
      : file.type === 'text'
        ? FileTextIcon
        : FileIcon;

  return (
    <div
      className={clsx(
        'inline-flex items-center gap-1.5 px-2 py-1 rounded-md',
        'text-xs font-medium border',
        'transition-colors',
        chipStyles[file.type]
      )}
      title={file.path}
    >
      <TypeIcon className="w-3.5 h-3.5 flex-shrink-0" />
      <span className="truncate max-w-[140px]">{file.name}</span>
      <span className="opacity-60">({formatFileSize(file.size)})</span>
      {onRemove && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
          className="ml-0.5 p-0.5 rounded hover:bg-black/10 dark:hover:bg-white/10 transition-colors"
          title="Remove"
        >
          <Cross2Icon className="w-3 h-3" />
        </button>
      )}
    </div>
  );
}

export default InputBox;
