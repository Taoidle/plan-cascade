/**
 * FileAttachment Component Tests
 * Story 011-3: File Attachment and @ File References
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { renderHook, act } from '@testing-library/react';
import {
  FileChip,
  FileReferenceAutocomplete,
  useFileReferences,
} from '../FileAttachment';

describe('FileChip', () => {
  it('renders file name', () => {
    const file = { id: '1', name: 'test.ts', path: '/path/test.ts', size: 1024 };
    render(<FileChip file={file} />);
    expect(screen.getByText('test.ts')).toBeInTheDocument();
  });

  it('shows file size', () => {
    const file = { id: '1', name: 'test.ts', path: '/path/test.ts', size: 1024 };
    render(<FileChip file={file} />);
    expect(screen.getByText('(1.0 KB)')).toBeInTheDocument();
  });

  it('shows remove button when onRemove is provided', () => {
    const onRemove = vi.fn();
    const file = { id: '1', name: 'test.ts', path: '/path/test.ts', size: 100 };
    render(<FileChip file={file} onRemove={onRemove} />);

    const removeButton = screen.getByRole('button');
    fireEvent.click(removeButton);
    expect(onRemove).toHaveBeenCalled();
  });

  it('applies reference styling when isReference is true', () => {
    const file = { id: '1', name: 'ref.ts', path: '/ref.ts' };
    const { container } = render(<FileChip file={file} isReference={true} />);
    expect(container.querySelector('.bg-blue-100')).toBeInTheDocument();
  });
});

describe('FileReferenceAutocomplete', () => {
  const files = [
    { id: '1', path: '/src/App.tsx', name: 'App.tsx' },
    { id: '2', path: '/src/main.tsx', name: 'main.tsx' },
    { id: '3', path: '/src/index.css', name: 'index.css' },
  ];

  it('does not render when closed', () => {
    render(
      <FileReferenceAutocomplete
        isOpen={false}
        searchQuery=""
        files={files}
        onSelect={vi.fn()}
        onClose={vi.fn()}
        position={{ top: 0, left: 0 }}
      />
    );
    expect(screen.queryByText('App.tsx')).not.toBeInTheDocument();
  });

  it('renders file list when open', () => {
    render(
      <FileReferenceAutocomplete
        isOpen={true}
        searchQuery=""
        files={files}
        onSelect={vi.fn()}
        onClose={vi.fn()}
        position={{ top: 0, left: 0 }}
      />
    );
    expect(screen.getByText('App.tsx')).toBeInTheDocument();
  });

  it('filters files based on search query', () => {
    render(
      <FileReferenceAutocomplete
        isOpen={true}
        searchQuery="App"
        files={files}
        onSelect={vi.fn()}
        onClose={vi.fn()}
        position={{ top: 0, left: 0 }}
      />
    );
    expect(screen.getByText('App.tsx')).toBeInTheDocument();
  });

  it('calls onSelect when file is clicked', () => {
    const onSelect = vi.fn();
    render(
      <FileReferenceAutocomplete
        isOpen={true}
        searchQuery=""
        files={files}
        onSelect={onSelect}
        onClose={vi.fn()}
        position={{ top: 0, left: 0 }}
      />
    );

    fireEvent.click(screen.getByText('App.tsx'));
    expect(onSelect).toHaveBeenCalledWith(files[0]);
  });

  it('closes on Escape key', () => {
    const onClose = vi.fn();
    render(
      <FileReferenceAutocomplete
        isOpen={true}
        searchQuery=""
        files={files}
        onSelect={vi.fn()}
        onClose={onClose}
        position={{ top: 0, left: 0 }}
      />
    );

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });
});

describe('useFileReferences', () => {
  it('initializes with empty references', () => {
    const { result } = renderHook(() => useFileReferences([]));
    expect(result.current.references).toEqual([]);
    expect(result.current.isAutocompleteOpen).toBe(false);
  });

  it('opens autocomplete when @ is detected', () => {
    const { result } = renderHook(() => useFileReferences([]));

    act(() => {
      result.current.handleInputChange('@test', 5);
    });

    expect(result.current.isAutocompleteOpen).toBe(true);
    expect(result.current.autocompleteQuery).toBe('test');
  });

  it('closes autocomplete when space is typed after @', () => {
    const { result } = renderHook(() => useFileReferences([]));

    act(() => {
      result.current.handleInputChange('@test ', 6);
    });

    expect(result.current.isAutocompleteOpen).toBe(false);
  });

  it('adds reference when file is selected', () => {
    const { result } = renderHook(() => useFileReferences([]));
    const file = { id: '1', path: '/test.ts', name: 'test.ts' };

    act(() => {
      result.current.handleSelectFile(file);
    });

    expect(result.current.references).toContain(file);
  });

  it('closes autocomplete after selection', () => {
    const { result } = renderHook(() => useFileReferences([]));

    act(() => {
      result.current.handleInputChange('@t', 2);
    });
    expect(result.current.isAutocompleteOpen).toBe(true);

    act(() => {
      result.current.handleSelectFile({ id: '1', path: '/t.ts', name: 't.ts' });
    });
    expect(result.current.isAutocompleteOpen).toBe(false);
  });
});
