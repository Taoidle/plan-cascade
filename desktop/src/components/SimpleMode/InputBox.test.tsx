/**
 * InputBox Component Tests
 *
 * Tests for the compact-first layout behavior:
 * - Default unfocused state has compact styling
 * - Focused state has expanded styling
 * - Content causes expanded styling even when blurred
 * - Auto-grow cap is 400px
 * - Submit behavior unchanged
 *
 * Story 002: Refine Input Box to Compact-First Layout
 * Story 003: Add Markdown Preview Toggle in Input Box
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { InputBox } from './InputBox';

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue || key,
  }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../ClaudeCodeMode/MarkdownRenderer', () => ({
  MarkdownRenderer: ({ content }: { content: string }) => (
    <div data-testid="mock-markdown-renderer">{content}</div>
  ),
}));

// --------------------------------------------------------------------------
// Test Helpers
// --------------------------------------------------------------------------

function renderInputBox(overrides: Partial<Parameters<typeof InputBox>[0]> = {}) {
  const defaultProps = {
    value: '',
    onChange: vi.fn(),
    onSubmit: vi.fn(),
  };

  return render(<InputBox {...defaultProps} {...overrides} />);
}

function getContainer() {
  // The container is the outermost div rendered by InputBox (the drop zone).
  // It's the element with the border / rounded classes.
  const textarea = screen.getByRole('textbox');
  // Walk up to find the container div with border styling
  let el: HTMLElement | null = textarea;
  while (el && !el.className.includes('border')) {
    el = el.parentElement;
  }
  return el;
}

function getInputArea() {
  // The input area div wrapping the textarea and buttons (has the padding classes)
  const textarea = screen.getByRole('textbox');
  return textarea.parentElement;
}

// --------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------

describe('InputBox compact-first layout', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('container styling', () => {
    it('uses border (not border-2) on the container', () => {
      renderInputBox();
      const container = getContainer();
      expect(container).not.toBeNull();
      // Should have single-width border class, not border-2
      expect(container!.className).not.toContain('border-2');
      // The container should include 'border' as a standalone class
      // (not just as part of border-gray-200 etc.)
      expect(container!.className).toMatch(/\bborder\b/);
    });

    it('uses rounded-lg (not rounded-xl) on the container', () => {
      renderInputBox();
      const container = getContainer();
      expect(container).not.toBeNull();
      expect(container!.className).toContain('rounded-lg');
      expect(container!.className).not.toContain('rounded-xl');
    });
  });

  describe('input area padding', () => {
    it('uses px-3 py-2 (not p-4) on the input area', () => {
      renderInputBox();
      const inputArea = getInputArea();
      expect(inputArea).not.toBeNull();
      expect(inputArea!.className).toContain('px-3');
      expect(inputArea!.className).toContain('py-2');
      expect(inputArea!.className).not.toContain('p-4');
    });
  });

  describe('compact vs expanded state', () => {
    it('has compact styling when unfocused and empty', () => {
      renderInputBox({ value: '' });
      const container = getContainer();
      expect(container).not.toBeNull();
      // In compact/unfocused state, the border should be lighter
      expect(container!.className).toContain('border-gray-200');
    });

    it('expands on focus', () => {
      renderInputBox({ value: '' });
      const textarea = screen.getByRole('textbox');

      fireEvent.focus(textarea);

      const container = getContainer();
      expect(container).not.toBeNull();
      // When focused, the container should show the focused styling
      expect(container!.className).toContain('border-primary-500');
    });

    it('remains expanded when content exists even after blur', () => {
      const { rerender } = render(
        <InputBox value="some content" onChange={vi.fn()} onSubmit={vi.fn()} />
      );
      const textarea = screen.getByRole('textbox');

      // Focus then blur
      fireEvent.focus(textarea);
      fireEvent.blur(textarea);

      const container = getContainer();
      expect(container).not.toBeNull();
      // With content, should still show the "expanded" border style
      // (not the ultra-light unfocused-empty style)
      expect(container!.className).toContain('border-gray-300');
    });

    it('returns to compact styling when blurred and empty', () => {
      renderInputBox({ value: '' });
      const textarea = screen.getByRole('textbox');

      // Focus then blur with no content
      fireEvent.focus(textarea);
      fireEvent.blur(textarea);

      const container = getContainer();
      expect(container).not.toBeNull();
      // Should return to the lighter unfocused-empty border
      expect(container!.className).toContain('border-gray-200');
    });
  });

  describe('auto-grow cap', () => {
    it('limits textarea height to 400px', () => {
      renderInputBox({ value: '' });
      const textarea = screen.getByRole('textbox') as HTMLTextAreaElement;

      // Simulate the textarea having a large scrollHeight
      Object.defineProperty(textarea, 'scrollHeight', {
        value: 600,
        configurable: true,
      });

      // Trigger the input handler which runs auto-resize
      fireEvent.input(textarea);

      // The textarea height should be capped at 400px
      expect(textarea.style.height).toBe('400px');
    });

    it('uses actual scrollHeight when under 400px', () => {
      renderInputBox({ value: '' });
      const textarea = screen.getByRole('textbox') as HTMLTextAreaElement;

      Object.defineProperty(textarea, 'scrollHeight', {
        value: 150,
        configurable: true,
      });

      fireEvent.input(textarea);

      expect(textarea.style.height).toBe('150px');
    });
  });

  describe('submit behavior', () => {
    it('submits on Cmd+Enter', () => {
      const onSubmit = vi.fn();
      renderInputBox({ value: 'test message', onSubmit });
      const textarea = screen.getByRole('textbox');

      fireEvent.keyDown(textarea, { key: 'Enter', metaKey: true });

      expect(onSubmit).toHaveBeenCalledTimes(1);
    });

    it('submits on Ctrl+Enter', () => {
      const onSubmit = vi.fn();
      renderInputBox({ value: 'test message', onSubmit });
      const textarea = screen.getByRole('textbox');

      fireEvent.keyDown(textarea, { key: 'Enter', ctrlKey: true });

      expect(onSubmit).toHaveBeenCalledTimes(1);
    });

    it('does not submit when disabled', () => {
      const onSubmit = vi.fn();
      renderInputBox({ value: 'test message', onSubmit, disabled: true });
      const textarea = screen.getByRole('textbox');

      fireEvent.keyDown(textarea, { key: 'Enter', metaKey: true });

      expect(onSubmit).not.toHaveBeenCalled();
    });
  });

  describe('file chips padding', () => {
    it('uses px-3 pt-2 pb-1 for attachment chips area', () => {
      const attachments = [
        {
          id: 'test-1',
          name: 'test.txt',
          path: '/path/test.txt',
          size: 100,
          type: 'text' as const,
          content: 'hello',
        },
      ];
      renderInputBox({ attachments, onAttach: vi.fn(), onRemoveAttachment: vi.fn() });

      // Find the file chip container (first child div with flex-wrap)
      const container = getContainer();
      expect(container).not.toBeNull();
      const chipContainer = container!.querySelector('.flex.flex-wrap');
      expect(chipContainer).not.toBeNull();
      expect(chipContainer!.className).toContain('px-3');
      expect(chipContainer!.className).toContain('pt-2');
      expect(chipContainer!.className).toContain('pb-1');
      expect(chipContainer!.className).not.toContain('px-4');
      expect(chipContainer!.className).not.toContain('pt-3');
    });
  });
});

// --------------------------------------------------------------------------
// Story 003: Markdown Preview Toggle
// --------------------------------------------------------------------------

describe('InputBox markdown preview toggle', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('toggle visibility', () => {
    it('does not show preview toggle when input is empty', () => {
      renderInputBox({ value: '' });
      expect(screen.queryByTestId('preview-toggle')).toBeNull();
    });

    it('does not show preview toggle when value is whitespace only', () => {
      renderInputBox({ value: '   ' });
      expect(screen.queryByTestId('preview-toggle')).toBeNull();
    });

    it('shows preview toggle when value has content', () => {
      renderInputBox({ value: 'hello world' });
      expect(screen.getByTestId('preview-toggle')).toBeTruthy();
    });
  });

  describe('toggle behavior', () => {
    it('starts in edit mode with textarea visible', () => {
      renderInputBox({ value: '# Hello' });
      expect(screen.getByRole('textbox')).toBeTruthy();
      expect(screen.queryByTestId('markdown-preview')).toBeNull();
    });

    it('switches to preview mode when toggle is clicked', () => {
      renderInputBox({ value: '# Hello' });
      const toggle = screen.getByTestId('preview-toggle');

      fireEvent.click(toggle);

      // Textarea should be gone, preview should be visible
      expect(screen.queryByRole('textbox')).toBeNull();
      expect(screen.getByTestId('markdown-preview')).toBeTruthy();
    });

    it('renders current value through MarkdownRenderer in preview mode', () => {
      renderInputBox({ value: '# Hello World' });
      const toggle = screen.getByTestId('preview-toggle');

      fireEvent.click(toggle);

      // The mock renderer renders content as text
      const renderer = screen.getByTestId('mock-markdown-renderer');
      expect(renderer.textContent).toBe('# Hello World');
    });

    it('switches back to edit mode when toggle is clicked again', () => {
      renderInputBox({ value: '# Hello' });
      const toggle = screen.getByTestId('preview-toggle');

      // Enter preview
      fireEvent.click(toggle);
      expect(screen.queryByRole('textbox')).toBeNull();

      // Exit preview
      fireEvent.click(screen.getByTestId('preview-toggle'));
      expect(screen.getByRole('textbox')).toBeTruthy();
      expect(screen.queryByTestId('markdown-preview')).toBeNull();
    });
  });

  describe('toggle icon and title', () => {
    it('shows eye icon with preview title in edit mode', () => {
      renderInputBox({ value: 'some text' });
      const toggle = screen.getByTestId('preview-toggle');
      // Title should be the preview label (key fallback since no defaultValue)
      expect(toggle.getAttribute('title')).toBe('input.previewMarkdown');
    });

    it('shows pencil icon with edit title in preview mode', () => {
      renderInputBox({ value: 'some text' });
      const toggle = screen.getByTestId('preview-toggle');

      fireEvent.click(toggle);

      const toggleAfter = screen.getByTestId('preview-toggle');
      expect(toggleAfter.getAttribute('title')).toBe('input.switchToEdit');
    });
  });

  describe('submission in preview mode', () => {
    it('submit button remains functional in preview mode', () => {
      const onSubmit = vi.fn();
      renderInputBox({ value: 'test content', onSubmit });

      // Switch to preview
      fireEvent.click(screen.getByTestId('preview-toggle'));

      // Find and click submit button (the one with bg-primary-600)
      const submitBtn = screen.getByTitle('input.submitTitle');
      fireEvent.click(submitBtn);

      expect(onSubmit).toHaveBeenCalledTimes(1);
    });
  });

  describe('attachments in preview mode', () => {
    it('file chips remain visible in preview mode', () => {
      const attachments = [
        {
          id: 'att-1',
          name: 'readme.md',
          path: '/readme.md',
          size: 256,
          type: 'text' as const,
          content: '# README',
        },
      ];
      renderInputBox({
        value: 'content with attachment',
        attachments,
        onAttach: vi.fn(),
        onRemoveAttachment: vi.fn(),
      });

      // Switch to preview
      fireEvent.click(screen.getByTestId('preview-toggle'));

      // Attachment chip should still be visible
      expect(screen.getByText('readme.md')).toBeTruthy();
    });

    it('attach button remains visible in preview mode', () => {
      renderInputBox({
        value: 'content',
        onAttach: vi.fn(),
      });

      // Switch to preview
      fireEvent.click(screen.getByTestId('preview-toggle'));

      // Attach button should still be there
      const attachBtn = screen.getByTitle('Pick a file');
      expect(attachBtn).toBeTruthy();
    });
  });
});
