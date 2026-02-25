/**
 * DocumentUploader Component
 *
 * Drag-and-drop zone for uploading documents to a knowledge collection.
 * Supports PDF, DOCX, XLSX, Markdown, and plain text files.
 */

import { useState, useCallback, useRef } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useKnowledgeStore } from '../../store/knowledge';
import type { DocumentInput } from '../../lib/knowledgeApi';

// ---------------------------------------------------------------------------
// Accepted file types
// ---------------------------------------------------------------------------

const ACCEPTED_EXTENSIONS = ['.pdf', '.docx', '.xlsx', '.md', '.txt', '.markdown'];
const ACCEPTED_MIME_TYPES = [
  'application/pdf',
  'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  'text/markdown',
  'text/plain',
];

interface DocumentUploaderProps {
  projectId: string;
  collectionName: string;
}

export function DocumentUploader({ projectId, collectionName }: DocumentUploaderProps) {
  const { t } = useTranslation('knowledge');
  const { ingestDocuments, isIngesting, uploadProgress } = useKnowledgeStore();

  const [isDragOver, setIsDragOver] = useState(false);
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
  const [uploadError, setUploadError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const isAcceptedFile = (file: File): boolean => {
    const ext = '.' + file.name.split('.').pop()?.toLowerCase();
    return ACCEPTED_EXTENSIONS.includes(ext) || ACCEPTED_MIME_TYPES.includes(file.type);
  };

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragOver(false);
      setUploadError(null);

      const files = Array.from(e.dataTransfer.files).filter(isAcceptedFile);
      if (files.length === 0) {
        setUploadError(t('upload.noValidFiles'));
        return;
      }
      setSelectedFiles((prev) => [...prev, ...files]);
    },
    [t],
  );

  const handleFileSelect = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    if (!e.target.files) return;
    setUploadError(null);
    const files = Array.from(e.target.files).filter(isAcceptedFile);
    setSelectedFiles((prev) => [...prev, ...files]);
    // Reset input so the same file can be selected again
    e.target.value = '';
  }, []);

  const removeFile = useCallback((index: number) => {
    setSelectedFiles((prev) => prev.filter((_, i) => i !== index));
  }, []);

  const handleUpload = useCallback(async () => {
    if (selectedFiles.length === 0) return;
    setUploadError(null);

    try {
      // Read files and convert to DocumentInput
      const documents: DocumentInput[] = await Promise.all(
        selectedFiles.map(async (file): Promise<DocumentInput> => {
          const text = await file.text();
          const ext = file.name.split('.').pop()?.toLowerCase() ?? 'txt';
          return {
            id: `${file.name}-${Date.now()}`,
            content: text,
            source_path: file.name,
            source_type: ext,
          };
        }),
      );

      const ok = await ingestDocuments(projectId, collectionName, documents);
      if (ok) {
        setSelectedFiles([]);
      }
    } catch (err) {
      setUploadError(err instanceof Error ? err.message : String(err));
    }
  }, [selectedFiles, projectId, collectionName, ingestDocuments]);

  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div className="p-6 space-y-6">
      <div>
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white">{t('upload.title')}</h3>
        <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">{t('upload.subtitle')}</p>
      </div>

      {/* Drop zone */}
      <div
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        onClick={() => fileInputRef.current?.click()}
        className={clsx(
          'border-2 border-dashed rounded-xl p-8 text-center cursor-pointer',
          'transition-colors duration-200',
          isDragOver
            ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
            : 'border-gray-300 dark:border-gray-600 hover:border-gray-400 dark:hover:border-gray-500',
          'bg-gray-50 dark:bg-gray-900/50',
        )}
      >
        <input
          ref={fileInputRef}
          type="file"
          multiple
          accept={ACCEPTED_EXTENSIONS.join(',')}
          onChange={handleFileSelect}
          className="hidden"
        />
        <svg
          className="mx-auto w-10 h-10 text-gray-400 dark:text-gray-500 mb-3"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.5}
            d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"
          />
        </svg>
        <p className="text-sm font-medium text-gray-700 dark:text-gray-300">
          {isDragOver ? t('upload.dropHere') : t('upload.dragOrClick')}
        </p>
        <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">{t('upload.acceptedFormats')}</p>
      </div>

      {/* Error */}
      {uploadError && (
        <div className="text-sm text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-900/20 p-3 rounded-lg">
          {uploadError}
        </div>
      )}

      {/* File list */}
      {selectedFiles.length > 0 && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300">
            {t('upload.selectedFiles', { count: selectedFiles.length })}
          </h4>
          <div className="divide-y divide-gray-200 dark:divide-gray-700 rounded-lg border border-gray-200 dark:border-gray-700">
            {selectedFiles.map((file, index) => (
              <div key={`${file.name}-${index}`} className="flex items-center justify-between px-4 py-2">
                <div className="flex items-center gap-3 min-w-0">
                  <svg className="w-5 h-5 text-gray-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"
                    />
                  </svg>
                  <div className="min-w-0">
                    <p className="text-sm text-gray-900 dark:text-white truncate">{file.name}</p>
                    <p className="text-xs text-gray-500">{formatSize(file.size)}</p>
                  </div>
                </div>
                <button onClick={() => removeFile(index)} className="text-gray-400 hover:text-red-500 p-1">
                  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Upload progress */}
      {isIngesting && (
        <div className="space-y-2">
          <div className="flex justify-between text-sm">
            <span className="text-gray-600 dark:text-gray-400">{t('upload.ingesting')}</span>
            <span className="text-gray-900 dark:text-white font-medium">{uploadProgress}%</span>
          </div>
          <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
            <div
              className="bg-primary-600 h-2 rounded-full transition-all duration-300"
              style={{ width: `${uploadProgress}%` }}
            />
          </div>
        </div>
      )}

      {/* Upload button */}
      {selectedFiles.length > 0 && !isIngesting && (
        <button
          onClick={handleUpload}
          className={clsx(
            'w-full px-4 py-2.5 rounded-lg text-sm font-medium',
            'bg-primary-600 hover:bg-primary-700',
            'text-white',
            'transition-colors',
          )}
        >
          {t('upload.uploadAndIndex', { count: selectedFiles.length })}
        </button>
      )}
    </div>
  );
}
