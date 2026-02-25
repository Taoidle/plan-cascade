/**
 * AnsiOutput Component
 *
 * Terminal-like output panel for Bash tool results with full ANSI color
 * and formatting support. Includes search, text selection, and output scrolling.
 *
 * Story-004: Bash output panel with ANSI color support
 */

import { useState, useMemo, useRef, useEffect, useCallback, KeyboardEvent } from 'react';
import { clsx } from 'clsx';
import {
  MagnifyingGlassIcon,
  Cross2Icon,
  ChevronDownIcon,
  ChevronUpIcon,
  CopyIcon,
  CheckIcon,
  TrashIcon,
  TextAlignJustifyIcon,
  ListBulletIcon,
} from '@radix-ui/react-icons';

// ============================================================================
// ANSI Color Definitions
// ============================================================================

// Standard 16 colors (0-15)
const ANSI_COLORS: Record<number, string> = {
  0: '#000000', // Black
  1: '#cd0000', // Red
  2: '#00cd00', // Green
  3: '#cdcd00', // Yellow
  4: '#0000ee', // Blue
  5: '#cd00cd', // Magenta
  6: '#00cdcd', // Cyan
  7: '#e5e5e5', // White
  8: '#7f7f7f', // Bright Black (Gray)
  9: '#ff0000', // Bright Red
  10: '#00ff00', // Bright Green
  11: '#ffff00', // Bright Yellow
  12: '#5c5cff', // Bright Blue
  13: '#ff00ff', // Bright Magenta
  14: '#00ffff', // Bright Cyan
  15: '#ffffff', // Bright White
};

// Convert 256-color index to RGB
function get256Color(colorIndex: number): string {
  // Standard colors (0-15)
  if (colorIndex < 16) {
    return ANSI_COLORS[colorIndex] || '#ffffff';
  }

  // 216 color cube (16-231)
  if (colorIndex < 232) {
    const index = colorIndex - 16;
    const r = Math.floor(index / 36);
    const g = Math.floor((index % 36) / 6);
    const b = index % 6;
    const toHex = (v: number) => (v === 0 ? 0 : 55 + v * 40).toString(16).padStart(2, '0');
    return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
  }

  // Grayscale (232-255)
  const gray = (colorIndex - 232) * 10 + 8;
  const hex = gray.toString(16).padStart(2, '0');
  return `#${hex}${hex}${hex}`;
}

// ============================================================================
// ANSI Parser Types
// ============================================================================

interface TextSpan {
  text: string;
  style: SpanStyle;
}

interface SpanStyle {
  color?: string;
  backgroundColor?: string;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strikethrough?: boolean;
  dim?: boolean;
}

interface ParsedLine {
  spans: TextSpan[];
  lineNumber: number;
}

// ============================================================================
// ANSI Parser
// ============================================================================

function parseAnsiText(text: string): ParsedLine[] {
  const lines: ParsedLine[] = [];
  const rawLines = text.split('\n');

  let currentStyle: SpanStyle = {};

  rawLines.forEach((rawLine, lineIndex) => {
    const spans: TextSpan[] = [];
    let currentText = '';
    let i = 0;

    while (i < rawLine.length) {
      // Check for escape sequence
      if (rawLine[i] === '\x1b' && rawLine[i + 1] === '[') {
        // Save any accumulated text
        if (currentText) {
          spans.push({ text: currentText, style: { ...currentStyle } });
          currentText = '';
        }

        // Find the end of the escape sequence
        let end = i + 2;
        while (end < rawLine.length && !/[A-Za-z]/.test(rawLine[end])) {
          end++;
        }

        // Parse the SGR (Select Graphic Rendition) sequence
        if (rawLine[end] === 'm') {
          const codes = rawLine
            .slice(i + 2, end)
            .split(';')
            .map(Number);
          currentStyle = applyAnsiCodes(currentStyle, codes);
        }

        i = end + 1;
      } else {
        currentText += rawLine[i];
        i++;
      }
    }

    // Add remaining text
    if (currentText) {
      spans.push({ text: currentText, style: { ...currentStyle } });
    }

    // If line is empty, add an empty span to preserve the line
    if (spans.length === 0) {
      spans.push({ text: '', style: {} });
    }

    lines.push({ spans, lineNumber: lineIndex + 1 });
  });

  return lines;
}

