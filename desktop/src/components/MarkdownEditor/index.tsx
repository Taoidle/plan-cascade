/**
 * MarkdownEditor Component
 *
 * Main container for the CLAUDE.md editor.
 * Integrates FileTree, Editor, Preview, and Templates.
 */

import { useEffect, useCallback, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  ColumnsIcon,
  Pencil1Icon,
  EyeOpenIcon,
  CheckCircledIcon,
  CrossCircledIcon,
  ReloadIcon,
} from '@radix-ui/react-icons';
import { useMarkdownStore } from '../../store/markdown';
import { useAutoSave } from '../../hooks/useAutoSave';
import type { ViewMode, SaveStatus } from '../../types/markdown';
import { FileTree } from './FileTree';
import { Editor } from './Editor';
import { Preview } from './Preview';
import { TemplateSelector } from './TemplateSelector';

interface MarkdownEditorProps {
  /** Root path to scan for CLAUDE.md files */
  rootPath: string;
  /** Whether dark mode is enabled */
  isDark?: boolean;
}

/** View mode toggle button */
interface ViewModeButtonProps {
  mode: ViewMode;
  currentMode: ViewMode;
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}

function ViewModeButton({ mode, currentMode, icon, label, onClick }: ViewModeButtonProps) {
  return (
    <button
      onClick={onClick}
      title={label}
      className={clsx(
        'p-1.5 rounded',
        'transition-colors',
        mode === currentMode
          ? 'bg-primary-100 dark:bg-primary-900/40 text-primary-600 dark:text-primary-400'
          : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700',
      )}
    >
      {icon}
    </button>
  );
}

/** Save status indicator */
function SaveStatusIndicator({ status }: { status: SaveStatus }) {
  const { t } = useTranslation();

  const statusConfig: Record<SaveStatus, { icon: React.ReactNode; color: string }> = {
    saved: {
      icon: <CheckCircledIcon className="w-4 h-4" />,
      color: 'text-green-600 dark:text-green-400',
    },
    saving: {
      icon: <ReloadIcon className="w-4 h-4 animate-spin" />,
      color: 'text-blue-600 dark:text-blue-400',
    },
    unsaved: {
      icon: <span className="w-2 h-2 rounded-full bg-yellow-500" />,
      color: 'text-yellow-600 dark:text-yellow-400',
    },
    error: {
      icon: <CrossCircledIcon className="w-4 h-4" />,
      color: 'text-red-600 dark:text-red-400',
    },
  };

  const config = statusConfig[status];

  return (
    <div className={clsx('flex items-center gap-1.5 text-xs', config.color)}>
      {config.icon}
      <span>{t(`markdownEditor.saveStatus.${status}`)}</span>
    </div>
  );
}

