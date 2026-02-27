/**
 * CodebaseFileList Component
 *
 * Paginated file table with language filter dropdown and text search.
 * Shows file path, language, size, line count, component, and test status.
 */

import { useState, useCallback, useEffect } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useCodebaseStore } from '../../store/codebase';

const PAGE_SIZE = 50;

interface CodebaseFileListProps {
  projectPath: string;
}

export function CodebaseFileList({ projectPath: _projectPath }: CodebaseFileListProps) {
  const { t } = useTranslation('codebase');
  const {
    files,
    filesTotalCount,
    filesLoading,
    filesPage,
    filesLanguageFilter,
    filesSearchPattern,
    projectDetail,
    setFilesPage,
    setFilesLanguageFilter,
    setFilesSearchPattern,
  } = useCodebaseStore();

  const [searchInput, setSearchInput] = useState(filesSearchPattern);

  const languages = projectDetail?.languages ?? [];
  const totalPages = Math.max(1, Math.ceil(filesTotalCount / PAGE_SIZE));

  // Debounce search pattern
  useEffect(() => {
    const timer = setTimeout(() => {
      if (searchInput !== filesSearchPattern) {
        setFilesSearchPattern(searchInput);
      }
    }, 300);
    return () => clearTimeout(timer);
  }, [searchInput, filesSearchPattern, setFilesSearchPattern]);

  const handleLanguageChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      const val = e.target.value;
      setFilesLanguageFilter(val === '' ? null : val);
    },
    [setFilesLanguageFilter],
  );

  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div className="flex flex-col h-full">
      {/* Filters */}
      <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 flex items-center gap-3">
        {/* Language filter */}
        <select
          value={filesLanguageFilter ?? ''}
          onChange={handleLanguageChange}
          className={clsx(
            'px-3 py-1.5 rounded-lg text-sm',
            'border border-gray-300 dark:border-gray-600',
            'bg-white dark:bg-gray-800',
            'text-gray-900 dark:text-white',
          )}
        >
          <option value="">{t('allLanguages')}</option>
          {languages.map((lang) => (
            <option key={lang.language} value={lang.language}>
              {lang.language} ({lang.count})
            </option>
          ))}
        </select>

        {/* Text search */}
        <input
          type="text"
          value={searchInput}
          onChange={(e) => setSearchInput(e.target.value)}
          placeholder={t('searchFiles')}
          className={clsx(
            'flex-1 min-w-0 px-3 py-1.5 rounded-lg text-sm',
            'border border-gray-300 dark:border-gray-600',
            'bg-white dark:bg-gray-800',
            'text-gray-900 dark:text-white',
            'placeholder-gray-400 dark:placeholder-gray-500',
          )}
        />

        <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap">
          {t('fileCount', { count: filesTotalCount })}
        </span>
      </div>

      {/* File table */}
      <div className="flex-1 overflow-auto">
        {filesLoading ? (
          <div className="p-8 text-center">
            <div className="animate-pulse text-sm text-gray-500">Loading...</div>
          </div>
        ) : files.length === 0 ? (
          <div className="p-8 text-center text-sm text-gray-500 dark:text-gray-400">{t('noResults')}</div>
        ) : (
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-gray-50 dark:bg-gray-900">
              <tr className="text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">
                <th className="px-4 py-2">{t('files')}</th>
                <th className="px-3 py-2">{t('languages')}</th>
                <th className="px-3 py-2 text-right">Size</th>
                <th className="px-3 py-2 text-right">Lines</th>
                <th className="px-3 py-2">{t('components')}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-200 dark:divide-gray-800">
              {files.map((file) => (
                <tr key={file.id} className="hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors">
                  <td className="px-4 py-2">
                    <div className="flex items-center gap-2">
                      <span
                        className="text-gray-900 dark:text-white font-mono text-xs truncate max-w-xs"
                        title={file.file_path}
                      >
                        {file.file_path}
                      </span>
                      {file.is_test && (
                        <span
                          className={clsx(
                            'px-1.5 py-0.5 rounded text-[10px] font-medium',
                            'bg-yellow-100 dark:bg-yellow-900/30',
                            'text-yellow-700 dark:text-yellow-300',
                          )}
                        >
                          {t('testFile')}
                        </span>
                      )}
                    </div>
                  </td>
                  <td className="px-3 py-2 text-gray-600 dark:text-gray-400">{file.language || '-'}</td>
                  <td className="px-3 py-2 text-right text-gray-600 dark:text-gray-400 tabular-nums">
                    {formatSize(file.size_bytes)}
                  </td>
                  <td className="px-3 py-2 text-right text-gray-600 dark:text-gray-400 tabular-nums">
                    {file.line_count.toLocaleString()}
                  </td>
                  <td className="px-3 py-2 text-gray-600 dark:text-gray-400 truncate max-w-[120px]">
                    {file.component || '-'}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Pagination */}
      {totalPages > 1 && (
        <div className="px-4 py-2 border-t border-gray-200 dark:border-gray-700 flex items-center justify-between">
          <button
            onClick={() => setFilesPage(Math.max(0, filesPage - 1))}
            disabled={filesPage === 0}
            className={clsx(
              'px-3 py-1 rounded text-sm',
              'text-gray-600 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors',
            )}
          >
            {t('previous')}
          </button>
          <span className="text-xs text-gray-500 dark:text-gray-400">
            {t('page')} {filesPage + 1} {t('of')} {totalPages}
          </span>
          <button
            onClick={() => setFilesPage(Math.min(totalPages - 1, filesPage + 1))}
            disabled={filesPage >= totalPages - 1}
            className={clsx(
              'px-3 py-1 rounded text-sm',
              'text-gray-600 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors',
            )}
          >
            {t('next')}
          </button>
        </div>
      )}
    </div>
  );
}