function applyAnsiCodes(currentStyle: SpanStyle, codes: number[]): SpanStyle {
  const style = { ...currentStyle };
  let i = 0;

  while (i < codes.length) {
    const code = codes[i];

    // Reset
    if (code === 0) {
      return {};
    }

    // Bold
    if (code === 1) {
      style.bold = true;
    }

    // Dim
    if (code === 2) {
      style.dim = true;
    }

    // Italic
    if (code === 3) {
      style.italic = true;
    }

    // Underline
    if (code === 4) {
      style.underline = true;
    }

    // Strikethrough
    if (code === 9) {
      style.strikethrough = true;
    }

    // Normal intensity (not bold, not dim)
    if (code === 22) {
      style.bold = false;
      style.dim = false;
    }

    // Not italic
    if (code === 23) {
      style.italic = false;
    }

    // Not underlined
    if (code === 24) {
      style.underline = false;
    }

    // Not strikethrough
    if (code === 29) {
      style.strikethrough = false;
    }

    // Foreground colors (30-37, 90-97)
    if (code >= 30 && code <= 37) {
      style.color = ANSI_COLORS[code - 30];
    }
    if (code >= 90 && code <= 97) {
      style.color = ANSI_COLORS[code - 90 + 8];
    }

    // Default foreground color
    if (code === 39) {
      delete style.color;
    }

    // Background colors (40-47, 100-107)
    if (code >= 40 && code <= 47) {
      style.backgroundColor = ANSI_COLORS[code - 40];
    }
    if (code >= 100 && code <= 107) {
      style.backgroundColor = ANSI_COLORS[code - 100 + 8];
    }

    // Default background color
    if (code === 49) {
      delete style.backgroundColor;
    }

    // 256 colors and 24-bit colors
    if (code === 38 || code === 48) {
      const isBackground = code === 48;
      const mode = codes[i + 1];

      if (mode === 5 && codes.length > i + 2) {
        // 256 color mode
        const colorIndex = codes[i + 2];
        const color = get256Color(colorIndex);
        if (isBackground) {
          style.backgroundColor = color;
        } else {
          style.color = color;
        }
        i += 2;
      } else if (mode === 2 && codes.length > i + 4) {
        // 24-bit color mode
        const r = codes[i + 2];
        const g = codes[i + 3];
        const b = codes[i + 4];
        const color = `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`;
        if (isBackground) {
          style.backgroundColor = color;
        } else {
          style.color = color;
        }
        i += 4;
      }
    }

    i++;
  }

  return style;
}

// ============================================================================
// Component Props
// ============================================================================

interface AnsiOutputProps {
  /** The output text with ANSI codes */
  output: string;
  /** Exit code of the command (optional) */
  exitCode?: number;
  /** Duration of the command in ms (optional) */
  duration?: number;
  /** Whether to show line numbers (default: false) */
  showLineNumbers?: boolean;
  /** Whether to wrap long lines (default: true) */
  wrapLines?: boolean;
  /** Whether auto-scroll is enabled (default: true) */
  autoScroll?: boolean;
  /** Maximum height in pixels (default: 400) */
  maxHeight?: number;
  /** Callback when output is cleared */
  onClear?: () => void;
  /** Additional CSS classes */
  className?: string;
}

// ============================================================================
// AnsiOutput Component
// ============================================================================