export function MarkdownEditor({ rootPath, isDark = false }: MarkdownEditorProps) {
  const { t } = useTranslation();
  const [isInitialized, setIsInitialized] = useState(false);

  const {
    files,
    selectedFile,
    content,
    viewMode,
    saveStatus,
    loading,
    error,
    autoSaveEnabled,
    fetchFiles,
    selectFile,
    setContent,
    saveContent,
    setViewMode,
    setSaveStatus,
    clearError,
  } = useMarkdownStore();

  // Initialize file list on mount
  useEffect(() => {
    if (rootPath && !isInitialized) {
      fetchFiles(rootPath);
      setIsInitialized(true);
    }
  }, [rootPath, isInitialized, fetchFiles]);

  // Auto-save hook
  useAutoSave({
    content,
    onSave: async () => {
      const result = await saveContent();
      if (result && !result.success) {
        setSaveStatus('error');
      }
    },
    delay: 2000,
    enabled: autoSaveEnabled && !!selectedFile,
    minLength: 1,
  });

  // Handle manual save
  const handleSave = useCallback(async () => {
    await saveContent();
  }, [saveContent]);

  // Handle template insertion
  const handleTemplateInsert = useCallback(
    (templateContent: string) => {
      // If there's existing content, append template
      if (content.trim()) {
        setContent(content + '\n\n' + templateContent);
      } else {
        setContent(templateContent);
      }
    },
    [content, setContent],
  );

  // Handle refresh
  const handleRefresh = useCallback(() => {
    fetchFiles(rootPath);
  }, [fetchFiles, rootPath]);

  return (
    <div className="h-full flex flex-col bg-white dark:bg-gray-900">
      {/* Top Bar */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800">
        <div className="flex items-center gap-3">
          <h2 className="text-sm font-semibold text-gray-900 dark:text-white">{t('markdownEditor.title')}</h2>

          {/* Template Selector */}
          <TemplateSelector onSelect={handleTemplateInsert} disabled={!selectedFile} />
        </div>

        <div className="flex items-center gap-4">
          {/* Save Status */}
          {selectedFile && <SaveStatusIndicator status={saveStatus} />}

          {/* View Mode Toggle */}
          <div className="flex items-center gap-0.5 bg-gray-100 dark:bg-gray-700 rounded-md p-0.5">
            <ViewModeButton
              mode="split"
              currentMode={viewMode}
              icon={<ColumnsIcon className="w-4 h-4" />}
              label={t('markdownEditor.viewMode.split')}
              onClick={() => setViewMode('split')}
            />
            <ViewModeButton
              mode="edit"
              currentMode={viewMode}
              icon={<Pencil1Icon className="w-4 h-4" />}
              label={t('markdownEditor.viewMode.edit')}
              onClick={() => setViewMode('edit')}
            />
            <ViewModeButton
              mode="preview"
              currentMode={viewMode}
              icon={<EyeOpenIcon className="w-4 h-4" />}
              label={t('markdownEditor.viewMode.preview')}
              onClick={() => setViewMode('preview')}
            />
          </div>
        </div>
      </div>

      {/* Error Banner */}
      {error && (
        <div className="px-4 py-2 bg-red-50 dark:bg-red-900/20 border-b border-red-200 dark:border-red-800 flex items-center justify-between">
          <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
          <button onClick={clearError} className="text-sm text-red-600 dark:text-red-400 hover:underline">
            {t('common.cancel')}
          </button>
        </div>
      )}

      {/* Main Content */}
      <div className="flex-1 flex min-h-0">
        {/* Sidebar - File Tree */}
        <div className="w-64 flex-shrink-0 border-r border-gray-200 dark:border-gray-700">
          <FileTree
            files={files}
            selectedFile={selectedFile}
            loading={loading.files}
            onSelectFile={selectFile}
            onRefresh={handleRefresh}
          />
        </div>

        {/* Editor/Preview Area */}
        <div className="flex-1 flex min-h-0 min-w-0">
          {selectedFile ? (
            <>
              {/* Editor */}
              {(viewMode === 'split' || viewMode === 'edit') && (
                <div
                  className={clsx(
                    'flex-1 min-w-0',
                    viewMode === 'split' && 'border-r border-gray-200 dark:border-gray-700',
                  )}
                >
                  {loading.content ? (
                    <div className="h-full flex items-center justify-center">
                      <ReloadIcon className="w-6 h-6 animate-spin text-gray-400" />
                    </div>
                  ) : (
                    <Editor
                      content={content}
                      onChange={setContent}
                      onSave={handleSave}
                      isDark={isDark}
                      fileName={selectedFile.name}
                    />
                  )}
                </div>
              )}

              {/* Preview */}
              {(viewMode === 'split' || viewMode === 'preview') && (
                <div className="flex-1 min-w-0">
                  <Preview content={content} />
                </div>
              )}
            </>
          ) : (
            /* No file selected placeholder */
            <div className="flex-1 flex items-center justify-center">
              <div className="text-center">
                <Pencil1Icon className="w-12 h-12 mx-auto mb-3 text-gray-300 dark:text-gray-600" />
                <p className="text-gray-500 dark:text-gray-400">{t('markdownEditor.editor.placeholder')}</p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default MarkdownEditor;
