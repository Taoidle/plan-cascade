/**
 * DocumentUploader Component Tests
 *
 * Tests rendering, file validation, upload flow, and progress display.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { DocumentUploader } from '../DocumentUploader';
import { useKnowledgeStore } from '../../../store/knowledge';
import { useSettingsStore } from '../../../store/settings';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      const translations: Record<string, string> = {
        'upload.title': 'Upload Documents',
        'upload.subtitle': 'Drag and drop or click to upload',
        'upload.dropHere': 'Drop files here',
        'upload.dragOrClick': 'Drag or click to upload',
        'upload.acceptedFormats': 'PDF, DOCX, XLSX, MD, TXT',
        'upload.noValidFiles': 'No valid files',
        'upload.selectedFiles': `${opts?.count ?? 0} files selected`,
        'upload.ingesting': 'Ingesting...',
        'upload.uploadAndIndex': `Upload ${opts?.count ?? 0} files`,
      };
      return translations[key] || key;
    },
  }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockImplementation(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function resetStore() {
  useKnowledgeStore.setState({
    collections: [],
    activeCollection: null,
    documents: [],
    documentsByCollection: {},
    queryResults: [],
    queryStateByCollection: {},
    totalSearched: 0,
    searchQuery: '',
    queryRuns: [],
    queryRunsByCollection: {},
    docsStatus: null,
    isLoading: false,
    isLoadingCollections: false,
    isLoadingDocuments: false,
    isUpdatingCollection: false,
    isIngesting: false,
    isQuerying: false,
    isDeleting: false,
    isDeletingCollection: false,
    isDeletingDocument: false,
    isLoadingQueryRuns: false,
    isLoadingDocsStatus: false,
    isSyncingDocs: false,
    uploadProgress: 0,
    uploadProgressByJob: {},
    activeUploadJobByCollection: {},
    pendingUpdates: null,
    isCheckingUpdates: false,
    isApplyingUpdates: false,
    error: null,
  });
}

function createMockFile(name: string, content: string, type: string): File {
  return new File([content], name, { type });
}

function renderUploader() {
  return render(<DocumentUploader projectId="proj-1" collectionId="col-1" />);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('DocumentUploader', () => {
  const mockListen = vi.mocked(listen);
  const mockInvoke = vi.mocked(invoke);

  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue({ success: true, data: true, error: null });
    mockListen.mockImplementation(() => Promise.resolve(() => {}));
    resetStore();
    useSettingsStore.setState({ kbIngestJobScopedProgress: true });
  });

  // ========================================================================
  // Rendering
  // ========================================================================

  describe('rendering', () => {
    it('renders title and subtitle', () => {
      renderUploader();
      expect(screen.getByText('Upload Documents')).toBeDefined();
      expect(screen.getByText('Drag and drop or click to upload')).toBeDefined();
    });

    it('renders drop zone with instructions', () => {
      renderUploader();
      expect(screen.getByText('Drag or click to upload')).toBeDefined();
      expect(screen.getByText('PDF, DOCX, XLSX, MD, TXT')).toBeDefined();
    });

    it('does not show upload button when no files selected', () => {
      renderUploader();
      expect(screen.queryByText(/Upload \d+ files/)).toBeNull();
    });
  });

  // ========================================================================
  // File selection
  // ========================================================================

  describe('file selection', () => {
    it('shows selected files after file input change', async () => {
      renderUploader();
      const input = document.querySelector('input[type="file"]') as HTMLInputElement;

      const file = createMockFile('test.txt', 'hello', 'text/plain');
      Object.defineProperty(input, 'files', { value: [file], configurable: true });
      fireEvent.change(input);

      await waitFor(() => {
        expect(screen.getByText('test.txt')).toBeDefined();
      });
    });

    it('shows upload button after file selection', async () => {
      renderUploader();
      const input = document.querySelector('input[type="file"]') as HTMLInputElement;

      const file = createMockFile('doc.md', '# Hello', 'text/markdown');
      Object.defineProperty(input, 'files', { value: [file], configurable: true });
      fireEvent.change(input);

      await waitFor(() => {
        expect(screen.getByText('Upload 1 files')).toBeDefined();
      });
    });

    it('allows removing a selected file', async () => {
      renderUploader();
      const input = document.querySelector('input[type="file"]') as HTMLInputElement;

      const file = createMockFile('test.txt', 'hello', 'text/plain');
      Object.defineProperty(input, 'files', { value: [file], configurable: true });
      fireEvent.change(input);

      await waitFor(() => {
        expect(screen.getByText('test.txt')).toBeDefined();
      });

      // Click the remove button (the X SVG button)
      const buttons = document.querySelectorAll('button');
      // The last button in the file row is the remove button
      const removeBtn = Array.from(buttons).find(
        (btn) => btn.querySelector('svg') && btn.closest('.flex.items-center.justify-between'),
      );
      if (removeBtn) {
        fireEvent.click(removeBtn);
      }

      await waitFor(() => {
        expect(screen.queryByText('test.txt')).toBeNull();
      });
    });
  });

  // ========================================================================
  // Progress display
  // ========================================================================

  describe('progress display', () => {
    it('shows progress bar for current collection active upload job', () => {
      useKnowledgeStore.setState({
        activeUploadJobByCollection: { 'col-1': 'job-1' },
        uploadProgressByJob: { 'job-1': 45 },
      });
      renderUploader();

      expect(screen.getByText('Ingesting...')).toBeDefined();
      expect(screen.getByText('45%')).toBeDefined();
    });

    it('hides upload button when current collection has active job', async () => {
      renderUploader();
      const input = document.querySelector('input[type="file"]') as HTMLInputElement;

      const file = createMockFile('test.txt', 'hello', 'text/plain');
      Object.defineProperty(input, 'files', { value: [file], configurable: true });
      fireEvent.change(input);

      act(() => {
        useKnowledgeStore.setState({
          activeUploadJobByCollection: { 'col-1': 'job-1' },
          uploadProgressByJob: { 'job-1': 50 },
        });
      });

      expect(screen.queryByText(/Upload \d+ files/)).toBeNull();
    });

    it('ignores progress events from other collections and applies matching events', async () => {
      let listener: ((event: { payload: Record<string, unknown> }) => void) | null = null;
      mockListen.mockImplementation((_eventName, cb) => {
        listener = cb as (event: { payload: Record<string, unknown> }) => void;
        return Promise.resolve(() => {});
      });

      renderUploader();
      await waitFor(() => expect(listener).not.toBeNull());

      act(() => {
        listener!({
          payload: {
            job_id: 'job-other',
            project_id: 'proj-1',
            collection_id: 'col-other',
            stage: 'embedding',
            progress: 33,
            detail: 'other',
          },
        });
      });
      expect(screen.queryByText('33%')).toBeNull();
      expect(mockInvoke).toHaveBeenCalledWith('rag_record_ingest_crosstalk_alert');

      act(() => {
        listener!({
          payload: {
            job_id: 'job-1',
            project_id: 'proj-1',
            collection_id: 'col-1',
            stage: 'embedding',
            progress: 66,
            detail: 'match',
          },
        });
      });
      expect(screen.getByText('66%')).toBeDefined();
    });

    it('supports legacy progress payloads without job_id when job scope flag is disabled', async () => {
      let listener: ((event: { payload: Record<string, unknown> }) => void) | null = null;
      useSettingsStore.setState({ kbIngestJobScopedProgress: false });
      mockListen.mockImplementation((_eventName, cb) => {
        listener = cb as (event: { payload: Record<string, unknown> }) => void;
        return Promise.resolve(() => {});
      });

      renderUploader();
      await waitFor(() => expect(listener).not.toBeNull());

      act(() => {
        listener!({
          payload: {
            project_id: 'proj-1',
            collection_id: 'col-1',
            stage: 'embedding',
            progress: 44,
            detail: 'legacy',
          },
        });
      });

      expect(screen.getByText('44%')).toBeDefined();
    });
  });

  // ========================================================================
  // Drop zone interaction
  // ========================================================================

  describe('drop zone', () => {
    it('shows drop text on drag over', () => {
      renderUploader();
      const dropZone = screen.getByText('Drag or click to upload').closest('div')!;

      fireEvent.dragOver(dropZone);

      expect(screen.getByText('Drop files here')).toBeDefined();
    });

    it('reverts text on drag leave', () => {
      renderUploader();
      const dropZone = screen.getByText('Drag or click to upload').closest('div')!;

      fireEvent.dragOver(dropZone);
      expect(screen.getByText('Drop files here')).toBeDefined();

      fireEvent.dragLeave(dropZone);
      expect(screen.getByText('Drag or click to upload')).toBeDefined();
    });
  });
});