export function AnsiOutput({
  output,
  exitCode,
  duration,
  showLineNumbers: initialShowLineNumbers = false,
  wrapLines: initialWrapLines = true,
  autoScroll: initialAutoScroll = true,
  maxHeight = 400,
  onClear,
  className,
}: AnsiOutputProps) {
  // State
  const [showLineNumbers, setShowLineNumbers] = useState(initialShowLineNumbers);
  const [wrapLines, setWrapLines] = useState(initialWrapLines);
  const [autoScroll] = useState(initialAutoScroll);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchOpen, setSearchOpen] = useState(false);
  const [currentMatchIndex, setCurrentMatchIndex] = useState(0);
  const [copied, setCopied] = useState(false);

  // Refs
  const containerRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Parse ANSI output
  const parsedLines = useMemo(() => parseAnsiText(output), [output]);

  // Search matches
  const searchMatches = useMemo(() => {
    if (!searchQuery.trim()) return [];

    const matches: { lineIndex: number; startIndex: number; length: number }[] = [];
    const query = searchQuery.toLowerCase();

    parsedLines.forEach((line, lineIndex) => {
      const lineText = line.spans
        .map((s) => s.text)
        .join('')
        .toLowerCase();
      let startIndex = 0;

      while (true) {
        const foundIndex = lineText.indexOf(query, startIndex);
        if (foundIndex === -1) break;
        matches.push({
          lineIndex,
          startIndex: foundIndex,
          length: searchQuery.length,
        });
        startIndex = foundIndex + 1;
      }
    });

    return matches;
  }, [parsedLines, searchQuery]);

  // Auto-scroll effect
  useEffect(() => {
    if (autoScroll && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [output, autoScroll]);

  // Focus search input when opened
  useEffect(() => {
    if (searchOpen && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [searchOpen]);

  // Handle keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: globalThis.KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        e.preventDefault();
        setSearchOpen(true);
      }
      if (e.key === 'Escape' && searchOpen) {
        setSearchOpen(false);
        setSearchQuery('');
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [searchOpen]);

  // Navigate to next/previous match
  const navigateMatch = useCallback(
    (direction: 'next' | 'prev') => {
      if (searchMatches.length === 0) return;

      let newIndex = currentMatchIndex;
      if (direction === 'next') {
        newIndex = (currentMatchIndex + 1) % searchMatches.length;
      } else {
        newIndex = (currentMatchIndex - 1 + searchMatches.length) % searchMatches.length;
      }
      setCurrentMatchIndex(newIndex);
    },
    [currentMatchIndex, searchMatches.length],
  );

  // Scroll to bottom
  const scrollToBottom = useCallback(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, []);

  // Copy output (strips ANSI codes)
  const handleCopy = useCallback(async () => {
    const plainText = parsedLines.map((line) => line.spans.map((s) => s.text).join('')).join('\n');

    try {
      await navigator.clipboard.writeText(plainText);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  }, [parsedLines]);

  // Handle search input key events
  const handleSearchKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      navigateMatch(e.shiftKey ? 'prev' : 'next');
    }
  };

  return (
    <div className={clsx('flex flex-col', className)}>
      {/* Header */}
      <div
        className={clsx(
          'flex items-center justify-between px-3 py-2',
          'bg-gray-800 border-b border-gray-700',
          'rounded-t-lg',
        )}
      >
        <div className="flex items-center gap-3">
          {/* Exit code */}
          {exitCode !== undefined && (
            <span
              className={clsx(
                'px-2 py-0.5 rounded text-xs font-mono',
                exitCode === 0 ? 'bg-green-900/50 text-green-400' : 'bg-red-900/50 text-red-400',
              )}
            >
              Exit: {exitCode}
            </span>
          )}

          {/* Duration */}
          {duration !== undefined && (
            <span className="text-xs text-gray-400">
              {duration < 1000 ? `${duration}ms` : `${(duration / 1000).toFixed(1)}s`}
            </span>
          )}
        </div>

        {/* Controls */}
        <div className="flex items-center gap-1">
          {/* Search toggle */}
          <button
            onClick={() => setSearchOpen(!searchOpen)}
            className={clsx(
              'p-1.5 rounded transition-colors',
              searchOpen ? 'bg-primary-600 text-white' : 'text-gray-400 hover:text-white hover:bg-gray-700',
            )}
            title="Search (Ctrl+F)"
          >
            <MagnifyingGlassIcon className="w-4 h-4" />
          </button>

          {/* Line numbers toggle */}
          <button
            onClick={() => setShowLineNumbers(!showLineNumbers)}
            className={clsx(
              'p-1.5 rounded transition-colors',
              showLineNumbers ? 'bg-primary-600 text-white' : 'text-gray-400 hover:text-white hover:bg-gray-700',
            )}
            title="Toggle line numbers"
          >
            <ListBulletIcon className="w-4 h-4" />
          </button>

          {/* Wrap lines toggle */}
          <button
            onClick={() => setWrapLines(!wrapLines)}
            className={clsx(
              'p-1.5 rounded transition-colors',
              wrapLines ? 'bg-primary-600 text-white' : 'text-gray-400 hover:text-white hover:bg-gray-700',
            )}
            title="Toggle word wrap"
          >
            <TextAlignJustifyIcon className="w-4 h-4" />
          </button>

          {/* Copy button */}
          <button
            onClick={handleCopy}
            className={clsx(
              'p-1.5 rounded transition-colors',
              copied ? 'bg-green-600 text-white' : 'text-gray-400 hover:text-white hover:bg-gray-700',
            )}
            title={copied ? 'Copied!' : 'Copy output'}
          >
            {copied ? <CheckIcon className="w-4 h-4" /> : <CopyIcon className="w-4 h-4" />}
          </button>

          {/* Clear button */}
          {onClear && (
            <button
              onClick={onClear}
              className="p-1.5 rounded text-gray-400 hover:text-white hover:bg-gray-700 transition-colors"
              title="Clear output"
            >
              <TrashIcon className="w-4 h-4" />
            </button>
          )}

          {/* Scroll to bottom */}
          <button
            onClick={scrollToBottom}
            className="p-1.5 rounded text-gray-400 hover:text-white hover:bg-gray-700 transition-colors"
            title="Scroll to bottom"
          >
            <ChevronDownIcon className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Search bar */}
      {searchOpen && (
        <div className={clsx('flex items-center gap-2 px-3 py-2', 'bg-gray-800 border-b border-gray-700')}>
          <MagnifyingGlassIcon className="w-4 h-4 text-gray-400" />
          <input
            ref={searchInputRef}
            type="text"
            value={searchQuery}
            onChange={(e) => {
              setSearchQuery(e.target.value);
              setCurrentMatchIndex(0);
            }}
            onKeyDown={handleSearchKeyDown}
            placeholder="Search..."
            className={clsx(
              'flex-1 bg-gray-900 text-white px-2 py-1 rounded text-sm',
              'border border-gray-700 focus:border-primary-500 focus:outline-none',
            )}
          />
          {searchQuery && (
            <span className="text-xs text-gray-400">
              {searchMatches.length > 0 ? `${currentMatchIndex + 1}/${searchMatches.length}` : 'No matches'}
            </span>
          )}
          <button
            onClick={() => navigateMatch('prev')}
            disabled={searchMatches.length === 0}
            className="p-1 rounded text-gray-400 hover:text-white disabled:opacity-50"
          >
            <ChevronUpIcon className="w-4 h-4" />
          </button>
          <button
            onClick={() => navigateMatch('next')}
            disabled={searchMatches.length === 0}
            className="p-1 rounded text-gray-400 hover:text-white disabled:opacity-50"
          >
            <ChevronDownIcon className="w-4 h-4" />
          </button>
          <button
            onClick={() => {
              setSearchOpen(false);
              setSearchQuery('');
            }}
            className="p-1 rounded text-gray-400 hover:text-white"
          >
            <Cross2Icon className="w-4 h-4" />
          </button>
        </div>
      )}

      {/* Output content */}
      <div
        ref={containerRef}
        className={clsx('bg-gray-900 text-gray-100 p-3 rounded-b-lg overflow-auto', 'font-mono text-sm')}
        style={{ maxHeight }}
      >
        {parsedLines.map((line, lineIndex) => (
          <div key={lineIndex} className={clsx('flex', !wrapLines && 'whitespace-nowrap')}>
            {/* Line number */}
            {showLineNumbers && (
              <span className="select-none w-12 text-right pr-3 text-gray-500 flex-shrink-0">{line.lineNumber}</span>
            )}

            {/* Line content */}
            <div className={clsx(wrapLines && 'break-all')}>
              {line.spans.map((span, spanIndex) => (
                <AnsiSpan
                  key={spanIndex}
                  span={span}
                  lineIndex={lineIndex}
                  spanIndex={spanIndex}
                  searchQuery={searchQuery}
                  searchMatches={searchMatches}
                  currentMatchIndex={currentMatchIndex}
                />
              ))}
              {/* Preserve empty lines */}
              {line.spans.length === 1 && line.spans[0].text === '' && '\u00A0'}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ============================================================================
// AnsiSpan Component
// ============================================================================

interface AnsiSpanProps {
  span: TextSpan;
  lineIndex: number;
  spanIndex: number;
  searchQuery: string;
  searchMatches: { lineIndex: number; startIndex: number; length: number }[];
  currentMatchIndex: number;
}

function AnsiSpan({ span, lineIndex, searchQuery, searchMatches, currentMatchIndex }: AnsiSpanProps) {
  const style: React.CSSProperties = {};

  if (span.style.color) style.color = span.style.color;
  if (span.style.backgroundColor) style.backgroundColor = span.style.backgroundColor;
  if (span.style.bold) style.fontWeight = 'bold';
  if (span.style.italic) style.fontStyle = 'italic';
  if (span.style.dim) style.opacity = 0.5;

  const textDecoration: string[] = [];
  if (span.style.underline) textDecoration.push('underline');
  if (span.style.strikethrough) textDecoration.push('line-through');
  if (textDecoration.length) style.textDecoration = textDecoration.join(' ');

  // Highlight search matches
  if (searchQuery && span.text) {
    const parts: JSX.Element[] = [];
    const text = span.text;
    // Note: query is used for case-insensitive matching in searchMatches calculation
    let lastIndex = 0;

    // Find matches in this line
    const lineMatches = searchMatches.filter((m) => m.lineIndex === lineIndex);

    lineMatches.forEach((match, idx) => {
      if (match.startIndex > lastIndex) {
        parts.push(
          <span key={`text-${idx}`} style={style}>
            {text.slice(lastIndex, match.startIndex)}
          </span>,
        );
      }

      const isCurrentMatch = searchMatches.indexOf(match) === currentMatchIndex;

      parts.push(
        <mark
          key={`match-${idx}`}
          className={clsx(
            'rounded px-0.5',
            isCurrentMatch ? 'bg-yellow-400 text-black' : 'bg-yellow-600/50 text-white',
          )}
          style={{ ...style, backgroundColor: undefined }}
        >
          {text.slice(match.startIndex, match.startIndex + match.length)}
        </mark>,
      );

      lastIndex = match.startIndex + match.length;
    });

    if (lastIndex < text.length) {
      parts.push(
        <span key="text-end" style={style}>
          {text.slice(lastIndex)}
        </span>,
      );
    }

    if (parts.length > 0) {
      return <>{parts}</>;
    }
  }

  return <span style={style}>{span.text}</span>;
}

// ============================================================================
// Exports
// ============================================================================

export { parseAnsiText, get256Color };
export default AnsiOutput;
