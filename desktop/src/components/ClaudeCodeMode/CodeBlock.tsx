/**
 * CodeBlock Component
 *
 * Interactive code block with copy button, line numbers toggle,
 * and language badge. Supports horizontal scrolling for long lines.
 *
 * Story 011-2: Code Block Actions (Copy, Line Numbers)
 */

import { useState, useCallback, useRef, useEffect, memo } from 'react';
import { Highlight, themes, Language } from 'prism-react-renderer';
import { clsx } from 'clsx';
import { CopyIcon, CheckIcon, CodeIcon } from '@radix-ui/react-icons';
import { useSettingsStore } from '../../store/settings';

// ============================================================================
// Types
// ============================================================================

interface CodeBlockProps {
  code: string;
  language?: string;
  isDarkMode?: boolean;
  showLineNumbers?: boolean;
  maxHeight?: string;
  className?: string;
}

// ============================================================================
// Language Display Names
// ============================================================================

const languageDisplayNames: Record<string, string> = {
  javascript: 'JavaScript',
  typescript: 'TypeScript',
  tsx: 'TSX',
  jsx: 'JSX',
  python: 'Python',
  ruby: 'Ruby',
  rust: 'Rust',
  go: 'Go',
  java: 'Java',
  kotlin: 'Kotlin',
  csharp: 'C#',
  cpp: 'C++',
  c: 'C',
  bash: 'Bash',
  shell: 'Shell',
  json: 'JSON',
  yaml: 'YAML',
  xml: 'XML',
  html: 'HTML',
  css: 'CSS',
  scss: 'SCSS',
  sass: 'Sass',
  less: 'Less',
  sql: 'SQL',
  markdown: 'Markdown',
  docker: 'Dockerfile',
  makefile: 'Makefile',
  toml: 'TOML',
  ini: 'INI',
  diff: 'Diff',
  graphql: 'GraphQL',
  swift: 'Swift',
  php: 'PHP',
  lua: 'Lua',
  r: 'R',
  scala: 'Scala',
  elixir: 'Elixir',
  erlang: 'Erlang',
  clojure: 'Clojure',
  haskell: 'Haskell',
  ocaml: 'OCaml',
  fsharp: 'F#',
  vim: 'Vim',
  powershell: 'PowerShell',
  text: 'Plain Text',
};

function getLanguageDisplayName(lang: string): string {
  return languageDisplayNames[lang.toLowerCase()] || lang.toUpperCase();
}

// Map language names to prism-react-renderer supported languages
function normalizeLanguage(lang: string): Language {
  const languageMap: Record<string, Language> = {
    js: 'javascript',
    ts: 'typescript',
    py: 'python',
    rb: 'ruby',
    rs: 'rust',
    sh: 'bash',
    zsh: 'bash',
    shell: 'bash',
    yml: 'yaml',
    dockerfile: 'docker',
    md: 'markdown',
    cs: 'csharp',
    ps1: 'powershell',
  };

  const normalized = languageMap[lang.toLowerCase()] || lang.toLowerCase();
  return normalized as Language;
}

// ============================================================================
// CodeBlock Component
// ============================================================================

