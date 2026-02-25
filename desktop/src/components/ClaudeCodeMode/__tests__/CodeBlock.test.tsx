/**
 * CodeBlock Component Tests
 * Story 011-2: Code Block Actions (Copy, Line Numbers)
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { CodeBlock, SimpleCodeBlock } from '../CodeBlock';

// Mock useSettingsStore
vi.mock('../../../store/settings', () => ({
  useSettingsStore: vi.fn((selector) => {
    const state = { showLineNumbers: true };
    return selector ? selector(state) : state;
  }),
}));

describe('CodeBlock', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders code content', () => {
    render(<CodeBlock code="const x = 1;" language="javascript" />);
    expect(screen.getByText('const')).toBeInTheDocument();
  });

  it('displays language badge', () => {
    render(<CodeBlock code="console.log('hello');" language="javascript" />);
    expect(screen.getByText('JavaScript')).toBeInTheDocument();
  });

  it('shows copy button', () => {
    render(<CodeBlock code="test code" language="text" />);
    expect(screen.getByTitle('Copy code')).toBeInTheDocument();
  });

  it('copies code to clipboard when copy button is clicked', async () => {
    const code = 'const x = 42;';
    render(<CodeBlock code={code} language="javascript" />);

    const copyButton = screen.getByTitle('Copy code');
    fireEvent.click(copyButton);

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(code);
    });
  });

  it('shows "Copied!" feedback after copying', async () => {
    render(<CodeBlock code="test" language="text" />);

    const copyButton = screen.getByTitle('Copy code');
    fireEvent.click(copyButton);

    await waitFor(() => {
      expect(screen.getByText('Copied!')).toBeInTheDocument();
    });
  });

  it('renders with dark mode styling', () => {
    const { container } = render(<CodeBlock code="test" isDarkMode={true} />);
    const pre = container.querySelector('pre');
    expect(pre).toHaveStyle({ background: '#1e1e1e' });
  });

  it('renders with light mode styling', () => {
    const { container } = render(<CodeBlock code="test" isDarkMode={false} />);
    const pre = container.querySelector('pre');
    expect(pre).toHaveStyle({ background: '#f8f8f8' });
  });

  it('shows line numbers when enabled', () => {
    const { container } = render(
      <CodeBlock code={'const a = 1;\nconst b = 2;\nconst c = 3;'} language="javascript" showLineNumbers={true} />,
    );
    const lineNumbers = container.querySelectorAll('.table-cell.pr-4.text-right');
    expect(lineNumbers).toHaveLength(3);
  });

  it('hides line numbers when disabled', () => {
    const { container } = render(
      <CodeBlock code={'const a = 1;\nconst b = 2;'} language="javascript" showLineNumbers={false} />,
    );
    const lineNumbers = container.querySelectorAll('.table-cell.pr-4.text-right');
    expect(lineNumbers).toHaveLength(0);
  });

  it('respects maxHeight prop', () => {
    const { container } = render(<CodeBlock code="test" maxHeight="200px" />);
    const scrollDiv = container.querySelector('.overflow-auto');
    expect(scrollDiv).toHaveStyle({ maxHeight: '200px' });
  });

  it('supports keyboard focus', () => {
    const { container } = render(<CodeBlock code="test" />);
    const codeBlock = container.querySelector('[tabindex="0"]');
    expect(codeBlock).toBeInTheDocument();
  });
});

describe('SimpleCodeBlock', () => {
  it('renders code without header', () => {
    render(<SimpleCodeBlock code="simple code" />);
    expect(screen.getByText('simple code')).toBeInTheDocument();
    expect(screen.queryByText('Copy')).not.toBeInTheDocument();
  });

  it('applies custom className', () => {
    const { container } = render(<SimpleCodeBlock code="test" className="custom-class" />);
    expect(container.querySelector('.custom-class')).toBeInTheDocument();
  });
});
