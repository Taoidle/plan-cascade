/**
 * DocumentUploader Component Tests
 *
 * Tests rendering, file validation, upload flow, and progress display.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { DocumentUploader } from '../DocumentUploader';
import { useKnowledgeStore } from '../../../store/knowledge';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

vi.mock('react-i18next', () => ({
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
    queryResults: [],
    totalSearched: 0,
    searchQuery: '',
    isLoading: false,
    isIngesting: false,
    isQuerying: false,
    isDeleting: false,
    uploadProgress: 0,
    error: null,
  });
}

function createMockFile(name: string, content: string, type: string): File {
  return new File([content], name, { type });
}

function renderUploader() {
  return render(<DocumentUploader projectId="proj-1" collectionName="test-col" />);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('DocumentUploader', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
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
    it('shows progress bar when ingesting', () => {
      useKnowledgeStore.setState({ isIngesting: true, uploadProgress: 45 });
      renderUploader();

      expect(screen.getByText('Ingesting...')).toBeDefined();
      expect(screen.getByText('45%')).toBeDefined();
    });

    it('hides upload button when ingesting', async () => {
      renderUploader();
      const input = document.querySelector('input[type="file"]') as HTMLInputElement;

      const file = createMockFile('test.txt', 'hello', 'text/plain');
      Object.defineProperty(input, 'files', { value: [file], configurable: true });
      fireEvent.change(input);

      // Simulate ingesting state
      useKnowledgeStore.setState({ isIngesting: true, uploadProgress: 50 });

      // Re-render to reflect state change
      renderUploader();

      expect(screen.queryByText(/Upload \d+ files/)).toBeNull();
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