export const CodeBlock = memo(function CodeBlock({
  code,
  language = 'text',
  isDarkMode = true,
  showLineNumbers: showLineNumbersProp,
  maxHeight = '400px',
  className,
}: CodeBlockProps) {
  const [copied, setCopied] = useState(false);
  const codeRef = useRef<HTMLDivElement>(null);

  // Get line numbers preference from settings if not explicitly set
  const lineNumbersFromSettings = useSettingsStore((state) => state.showLineNumbers ?? true);
  const showLineNumbers = showLineNumbersProp ?? lineNumbersFromSettings;

  // Copy to clipboard handler
  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy code:', err);
    }
  }, [code]);

  // Handle keyboard shortcut for copy when focused
  useEffect(() => {
    const element = codeRef.current;
    if (!element) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'c') {
        const selection = window.getSelection();
        // Only copy entire code if there's no text selection
        if (!selection || selection.toString().length === 0) {
          e.preventDefault();
          handleCopy();
        }
      }
    };

    element.addEventListener('keydown', handleKeyDown);
    return () => element.removeEventListener('keydown', handleKeyDown);
  }, [handleCopy]);

  const theme = isDarkMode ? themes.oneDark : themes.oneLight;
  const normalizedLang = normalizeLanguage(language);

  return (
    <div
      ref={codeRef}
      tabIndex={0}
      className={clsx(
        'relative group rounded-lg overflow-hidden',
        'border border-gray-200 dark:border-gray-700',
        'focus:outline-none focus:ring-2 focus:ring-primary-500',
        className,
      )}
    >
      {/* Header with language badge and copy button */}
      <div
        className={clsx(
          'flex items-center justify-between px-4 py-2',
          'bg-gray-100 dark:bg-gray-800',
          'border-b border-gray-200 dark:border-gray-700',
        )}
      >
        {/* Language badge */}
        <div className="flex items-center gap-2">
          <CodeIcon className="w-4 h-4 text-gray-500 dark:text-gray-400" />
          <span className="text-xs font-medium text-gray-600 dark:text-gray-300">
            {getLanguageDisplayName(language)}
          </span>
        </div>

        {/* Copy button */}
        <button
          onClick={handleCopy}
          className={clsx(
            'flex items-center gap-1.5 px-2 py-1 rounded',
            'text-xs font-medium transition-all',
            copied
              ? 'bg-green-100 dark:bg-green-900/50 text-green-700 dark:text-green-400'
              : 'bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-gray-600',
          )}
          title={copied ? 'Copied!' : 'Copy code'}
        >
          {copied ? (
            <>
              <CheckIcon className="w-3.5 h-3.5" />
              <span>Copied!</span>
            </>
          ) : (
            <>
              <CopyIcon className="w-3.5 h-3.5" />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>

      {/* Code content */}
      <div className="overflow-auto" style={{ maxHeight }}>
        <Highlight theme={theme} code={code} language={normalizedLang}>
          {({ className: highlightClassName, style, tokens, getLineProps, getTokenProps }) => (
            <pre
              className={clsx(highlightClassName, 'p-4 m-0 text-sm leading-relaxed')}
              style={{
                ...style,
                background: isDarkMode ? '#1e1e1e' : '#f8f8f8',
                fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace',
              }}
            >
              {tokens.map((line, i) => (
                <div key={i} {...getLineProps({ line })} className="table-row">
                  {showLineNumbers && (
                    <span
                      className={clsx('table-cell pr-4 text-right select-none', 'text-gray-400 dark:text-gray-600')}
                      style={{ minWidth: '3em' }}
                    >
                      {i + 1}
                    </span>
                  )}
                  <span className="table-cell">
                    {line.map((token, key) => (
                      <span key={key} {...getTokenProps({ token })} />
                    ))}
                  </span>
                </div>
              ))}
            </pre>
          )}
        </Highlight>
      </div>
    </div>
  );
});

// ============================================================================
// Simple CodeBlock without header (for inline use)
// ============================================================================

interface SimpleCodeBlockProps {
  code: string;
  language?: string;
  isDarkMode?: boolean;
  className?: string;
}

export const SimpleCodeBlock = memo(function SimpleCodeBlock({
  code,
  language = 'text',
  isDarkMode = true,
  className,
}: SimpleCodeBlockProps) {
  const theme = isDarkMode ? themes.oneDark : themes.oneLight;
  const normalizedLang = normalizeLanguage(language);

  return (
    <div className={clsx('rounded-lg overflow-hidden', className)}>
      <Highlight theme={theme} code={code} language={normalizedLang}>
        {({ className: highlightClassName, style, tokens, getLineProps, getTokenProps }) => (
          <pre
            className={clsx(highlightClassName, 'p-3 m-0 text-sm leading-relaxed')}
            style={{
              ...style,
              background: isDarkMode ? '#1e1e1e' : '#f8f8f8',
              fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace',
            }}
          >
            {tokens.map((line, i) => (
              <div key={i} {...getLineProps({ line })}>
                {line.map((token, key) => (
                  <span key={key} {...getTokenProps({ token })} />
                ))}
              </div>
            ))}
          </pre>
        )}
      </Highlight>
    </div>
  );
});

export default CodeBlock;
